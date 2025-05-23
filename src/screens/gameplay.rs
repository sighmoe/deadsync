use crate::assets::{AssetManager, FontId, TextureId}; // Added FontId
use crate::config;
use crate::graphics::renderer::{DescriptorSetId, Renderer};
use crate::parsing::simfile::{ChartInfo, NoteChar, ProcessedChartData, SongInfo};
use crate::state::{
    ActiveExplosion,
    AppState, Arrow, ArrowDirection, GameState, Judgment, TargetInfo,
    VirtualKeyCode, ALL_ARROW_DIRECTIONS, ALL_JUDGMENTS,
};
use ash::vk;
use cgmath::{Rad, Vector3};
use log::{debug, error, info, trace, warn};
use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;
use std::sync::Arc;
use std::time::Instant;
use winit::event::{ElementState, KeyEvent};
use rand::Rng;


// --- Placeholder for TimingData ---
#[derive(Debug, Clone, Default)]
pub struct BeatTimePoint { pub beat: f32, pub time_sec: f32, pub bpm: f32, }
#[derive(Debug, Clone, Default)]
pub struct TimingData { pub points: Vec<BeatTimePoint>, pub stops_at_beat: Vec<(f32, f32)>, pub song_offset_sec: f32, }
impl TimingData {
    pub fn get_time_for_beat(&self, target_beat: f32) -> f32 {
        if self.points.is_empty() { return self.song_offset_sec + target_beat * 0.5; }
        let mut current_time_sec = 0.0;
        let mut last_beat = 0.0;
        let mut last_bpm = self.points[0].bpm;
        for point in &self.points {
            if point.beat <= last_beat { last_bpm = point.bpm; continue; }
            if target_beat >= point.beat {
                current_time_sec += (point.beat - last_beat) * (60.0 / last_bpm);
                last_beat = point.beat; last_bpm = point.bpm;
            } else {
                current_time_sec += (target_beat - last_beat) * (60.0 / last_bpm);
                last_beat = target_beat; break;
            }
        }
        if target_beat > last_beat { current_time_sec += (target_beat - last_beat) * (60.0 / last_bpm); }
        let mut final_time = self.song_offset_sec + current_time_sec;
        for (stop_beat, stop_duration_sec) in &self.stops_at_beat {
            if *stop_beat < target_beat { final_time += stop_duration_sec; }
        }
        final_time
    }
    pub fn get_beat_for_time(&self, target_time_sec: f32) -> f32 {
        if self.points.is_empty() { return (target_time_sec - self.song_offset_sec).max(0.0) / 0.5; }
        let mut accumulated_stop_duration = 0.0;
        let mut beat_at_last_event = 0.0;
        let mut time_at_last_event_pre_stop = self.song_offset_sec;
        let mut current_bpm = self.points[0].bpm;
        let mut events: Vec<(f32, Option<f32>, Option<f32>)> = Vec::new();
        for p in &self.points { events.push((p.beat, Some(p.bpm), None)); }
        for s in &self.stops_at_beat { events.push((s.0, None, Some(s.1))); }
        events.sort_by(|a,b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut unique_events: Vec<(f32, Option<f32>, Option<f32>)> = Vec::new();
        if !events.is_empty() {
            unique_events.push(events[0]);
            for i in 1..events.len() {
                if events[i].0 > events[i-1].0 { unique_events.push(events[i]); }
                else {
                    let last = unique_events.last_mut().unwrap();
                    if events[i].1.is_some() { last.1 = events[i].1; }
                    if events[i].2.is_some() { last.2 = events[i].2; }
                }
            }
        }
        for (event_beat, new_bpm_opt, stop_duration_opt) in &unique_events {
            let time_to_reach_event_beat = (event_beat - beat_at_last_event) * (60.0 / current_bpm);
            let time_at_event_beat_no_stops = time_at_last_event_pre_stop + time_to_reach_event_beat;
            if time_at_event_beat_no_stops + accumulated_stop_duration >= target_time_sec {
                let remaining_effective_time = target_time_sec - (time_at_last_event_pre_stop + accumulated_stop_duration);
                return beat_at_last_event + (remaining_effective_time / (60.0 / current_bpm));
            }
            if let Some(stop_dur) = stop_duration_opt { accumulated_stop_duration += stop_dur; }
            if let Some(new_bpm) = new_bpm_opt { current_bpm = *new_bpm; }
            beat_at_last_event = *event_beat;
            time_at_last_event_pre_stop = time_at_event_beat_no_stops;
        }
        let remaining_effective_time = target_time_sec - (time_at_last_event_pre_stop + accumulated_stop_duration);
        beat_at_last_event + (remaining_effective_time / (60.0 / current_bpm))
    }
}

// --- Initialization ---
pub fn initialize_game_state(
    win_w: f32,
    win_h: f32,
    audio_start_time: Instant, // This is when audio t=0 occurs (AFTER lead-in)
    song: Arc<SongInfo>,
    selected_chart_idx: usize,
) -> GameState {
    info!( "Initializing game state for song: '{}', chart index: {}", song.title, selected_chart_idx );

    let width_scale = win_w / config::GAMEPLAY_REF_WIDTH;
    let height_scale = win_h / config::GAMEPLAY_REF_HEIGHT;

    let desired_on_screen_width_for_layout = config::TARGET_VISUAL_SIZE_REF * width_scale;
    let desired_on_screen_height_for_layout = config::TARGET_VISUAL_SIZE_REF * height_scale;

    let arrow_lane_width = desired_on_screen_width_for_layout;
    let current_target_spacing = config::TARGET_SPACING_REF * width_scale;

    let current_target_top_margin = config::TARGET_TOP_MARGIN_REF * height_scale;
    let current_first_target_left_margin = config::FIRST_TARGET_LEFT_MARGIN_REF * width_scale;

    let target_center_y = current_target_top_margin + desired_on_screen_height_for_layout / 2.0;
    let first_lane_center_x = current_first_target_left_margin + arrow_lane_width / 2.0;

    let targets = ALL_ARROW_DIRECTIONS
        .iter()
        .enumerate()
        .map(|(i, &dir)| TargetInfo {
            x: first_lane_center_x + i as f32 * (arrow_lane_width + current_target_spacing),
            y: target_center_y,
            direction: dir,
        })
        .collect();

    let mut arrows_map = HashMap::new();
    for dir in ALL_ARROW_DIRECTIONS.iter() { arrows_map.insert(*dir, Vec::new()); }

    // Standard interpretation: positive file offset means beat 0 of the chart occurs LATER than the music's t=0.
    // So, TimingData.song_offset_sec should store this value directly.
    let effective_file_offset = -song.offset;

    let mut temp_timing_data = TimingData { song_offset_sec: effective_file_offset, ..Default::default() };
    let chart_info = &song.charts[selected_chart_idx];
    let mut combined_bpms = song.bpms_header.clone();
    if let Some(chart_bpms_str) = &chart_info.bpms_chart {
        if let Ok(chart_bpms_vec) = crate::parsing::simfile::parse_bpms(chart_bpms_str) {
            combined_bpms.extend(chart_bpms_vec);
        }
    }
    combined_bpms.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    combined_bpms.dedup_by_key(|k| k.0);

    if combined_bpms.is_empty() {
        warn!("No BPMs found for song {} chart {}, defaulting to 120 BPM at beat 0", song.title, selected_chart_idx);
        // Beat 0 occurs at effective_file_offset seconds into the audio track.
        temp_timing_data.points.push(BeatTimePoint { beat: 0.0, time_sec: effective_file_offset, bpm: 120.0 });
    } else {
        // current_time tracks the time *into the audio file* for each BPM point's beat.
        // It starts at the effective_file_offset, as that's the time of beat 0 (if no prior BPM changes).
        let mut current_time = effective_file_offset;
        let mut last_b_beat = 0.0;
        let mut last_b_bpm = combined_bpms[0].1;
        if combined_bpms[0].0 != 0.0 { // If the first BPM change isn't at beat 0
            // Add a point for beat 0; its time is the effective_file_offset.
            temp_timing_data.points.push(BeatTimePoint { beat: 0.0, time_sec: effective_file_offset, bpm: last_b_bpm});
        }
        for (beat, bpm) in &combined_bpms {
            if *beat < last_b_beat { continue; }
            if *beat > last_b_beat { current_time += (*beat - last_b_beat) * (60.0 / last_b_bpm); } // Accumulate time from last BPM point
            temp_timing_data.points.push(BeatTimePoint { beat: *beat, time_sec: current_time, bpm: *bpm });
            last_b_beat = *beat; last_b_bpm = *bpm;
        }
    }

    let mut combined_stops = song.stops_header.clone();
     if let Some(chart_stops_str) = &chart_info.stops_chart {
         if let Ok(chart_stops_vec) = crate::parsing::simfile::parse_stops(chart_stops_str) {
             combined_stops.extend(chart_stops_vec);
         }
     }
    combined_stops.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    for (beat, duration_simfile_value) in &combined_stops {
        let bpm_at_stop = temp_timing_data.points.iter().rfind(|p| p.beat <= *beat).map_or(120.0, |p| p.bpm);
        let duration_sec = duration_simfile_value * (60.0 / bpm_at_stop);
        temp_timing_data.stops_at_beat.push((*beat, duration_sec));
    }

    let processed_chart_data = chart_info.processed_data.as_ref().cloned().unwrap_or_else(|| {
        warn!("Chart {} for song {} has no processed data! Gameplay might be empty.", selected_chart_idx, song.title);
        ProcessedChartData::default()
    });

    // At the moment gameplay screen appears, the time relative to audio_start_time (which is in the future)
    // is -GAME_LEAD_IN_DURATION_SECONDS.
    // Note: audio_start_time itself ALREADY ACCOUNTS for GAME_LEAD_IN_DURATION_SECONDS.
    // So, when gameplay *starts visually*, current_raw_time_sec relative to audio_start_time (game_state.audio_start_time)
    // will effectively be -config::GAME_LEAD_IN_DURATION_SECONDS.
    // We need to find what chart beat corresponds to this "pre-audio-start" time.
    let time_at_visual_start_relative_to_audio_zero = -config::GAME_LEAD_IN_DURATION_SECONDS;
    let initial_actual_chart_beat = temp_timing_data.get_beat_for_time(time_at_visual_start_relative_to_audio_zero);

    // The display beat offset is based on the BPM around the *actual chart beat* we're calculating for.
    // For the initial display beat, this is the BPM at `initial_actual_chart_beat`.
    let bpm_at_initial_actual_chart_beat = temp_timing_data.points.iter()
        .rfind(|p| p.beat <= initial_actual_chart_beat) // Find BPM at or before this beat
        .map_or(120.0, |p| p.bpm); // Default to 120 if no points or all are later

    let display_beat_offset_due_to_audio_sync = if bpm_at_initial_actual_chart_beat > 0.0 {
        (config::AUDIO_SYNC_OFFSET_MS as f32 / 1000.0) / (60.0 / bpm_at_initial_actual_chart_beat)
    } else {
        0.0 // Avoid division by zero
    };

    let mut judgment_counts = HashMap::new();
    for judgment_type in ALL_JUDGMENTS.iter() {
        judgment_counts.insert(*judgment_type, 0);
    }


    GameState {
        targets,
        arrows: arrows_map,
        pressed_keys: HashSet::new(),
        // current_beat is the beat used for display purposes (arrow positions, hit checking against display)
        current_beat: initial_actual_chart_beat - display_beat_offset_due_to_audio_sync,
        current_chart_beat_actual: initial_actual_chart_beat,
        window_size: (win_w, win_h),
        active_explosions: HashMap::new(),
        audio_start_time: Some(audio_start_time),
        song_info: song,
        selected_chart_idx,
        timing_data: Arc::new(temp_timing_data),
        processed_chart: Arc::new(processed_chart_data),
        current_measure_idx: 0,
        current_line_in_measure_idx: 0,
        current_processed_beat: -1.0, // Start before any valid chart beat
        judgment_counts,
        lead_in_timer: config::GAME_LEAD_IN_DURATION_SECONDS,
        music_started: false,
    }
}

// --- Input Handling ---
pub fn handle_input(key_event: &KeyEvent, game_state: &mut GameState) -> Option<AppState> {
    if key_event.state == ElementState::Pressed && !key_event.repeat {
        if let Some(VirtualKeyCode::Escape) = crate::state::key_to_virtual_keycode(key_event.logical_key.clone()) {
            info!("Escape pressed in gameplay, returning to Select Music.");
            return Some(AppState::SelectMusic);
        }
    }

    if let Some(virtual_keycode) = crate::state::key_to_virtual_keycode(key_event.logical_key.clone()) {
        match virtual_keycode {
            VirtualKeyCode::Left | VirtualKeyCode::Down | VirtualKeyCode::Up | VirtualKeyCode::Right => match key_event.state {
                ElementState::Pressed => {
                    if game_state.pressed_keys.insert(virtual_keycode) && !key_event.repeat {
                        trace!("Gameplay Key Pressed: {:?}", virtual_keycode);
                        check_hits_on_press(game_state, virtual_keycode);
                    }
                }
                ElementState::Released => {
                    if game_state.pressed_keys.remove(&virtual_keycode) {
                        trace!("Gameplay Key Released: {:?}", virtual_keycode);
                    }
                }
            },
            _ => {}
        }
    }
    None
}

// --- Update Logic ---
pub fn update(game_state: &mut GameState, dt: f32, _rng: &mut impl Rng) {
    if !game_state.music_started && game_state.lead_in_timer > 0.0 {
        game_state.lead_in_timer -= dt;
        // Music will be started by App when lead_in_timer <= 0.0
    }

    if let Some(start_time) = game_state.audio_start_time {
        // `start_time` is when audio t=0 occurs.
        // `current_time_relative_to_audio_zero` can be negative during lead-in.
        let current_time_relative_to_audio_zero = if Instant::now() >= start_time {
            Instant::now().duration_since(start_time).as_secs_f32()
        } else {
            -(start_time.duration_since(Instant::now()).as_secs_f32())
        };

        game_state.current_chart_beat_actual = game_state.timing_data.get_beat_for_time(current_time_relative_to_audio_zero);

        // Calculate display beat offset based on current actual chart beat's BPM
        let bpm_at_current_actual_chart_beat = game_state.timing_data.points.iter()
            .rfind(|p| p.beat <= game_state.current_chart_beat_actual)
            .map_or(120.0, |p| p.bpm);

        let display_beat_offset_due_to_audio_sync = if bpm_at_current_actual_chart_beat > 0.0 {
            (config::AUDIO_SYNC_OFFSET_MS as f32 / 1000.0) / (60.0 / bpm_at_current_actual_chart_beat)
        } else {
            0.0 // Avoid division by zero
        };

        game_state.current_beat = game_state.current_chart_beat_actual - display_beat_offset_due_to_audio_sync;
        trace!("Time Rel. Audio 0: {:.3}s, LeadInTimer: {:.2}, Actual Chart Beat: {:.4}, Display Beat: {:.4}", current_time_relative_to_audio_zero, game_state.lead_in_timer, game_state.current_chart_beat_actual, game_state.current_beat);
    } else {
        warn!("Audio start time not set, cannot update beat accurately!");
    }

    spawn_arrows_from_chart(game_state);

    let current_arrow_speed = config::ARROW_SPEED * (game_state.window_size.1 / config::GAMEPLAY_REF_HEIGHT);

    let arrow_delta_y = current_arrow_speed * dt;
    for column_arrows in game_state.arrows.values_mut() {
        for arrow in column_arrows.iter_mut() {
            arrow.y -= arrow_delta_y;
        }
    }

    check_misses(game_state);

    let now = Instant::now();
    game_state.active_explosions.retain(|_dir, explosion| {
        now < explosion.end_time
    });
}

// --- Arrow Spawning Logic ---
fn spawn_arrows_from_chart(state: &mut GameState) {
    if state.processed_chart.measures.is_empty() { return; }
    let current_audio_time_chart_beat = state.current_chart_beat_actual;
    let lookahead_chart_beat_limit = current_audio_time_chart_beat + config::SPAWN_LOOKAHEAD_BEATS;

    loop {
        if state.current_measure_idx >= state.processed_chart.measures.len() { break; }
        let current_measure_data = &state.processed_chart.measures[state.current_measure_idx];
        if current_measure_data.is_empty() {
            state.current_measure_idx += 1; state.current_line_in_measure_idx = 0;
            trace!("Skipping empty measure at index {}", state.current_measure_idx -1);
            continue;
        }
        if state.current_line_in_measure_idx >= current_measure_data.len() {
            state.current_measure_idx += 1; state.current_line_in_measure_idx = 0;
            trace!("Advanced to measure {}", state.current_measure_idx);
            continue;
        }
        let num_lines_in_measure = current_measure_data.len() as f32;
        let measure_base_beat = state.current_measure_idx as f32 * 4.0;
        let beat_offset_in_measure = (state.current_line_in_measure_idx as f32 / num_lines_in_measure) * 4.0;
        let target_beat_for_line = measure_base_beat + beat_offset_in_measure;

        if target_beat_for_line <= state.current_processed_beat {
            state.current_line_in_measure_idx += 1; continue;
        }
        if target_beat_for_line > lookahead_chart_beat_limit { break; }

        let time_of_line_sec_audio_relative = state.timing_data.get_time_for_beat(target_beat_for_line);
        
        // Calculate current audio time relative to audio t=0
        let current_audio_time_sec_audio_relative = if let Some(start_time) = state.audio_start_time {
             if Instant::now() >= start_time {
                Instant::now().duration_since(start_time).as_secs_f32()
            } else {
                -(start_time.duration_since(Instant::now()).as_secs_f32())
            }
        } else { return; };

        let time_to_target_on_screen_sec = time_of_line_sec_audio_relative - current_audio_time_sec_audio_relative;


        if time_to_target_on_screen_sec < -0.05 { // Arrow is already past the target significantly
            trace!("Missed spawn window for line at chart_beat {:.2} (current audio time implies chart_beat {:.2})", target_beat_for_line, current_audio_time_chart_beat);
            state.current_processed_beat = target_beat_for_line;
            state.current_line_in_measure_idx += 1;
            continue;
        }

        let target_y_pos = state.targets.first().map_or(0.0, |t| t.y);
        let current_arrow_speed = config::ARROW_SPEED * (state.window_size.1 / config::GAMEPLAY_REF_HEIGHT);
        let distance_to_travel_pixels = current_arrow_speed * time_to_target_on_screen_sec.max(0.0);
        let spawn_y = target_y_pos + distance_to_travel_pixels;

        let note_line = &current_measure_data[state.current_line_in_measure_idx];
        let mut spawned_on_this_line = false;
        for (col_idx, &note_char) in note_line.iter().enumerate() {
            let direction = match col_idx { 0 => ArrowDirection::Left, 1 => ArrowDirection::Down, 2 => ArrowDirection::Up, 3 => ArrowDirection::Right, _ => continue, };
            let arrow_type_for_render = match note_char { NoteChar::Tap | NoteChar::HoldStart | NoteChar::RollStart => note_char, _ => NoteChar::Empty, };
            if arrow_type_for_render != NoteChar::Empty {
                let target_x_pos = state.targets.iter().find(|t| t.direction == direction).map_or(0.0, |t| t.x);
                if let Some(column_arrows) = state.arrows.get_mut(&direction) {
                    column_arrows.push(Arrow { x: target_x_pos, y: spawn_y, direction, note_char: arrow_type_for_render, target_beat: target_beat_for_line, });
                    spawned_on_this_line = true;
                }
            }
        }
        if spawned_on_this_line {
            // Get the time of the line again for logging, using the same method as for spawning.
            // This `time_of_line_sec_audio_relative` is the time from the audio file's t=0.
            let time_for_log = state.timing_data.get_time_for_beat(target_beat_for_line);
            debug!("Spawned notes for line at measure {}, line_idx {}, chart_beat {:.3}, audio_time {:.3}s",
                   state.current_measure_idx, state.current_line_in_measure_idx, target_beat_for_line, time_for_log);
        }
        state.current_processed_beat = target_beat_for_line;
        state.current_line_in_measure_idx += 1;
    }
}

// --- Hit Checking Logic ---
fn check_hits_on_press(state: &mut GameState, keycode: VirtualKeyCode) {
    let direction = match keycode {
        VirtualKeyCode::Left => Some(ArrowDirection::Left),
        VirtualKeyCode::Down => Some(ArrowDirection::Down),
        VirtualKeyCode::Up => Some(ArrowDirection::Up),
        VirtualKeyCode::Right => Some(ArrowDirection::Right),
        _ => None,
    };

    if let Some(dir) = direction {
        if let Some(column_arrows) = state.arrows.get_mut(&dir) {
            let current_display_beat = state.current_beat;
            let bpm_at_arrow_approx = state.timing_data.points.iter()
                .rfind(|p| p.beat <= current_display_beat)
                .map_or(120.0, |p| p.bpm);
            let seconds_per_beat_approx = if bpm_at_arrow_approx > 0.0 { 60.0 / bpm_at_arrow_approx } else { 0.5 };


            let mut best_hit_idx: Option<usize> = None;
            let mut min_abs_time_diff_ms = config::MAX_HIT_WINDOW_MS + 1.0;

            for (idx, arrow) in column_arrows.iter().enumerate() {
                let beat_diff = current_display_beat - arrow.target_beat;
                let time_diff_ms = beat_diff * seconds_per_beat_approx * 1000.0;
                let abs_time_diff_ms = time_diff_ms.abs();

                if abs_time_diff_ms <= config::MAX_HIT_WINDOW_MS && abs_time_diff_ms < min_abs_time_diff_ms {
                    min_abs_time_diff_ms = abs_time_diff_ms;
                    best_hit_idx = Some(idx);
                }
            }

            if let Some(idx) = best_hit_idx {
                let hit_arrow = &column_arrows[idx];
                let time_diff_for_log = (current_display_beat - hit_arrow.target_beat) * seconds_per_beat_approx * 1000.0;
                let note_char_for_log = hit_arrow.note_char;

                let judgment = if min_abs_time_diff_ms <= config::W1_WINDOW_MS { Judgment::W1 }
                               else if min_abs_time_diff_ms <= config::W2_WINDOW_MS { Judgment::W2 }
                               else if min_abs_time_diff_ms <= config::W3_WINDOW_MS { Judgment::W3 }
                               else if min_abs_time_diff_ms <= config::W4_WINDOW_MS { Judgment::W4 }
                               else { Judgment::W5 };

                *state.judgment_counts.entry(judgment).or_insert(0) += 1;

                info!( "HIT! {:?} {:?} ({:.1}ms) -> {:?} (Count: {})", dir, note_char_for_log, time_diff_for_log, judgment, state.judgment_counts[&judgment] );

                let explosion_end_time = Instant::now() + config::EXPLOSION_DURATION;
                state.active_explosions.insert(dir, ActiveExplosion {
                    judgment,
                    direction: dir,
                    end_time: explosion_end_time,
                });

                debug!(
                    "Added ActiveExplosion: dir={:?}, judgment={:?}, end_time will be in {:?}. Map size: {}",
                    dir, judgment, config::EXPLOSION_DURATION, state.active_explosions.len()
                );

                column_arrows.remove(idx);
            } else {
                debug!( "Input {:?} registered, but no arrow within {:.1}ms hit window (Display Beat: {:.2}).", keycode, config::MAX_HIT_WINDOW_MS, current_display_beat );
            }
        }
    }
}

// --- Miss Checking Logic ---
fn check_misses(state: &mut GameState) {
    let current_display_beat = state.current_beat;
    let mut missed_count_this_frame = 0;
    for (_dir, column_arrows) in state.arrows.iter_mut() {
        column_arrows.retain(|arrow| {
            let bpm_at_arrow_target = state.timing_data.points.iter().rfind(|p| p.beat <= arrow.target_beat).map_or(120.0, |p| p.bpm);
            let seconds_per_beat_at_target = if bpm_at_arrow_target > 0.0 { 60.0 / bpm_at_arrow_target } else { 0.5 };
            let miss_window_beats_dynamic = (config::MISS_WINDOW_MS / 1000.0) / seconds_per_beat_at_target;
            let beat_diff = current_display_beat - arrow.target_beat;
            if beat_diff > miss_window_beats_dynamic {
                *state.judgment_counts.entry(Judgment::Miss).or_insert(0) += 1;
                info!( "MISSED! {:?} {:?} (TgtBeat: {:.2}, DispBeat: {:.2}, DiffBeat: {:.2} > {:.2} ({:.1}ms)) (Miss Count: {})",
                       arrow.direction, arrow.note_char, arrow.target_beat, current_display_beat, beat_diff, miss_window_beats_dynamic, config::MISS_WINDOW_MS, state.judgment_counts[&Judgment::Miss] );
                missed_count_this_frame += 1;
                false
            } else { true }
        });
    }
    if missed_count_this_frame > 0 { trace!("Removed {} missed arrows.", missed_count_this_frame); }
}


fn draw_judgment_line(
    renderer: &Renderer,
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
    assets: &AssetManager,
    line_top_y: f32,
    initial_pen_x_for_digits: f32, // Renamed for clarity
    zero_scale: f32,
    label_scale: f32,
    label_text: &str,
    current_count: u32,
    dim_color: [f32; 4],
    bright_color: [f32; 4],
    height_scale: f32,
    width_scale: f32,
) {
    let wendy_font = assets.get_font(FontId::Wendy).unwrap();
    let miso_font = assets.get_font(FontId::Miso).unwrap();

    let mut current_digit_pen_x = initial_pen_x_for_digits;
    let wendy_zero_baseline_y = line_top_y + (wendy_font.metrics.ascender * zero_scale);

    let scaled_label_nudge = config::JUDGMENT_LABEL_VERTICAL_NUDGE_REF * height_scale;
    let miso_label_baseline_y = line_top_y + (miso_font.metrics.ascender * label_scale) + scaled_label_nudge;

    let general_digit_spacing = config::JUDGMENT_ZERO_SPACING_REF * width_scale;
    let digit_one_pre_extra_space = config::JUDGMENT_DIGIT_ONE_PRE_SPACE_REF * width_scale;
    let digit_one_post_extra_space = config::JUDGMENT_DIGIT_ONE_POST_SPACE_REF * width_scale;

    let count_str = format!("{:04}", current_count.min(9999));
    let count_chars: Vec<char> = count_str.chars().collect();

    let mut first_non_zero_found = false;
    let mut total_width_of_drawn_digits_and_their_spacing = 0.0;

    for (idx, digit_char) in count_chars.iter().enumerate() {
        let digit_str = digit_char.to_string();
        let mut actual_pre_spacing = 0.0;

        if *digit_char == '1' {
            actual_pre_spacing = digit_one_pre_extra_space;
        }

        let digit_width = wendy_font.measure_text_normalized(&digit_str) * zero_scale;

        let is_bright;
        if *digit_char == '0' && !first_non_zero_found && idx < count_chars.len() -1 {
             is_bright = false;
        } else {
            is_bright = true;
            if *digit_char != '0' {
                first_non_zero_found = true;
            }
        }
        let color = if is_bright { bright_color } else { dim_color };

        renderer.draw_text(
            device, cmd_buf, wendy_font, &digit_str,
            current_digit_pen_x + actual_pre_spacing, wendy_zero_baseline_y,
            color, zero_scale, None
        );

        let mut actual_post_spacing = general_digit_spacing;
        if *digit_char == '1' {
             actual_post_spacing = digit_one_post_extra_space;
        }

        let advance_for_this_digit_segment = actual_pre_spacing + digit_width + actual_post_spacing;
        current_digit_pen_x += advance_for_this_digit_segment;
        total_width_of_drawn_digits_and_their_spacing += advance_for_this_digit_segment;
    }

    // The label starts after the total width taken by digits and their specific spacing,
    // plus the general spacing between the digit block and the label.
    let label_start_x = initial_pen_x_for_digits + total_width_of_drawn_digits_and_their_spacing +
                        (config::JUDGMENT_ZERO_TO_LABEL_SPACING_REF * width_scale);

    renderer.draw_text(
        device, cmd_buf, miso_font, label_text,
        label_start_x, miso_label_baseline_y,
        bright_color, label_scale, None
    );
}


// --- Drawing Logic ---
pub fn draw(
    renderer: &Renderer,
    game_state: &GameState,
    assets: &AssetManager,
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
) {
    let (win_w, win_h) = renderer.window_size();
    let width_scale = win_w / config::GAMEPLAY_REF_WIDTH;
    let height_scale = win_h / config::GAMEPLAY_REF_HEIGHT;

    let desired_on_screen_width = config::TARGET_VISUAL_SIZE_REF * width_scale;
    let desired_on_screen_height = config::TARGET_VISUAL_SIZE_REF * height_scale;

    let desired_explosion_on_screen_width = desired_on_screen_width * config::EXPLOSION_SIZE_MULTIPLIER;
    let desired_explosion_on_screen_height = desired_on_screen_height * config::EXPLOSION_SIZE_MULTIPLIER;

    // --- Draw Gameplay Song Banner ---
    let banner_width = config::GAMEPLAY_BANNER_WIDTH_REF * width_scale;
    let banner_height = config::GAMEPLAY_BANNER_HEIGHT_REF * height_scale;
    let banner_right_margin = config::GAMEPLAY_BANNER_RIGHT_MARGIN_REF * width_scale;
    let banner_top_margin = config::GAMEPLAY_BANNER_TOP_MARGIN_REF * height_scale;

    let banner_pos_x = win_w - banner_right_margin - banner_width / 2.0;
    let banner_pos_y = banner_top_margin + banner_height / 2.0;

    renderer.draw_quad(
        device,
        cmd_buf,
        DescriptorSetId::DynamicBanner, // Use the dynamic banner set
        Vector3::new(banner_pos_x, banner_pos_y, 0.0),
        (banner_width, banner_height),
        Rad(0.0),
        [1.0, 1.0, 1.0, 1.0], // Full tint
        [0.0, 0.0],           // Default UV offset
        [1.0, 1.0],           // Default UV scale
    );


    // --- Draw Health Meter ---
    let health_meter_width = config::HEALTH_METER_WIDTH_REF * width_scale;
    let health_meter_height = config::HEALTH_METER_HEIGHT_REF * height_scale;
    let health_meter_border_thickness = config::HEALTH_METER_BORDER_THICKNESS_REF * width_scale.min(height_scale);

    let health_meter_outer_left_x = config::HEALTH_METER_LEFT_MARGIN_REF * width_scale;
    let health_meter_outer_top_y = config::HEALTH_METER_TOP_MARGIN_REF * height_scale;

    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::SolidColor,
        Vector3::new(
            health_meter_outer_left_x + health_meter_width / 2.0,
            health_meter_outer_top_y + health_meter_height / 2.0,
            0.0
        ),
        (health_meter_width, health_meter_height),
        Rad(0.0),
        config::HEALTH_METER_BORDER_COLOR,
        [0.0, 0.0], [1.0, 1.0]
    );

    let hm_inner_width = (health_meter_width - 2.0 * health_meter_border_thickness).max(0.0);
    let hm_inner_height = (health_meter_height - 2.0 * health_meter_border_thickness).max(0.0);
    let hm_inner_left_x = health_meter_outer_left_x + health_meter_border_thickness;
    let hm_inner_top_y = health_meter_outer_top_y + health_meter_border_thickness;

    if hm_inner_width > 0.0 && hm_inner_height > 0.0 {
        renderer.draw_quad(
            device, cmd_buf, DescriptorSetId::SolidColor,
            Vector3::new(
                hm_inner_left_x + hm_inner_width / 2.0,
                hm_inner_top_y + hm_inner_height / 2.0,
                0.0
            ),
            (hm_inner_width, hm_inner_height),
            Rad(0.0),
            config::HEALTH_METER_EMPTY_COLOR,
            [0.0, 0.0], [1.0, 1.0]
        );

        let current_health_percentage = 0.5;
        let hm_fill_width = (hm_inner_width * current_health_percentage).max(0.0);
        if hm_fill_width > 0.0 {
            renderer.draw_quad(
                device, cmd_buf, DescriptorSetId::SolidColor,
                Vector3::new(
                    hm_inner_left_x + hm_fill_width / 2.0,
                    hm_inner_top_y + hm_inner_height / 2.0,
                    0.0
                ),
                (hm_fill_width, hm_inner_height),
                Rad(0.0),
                config::HEALTH_METER_FILL_COLOR,
                [0.0, 0.0], [1.0, 1.0]
            );
        }
    }

    // --- Draw Song Duration Meter ---
    let duration_meter_width = config::DURATION_METER_WIDTH_REF * width_scale;
    let duration_meter_height = config::DURATION_METER_HEIGHT_REF * height_scale;
    let duration_meter_border_thickness = config::DURATION_METER_BORDER_THICKNESS_REF * width_scale.min(height_scale);

    let duration_meter_outer_left_x = config::DURATION_METER_LEFT_MARGIN_REF * width_scale;
    let duration_meter_outer_top_y = config::DURATION_METER_TOP_MARGIN_REF * height_scale;
    let duration_meter_outer_bottom_y = duration_meter_outer_top_y + duration_meter_height;


    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::SolidColor,
        Vector3::new(
            duration_meter_outer_left_x + duration_meter_width / 2.0,
            duration_meter_outer_top_y + duration_meter_height / 2.0,
            0.0
        ),
        (duration_meter_width, duration_meter_height),
        Rad(0.0),
        config::DURATION_METER_BORDER_COLOR,
        [0.0, 0.0], [1.0, 1.0]
    );

    let dm_inner_width = (duration_meter_width - 2.0 * duration_meter_border_thickness).max(0.0);
    let dm_inner_height = (duration_meter_height - 2.0 * duration_meter_border_thickness).max(0.0);
    let dm_inner_left_x = duration_meter_outer_left_x + duration_meter_border_thickness;
    let dm_inner_top_y = duration_meter_outer_top_y + duration_meter_border_thickness;

    if dm_inner_width > 0.0 && dm_inner_height > 0.0 {
        renderer.draw_quad(
            device, cmd_buf, DescriptorSetId::SolidColor,
            Vector3::new(
                dm_inner_left_x + dm_inner_width / 2.0,
                dm_inner_top_y + dm_inner_height / 2.0,
                0.0
            ),
            (dm_inner_width, dm_inner_height),
            Rad(0.0),
            config::DURATION_METER_EMPTY_COLOR,
            [0.0, 0.0], [1.0, 1.0]
        );

        let total_duration_sec = game_state.song_info.charts[game_state.selected_chart_idx]
            .calculated_length_sec.unwrap_or(0.0);

        let current_elapsed_song_time_sec = if total_duration_sec > 0.0 && game_state.audio_start_time.is_some() {
            (game_state.timing_data.get_time_for_beat(game_state.current_chart_beat_actual) - game_state.timing_data.song_offset_sec).max(0.0)
        } else {
            0.0
        };

        let progress_percentage = if total_duration_sec > 0.01 {
            (current_elapsed_song_time_sec / total_duration_sec).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let dm_fill_width = (dm_inner_width * progress_percentage).max(0.0);
        if dm_fill_width > 0.0 {
            renderer.draw_quad(
                device, cmd_buf, DescriptorSetId::SolidColor,
                Vector3::new(
                    dm_inner_left_x + dm_fill_width / 2.0,
                    dm_inner_top_y + dm_inner_height / 2.0,
                    0.0
                ),
                (dm_fill_width, dm_inner_height),
                Rad(0.0),
                config::DURATION_METER_FILL_COLOR,
                [0.0, 0.0], [1.0, 1.0]
            );
        }

        if let Some(font) = assets.get_font(FontId::Miso) {
            let song_title = &game_state.song_info.title;

            let target_text_visual_height = dm_inner_height * 0.80;
            let font_typographic_height_norm = (font.metrics.ascender - font.metrics.descender).max(1e-5);
            let text_scale = target_text_visual_height / font_typographic_height_norm;

            let text_width_pixels = font.measure_text_normalized(song_title) * text_scale;

            let text_x = dm_inner_left_x + (dm_inner_width - text_width_pixels) / 2.0;

            let mut text_baseline_y = (dm_inner_top_y + dm_inner_height / 2.0) - (font.metrics.ascender + font.metrics.descender) / 2.0 * text_scale;

            const TEXT_VERTICAL_NUDGE_REF_PX: f32 = 13.0;
            let current_text_vertical_nudge = TEXT_VERTICAL_NUDGE_REF_PX * height_scale;
            text_baseline_y += current_text_vertical_nudge;

            renderer.draw_text(
                device,
                cmd_buf,
                font,
                song_title,
                text_x.max(dm_inner_left_x),
                text_baseline_y,
                config::UI_BAR_TEXT_COLOR,
                text_scale,
                None
            );
        }
    }

    // --- Draw Judgment Counts ---
    if let (Some(wendy_font_ref), Some(miso_font_ref)) = (assets.get_font(FontId::Wendy), assets.get_font(FontId::Miso)) {
        let mut current_line_visual_top_y = duration_meter_outer_bottom_y + (config::JUDGMENT_TEXT_LINE_TOP_OFFSET_FROM_DURATION_METER_REF * height_scale);
        let judgment_start_x = config::JUDGMENT_ZERO_LEFT_START_OFFSET_REF * width_scale;

        let zero_target_visual_height = config::JUDGMENT_ZERO_VISUAL_HEIGHT_REF * height_scale;
        let wendy_font_typographic_height_norm = (wendy_font_ref.metrics.ascender - wendy_font_ref.metrics.descender).max(1e-5);
        let wendy_zero_scale = zero_target_visual_height / wendy_font_typographic_height_norm;

        let label_target_visual_height = config::JUDGMENT_LABEL_VISUAL_HEIGHT_REF * height_scale;
        let miso_font_typographic_height_norm = (miso_font_ref.metrics.ascender - miso_font_ref.metrics.descender).max(1e-5);
        let miso_label_scale = label_target_visual_height / miso_font_typographic_height_norm;

        let judgment_lines_data = [
            (Judgment::W1, "FANTASTIC", config::JUDGMENT_W1_DIM_COLOR, config::JUDGMENT_W1_BRIGHT_COLOR),
            (Judgment::W2, "PERFECT",   config::JUDGMENT_W2_DIM_COLOR, config::JUDGMENT_W2_BRIGHT_COLOR),
            (Judgment::W3, "GREAT",     config::JUDGMENT_W3_DIM_COLOR, config::JUDGMENT_W3_BRIGHT_COLOR),
            (Judgment::W4, "DECENT",    config::JUDGMENT_W4_DIM_COLOR, config::JUDGMENT_W4_BRIGHT_COLOR),
            (Judgment::W5, "WAY OFF",   config::JUDGMENT_W5_DIM_COLOR, config::JUDGMENT_W5_BRIGHT_COLOR),
            (Judgment::Miss,"MISS",      config::JUDGMENT_MISS_DIM_COLOR, config::JUDGMENT_MISS_BRIGHT_COLOR),
        ];

        let line_visual_height_for_spacing = zero_target_visual_height.max(label_target_visual_height);
        let vertical_spacing_between_lines = config::JUDGMENT_LINE_VERTICAL_SPACING_REF * height_scale;


        for (judgment_type, label, dim_color, bright_color) in judgment_lines_data.iter() {
            let count = game_state.judgment_counts.get(judgment_type).copied().unwrap_or(0);
            draw_judgment_line(
                renderer, device, cmd_buf, assets,
                current_line_visual_top_y, judgment_start_x,
                wendy_zero_scale, miso_label_scale,
                label, count, *dim_color, *bright_color,
                height_scale, width_scale
            );
            current_line_visual_top_y += line_visual_height_for_spacing + vertical_spacing_between_lines;
        }
    }


    // --- Draw Targets & Arrows ---
    let frame_index = ((game_state.current_beat * 2.0).floor().abs() as usize) % 4;
    let uv_width = 1.0 / 4.0;
    let uv_x_start = frame_index as f32 * uv_width;
    let base_uv_offset_arrows = [uv_x_start, 0.0];
    let base_uv_scale_arrows = [uv_width, 1.0];

    let target_uv_offset = [0.0, 0.0];
    let target_uv_scale = [0.25, 1.0];

    for target in &game_state.targets {
        let rotation_angle = match target.direction {
            ArrowDirection::Left => Rad(PI / 2.0),
            ArrowDirection::Down => Rad(0.0),
            ArrowDirection::Up => Rad(PI),
            ArrowDirection::Right => Rad(-PI / 2.0),
        };
        let quad_size_for_draw_quad = match target.direction {
            ArrowDirection::Up | ArrowDirection::Down => (desired_on_screen_width, desired_on_screen_height),
            ArrowDirection::Left | ArrowDirection::Right => (desired_on_screen_height, desired_on_screen_width),
        };
        renderer.draw_quad( device, cmd_buf, DescriptorSetId::Gameplay,
            Vector3::new(target.x, target.y, 0.0),
            quad_size_for_draw_quad,
            rotation_angle, config::TARGET_TINT,
            target_uv_offset,
            target_uv_scale,
        );
    }

    for (_direction, column_arrows) in &game_state.arrows {
        for arrow in column_arrows {
            let culling_margin_w = desired_on_screen_width;
            let culling_margin_h = desired_on_screen_height;
            if arrow.y < (0.0 - culling_margin_h) || arrow.y > (win_h + culling_margin_h) {
                continue;
            }
            let measure_idx_for_arrow = (arrow.target_beat / 4.0).floor() as usize;
            let mut arrow_tint = config::ARROW_TINT_OTHER;
            if measure_idx_for_arrow < game_state.processed_chart.measures.len() {
                let measure_data = &game_state.processed_chart.measures[measure_idx_for_arrow];
                let num_lines_in_measure = measure_data.len();
                if num_lines_in_measure > 0 {
                    let measure_base_beat = measure_idx_for_arrow as f32 * 4.0;
                    let beat_offset_from_measure_start = arrow.target_beat - measure_base_beat;
                    let line_index_in_measure_float = (beat_offset_from_measure_start / 4.0) * num_lines_in_measure as f32;
                    let line_index_in_measure = (line_index_in_measure_float + 0.001).round() as usize;
                    match num_lines_in_measure {
                        4 | 2 | 1 => { arrow_tint = config::ARROW_TINT_QUARTER; }
                        8 => { if line_index_in_measure % 2 == 0 { arrow_tint = config::ARROW_TINT_QUARTER; } else { arrow_tint = config::ARROW_TINT_EIGHTH; } }
                        16 => { if line_index_in_measure % 4 == 0 { arrow_tint = config::ARROW_TINT_QUARTER; } else if line_index_in_measure % 2 == 0 { arrow_tint = config::ARROW_TINT_EIGHTH; } else { arrow_tint = config::ARROW_TINT_SIXTEENTH; } }
                        12 => { if line_index_in_measure % 3 == 0 { arrow_tint = config::ARROW_TINT_QUARTER; } else { arrow_tint = config::ARROW_TINT_TWELFTH; } }
                        24 => { if line_index_in_measure % 6 == 0 { arrow_tint = config::ARROW_TINT_QUARTER; } else if line_index_in_measure % 3 == 0 { arrow_tint = config::ARROW_TINT_EIGHTH; } else { arrow_tint = config::ARROW_TINT_TWENTYFOURTH; } }
                        _ => {}
                    }
                }
            }
            let rotation_angle = match arrow.direction {
                ArrowDirection::Left => Rad(PI / 2.0), ArrowDirection::Down => Rad(0.0),
                ArrowDirection::Up => Rad(PI), ArrowDirection::Right => Rad(-PI / 2.0),
            };
            let quad_size_for_draw_quad = match arrow.direction {
                ArrowDirection::Up | ArrowDirection::Down => (desired_on_screen_width, desired_on_screen_height),
                ArrowDirection::Left | ArrowDirection::Right => (desired_on_screen_height, desired_on_screen_width),
            };
            renderer.draw_quad( device, cmd_buf, DescriptorSetId::Gameplay,
                Vector3::new(arrow.x, arrow.y, 0.0),
                quad_size_for_draw_quad,
                rotation_angle, arrow_tint,
                base_uv_offset_arrows, base_uv_scale_arrows,
            );
        }
    }

    // --- Draw Active Explosions ---
    let now = Instant::now();
    for (direction, explosion) in &game_state.active_explosions {
        if now < explosion.end_time {
            if let Some(explosion_set_id) = DescriptorSetId::from_judgment(explosion.judgment) {
                if let Some(target_info) = game_state.targets.iter().find(|t| t.direction == *direction) {
                    let explosion_rotation_angle = match target_info.direction {
                        ArrowDirection::Left => Rad(PI / 2.0),
                        ArrowDirection::Down => Rad(0.0),
                        ArrowDirection::Up => Rad(PI),
                        ArrowDirection::Right => Rad(-PI / 2.0),
                    };
                    let quad_size_for_draw_quad = match target_info.direction {
                        ArrowDirection::Up | ArrowDirection::Down => (desired_explosion_on_screen_width, desired_explosion_on_screen_height),
                        ArrowDirection::Left | ArrowDirection::Right => (desired_explosion_on_screen_height, desired_explosion_on_screen_width),
                    };
                    renderer.draw_quad(
                        device,
                        cmd_buf,
                        explosion_set_id,
                        Vector3::new(target_info.x, target_info.y, 0.0),
                        quad_size_for_draw_quad,
                        explosion_rotation_angle,
                        [1.0, 1.0, 1.0, 1.0],
                        [0.0, 0.0],
                        [1.0, 1.0],
                    );
                } else {
                    warn!(
                        "Draw: Could not find target_info for explosion direction {:?}",
                        *direction
                    );
                }
            }
        }
    }
}