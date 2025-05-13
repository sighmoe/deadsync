use crate::assets::{AssetManager, TextureId};
use crate::config;
use crate::graphics::renderer::{DescriptorSetId, Renderer};
use crate::parsing::simfile::{ChartInfo, NoteChar, ProcessedChartData, SongInfo}; // ADDED NoteChar, ProcessedChartData
use crate::state::{
    AppState, Arrow, ArrowDirection, FlashState, GameState, Judgment, TargetInfo, // NoteType might be replaced
    VirtualKeyCode, ALL_ARROW_DIRECTIONS,
};
use ash::vk;
use cgmath::{Rad, Vector3};
use log::{debug, error, info, trace, warn};
// Removed: use rand::prelude::IndexedRandom;
// Removed: use rand::{distr::Bernoulli, prelude::{Distribution, Rng}};
use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;
use std::sync::Arc; // To potentially share SongInfo/ChartInfo
use std::time::Instant;
use winit::event::{ElementState, KeyEvent};
use rand::Rng;

// --- Placeholder for TimingData (COPIED FROM ABOVE FOR THIS EXAMPLE) ---
#[derive(Debug, Clone, Default)]
pub struct BeatTimePoint {
    pub beat: f32,
    pub time_sec: f32,
    pub bpm: f32,
}
#[derive(Debug, Clone, Default)]
pub struct TimingData {
    pub points: Vec<BeatTimePoint>,
    pub stops_at_beat: Vec<(f32, f32)>,
    pub song_offset_sec: f32,
}
impl TimingData {
    pub fn get_time_for_beat(&self, target_beat: f32) -> f32 {
        if self.points.is_empty() { return self.song_offset_sec + target_beat * 0.5; }
        let mut current_time_sec = 0.0; // Time relative to song_offset_sec start
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

        // Create a sorted list of all events (BPM changes and stops)
        let mut events: Vec<(f32, Option<f32>, Option<f32>)> = Vec::new(); // (beat, new_bpm_opt, stop_duration_opt)
        for p in &self.points { events.push((p.beat, Some(p.bpm), None)); }
        for s in &self.stops_at_beat { events.push((s.0, None, Some(s.1))); }
        events.sort_by(|a,b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        
        let mut unique_events: Vec<(f32, Option<f32>, Option<f32>)> = Vec::new();
        if !events.is_empty() {
            unique_events.push(events[0]);
            for i in 1..events.len() {
                if events[i].0 > events[i-1].0 { // Only add if beat is different
                    unique_events.push(events[i]);
                } else { // Same beat, merge (prioritize BPM, then stop)
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
                // Target time falls before or at this event
                let remaining_effective_time = target_time_sec - (time_at_last_event_pre_stop + accumulated_stop_duration);
                return beat_at_last_event + (remaining_effective_time / (60.0 / current_bpm));
            }

            if let Some(stop_dur) = stop_duration_opt {
                accumulated_stop_duration += stop_dur;
            }
            if let Some(new_bpm) = new_bpm_opt {
                current_bpm = *new_bpm;
            }
            beat_at_last_event = *event_beat;
            time_at_last_event_pre_stop = time_at_event_beat_no_stops;
        }

        // Target time is after all defined events
        let remaining_effective_time = target_time_sec - (time_at_last_event_pre_stop + accumulated_stop_duration);
        beat_at_last_event + (remaining_effective_time / (60.0 / current_bpm))
    }
}
// --- End of Placeholder ---


// --- Initialization ---
pub fn initialize_game_state(
    win_w: f32,
    win_h: f32,
    audio_start_time: Instant,
    song: Arc<SongInfo>,      // ADDED
    selected_chart_idx: usize, // ADDED
) -> GameState {
    info!(
        "Initializing game state for song: '{}', chart index: {}",
        song.title, selected_chart_idx
    );
    let center_x = win_w / 2.0;
    let target_spacing = config::TARGET_SIZE * 1.2;
    let total_targets_width = (ALL_ARROW_DIRECTIONS.len() as f32 - 1.0) * target_spacing;
    let start_x_targets = center_x - total_targets_width / 2.0;

    let targets = ALL_ARROW_DIRECTIONS
        .iter()
        .enumerate()
        .map(|(i, &dir)| TargetInfo {
            x: start_x_targets + i as f32 * target_spacing,
            y: config::TARGET_Y_POS,
            direction: dir,
        })
        .collect();

    let mut arrows_map = HashMap::new();
    for dir in ALL_ARROW_DIRECTIONS.iter() {
        arrows_map.insert(*dir, Vec::new());
    }

    // --- TimingData and ProcessedChartData ---
    // This is a simplified placeholder for TimingData construction.
    // A full implementation would live in simfile.rs or timing.rs and be more robust.
    let mut temp_timing_data = TimingData {
        song_offset_sec: song.offset,
        ..Default::default()
    };

    let chart_info = &song.charts[selected_chart_idx];
    let mut combined_bpms = song.bpms_header.clone();
    if let Some(chart_bpms_str) = &chart_info.bpms_chart {
        if let Ok(chart_bpms_vec) = crate::parsing::simfile::parse_bpms(chart_bpms_str) {
            combined_bpms.extend(chart_bpms_vec);
        }
    }
    combined_bpms.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    combined_bpms.dedup_by_key(|k| k.0); // Keep first BPM at a given beat

    if combined_bpms.is_empty() { // Ensure there's at least one BPM, default to 120
        warn!("No BPMs found for song {} chart {}, defaulting to 120 BPM at beat 0", song.title, selected_chart_idx);
        temp_timing_data.points.push(BeatTimePoint { beat: 0.0, time_sec: song.offset, bpm: 120.0 });
    } else {
        let mut current_time = song.offset;
        let mut last_b_beat = 0.0;
        let mut last_b_bpm = combined_bpms[0].1; // BPM at beat 0
        if combined_bpms[0].0 != 0.0 { // If first BPM isn't at beat 0, add a point for beat 0
            temp_timing_data.points.push(BeatTimePoint { beat: 0.0, time_sec: song.offset, bpm: last_b_bpm});
        }

        for (beat, bpm) in &combined_bpms {
            if *beat < last_b_beat { continue; } // Should not happen if sorted
            if *beat > last_b_beat {
                current_time += (*beat - last_b_beat) * (60.0 / last_b_bpm);
            }
            temp_timing_data.points.push(BeatTimePoint { beat: *beat, time_sec: current_time, bpm: *bpm });
            last_b_beat = *beat;
            last_b_bpm = *bpm;
        }
    }
    
    // Simplified stops (assumes simfile stop values are already in seconds, which is NOT standard)
    // A proper implementation needs to convert simfile stop beat-values to seconds based on BPM at stop.
    let mut combined_stops = song.stops_header.clone();
     if let Some(chart_stops_str) = &chart_info.stops_chart {
         if let Ok(chart_stops_vec) = crate::parsing::simfile::parse_stops(chart_stops_str) {
             combined_stops.extend(chart_stops_vec);
         }
     }
    combined_stops.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    // For this placeholder, we directly use the second value as duration_sec
    // In a real system: duration_sec = stop_value_from_simfile * (60.0 / bpm_at_stop_beat)
    // And then TimingData.points[...].time_sec needs to be adjusted for all points *after* each stop.
    // This is non-trivial and best done in the parser.
    for (beat, duration_simfile_value) in &combined_stops {
        // Find BPM at stop_beat for duration calculation
        let bpm_at_stop = temp_timing_data.points.iter()
            .rfind(|p| p.beat <= *beat)
            .map_or(120.0, |p| p.bpm); // Default if no prior BPM
        let duration_sec = duration_simfile_value * (60.0 / bpm_at_stop);
        temp_timing_data.stops_at_beat.push((*beat, duration_sec));
    }
    // Critical: After calculating stops_at_beat with correct second durations,
    // iterate through temp_timing_data.points AGAIN. For each point, sum up all
    // stop_durations from stops_at_beat that occur *before* that point.beat,
    // and add that sum to point.time_sec. This makes point.time_sec the "true"
    // wall-clock time for that beat. This step is SKIPPED here for brevity.

    let processed_chart_data = chart_info
        .processed_data
        .as_ref()
        .cloned() // Clone it for GameState
        .unwrap_or_else(|| {
            warn!("Chart {} for song {} has no processed data! Gameplay might be empty.", selected_chart_idx, song.title);
            ProcessedChartData::default()
        });


    let initial_display_beat_offset = (config::AUDIO_SYNC_OFFSET_MS as f32 / 1000.0) / (60.0 / temp_timing_data.points[0].bpm);

    GameState {
        targets,
        arrows: arrows_map,
        pressed_keys: HashSet::new(),
        current_beat: -initial_display_beat_offset, // Display beat, starts negative due to offset
        window_size: (win_w, win_h),
        flash_states: HashMap::new(),
        audio_start_time: Some(audio_start_time),
        
        // New fields for chart-based gameplay
        song_info: song, // Store the Arc<SongInfo>
        selected_chart_idx,
        timing_data: Arc::new(temp_timing_data), // Store the Arc<TimingData>
        processed_chart: Arc::new(processed_chart_data), // Store Arc<ProcessedChartData>
        current_measure_idx: 0,
        current_line_in_measure_idx: 0,
        current_processed_beat: -1.0, // Start before beat 0 to ensure beat 0 notes can spawn
    }
}

// --- Input Handling --- (No changes needed for this part yet)
pub fn handle_input(key_event: &KeyEvent, game_state: &mut GameState) -> Option<AppState> {
    // ... (same as before) ...
    if key_event.state == ElementState::Pressed && !key_event.repeat {
        if let Some(VirtualKeyCode::Escape) =
            crate::state::key_to_virtual_keycode(key_event.logical_key.clone())
        {
            info!("Escape pressed in gameplay, returning to Select Music.");
            return Some(AppState::SelectMusic);
        }
    }

    if let Some(virtual_keycode) =
        crate::state::key_to_virtual_keycode(key_event.logical_key.clone())
    {
        match virtual_keycode {
            VirtualKeyCode::Left
            | VirtualKeyCode::Down
            | VirtualKeyCode::Up
            | VirtualKeyCode::Right => match key_event.state {
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
pub fn update(game_state: &mut GameState, dt: f32, _rng: &mut impl Rng) { // rng no longer needed for spawning
    if let Some(start_time) = game_state.audio_start_time {
        let current_raw_time_sec = Instant::now().duration_since(start_time).as_secs_f32();
        
        let chart_beat = game_state.timing_data.get_beat_for_time(current_raw_time_sec);

        // Calculate initial display beat offset based on the BPM at beat 0
        let initial_bpm_at_zero = game_state.timing_data.points.first().map_or(120.0, |p| p.bpm);
        let initial_display_beat_offset = (config::AUDIO_SYNC_OFFSET_MS as f32 / 1000.0) / (60.0 / initial_bpm_at_zero);
        
        game_state.current_beat = chart_beat - initial_display_beat_offset; // This is the "display" beat for judging
        trace!("Current Raw Time: {:.3}s, Chart Beat: {:.4}, Display Beat: {:.4}", current_raw_time_sec, chart_beat, game_state.current_beat);

    } else {
        warn!("Audio start time not set, cannot update beat accurately!");
        // Fallback or paused state update if needed
    }

    spawn_arrows_from_chart(game_state); // MODIFIED

    let arrow_delta_y = config::ARROW_SPEED * dt;
    for column_arrows in game_state.arrows.values_mut() {
        for arrow in column_arrows.iter_mut() {
            arrow.y -= arrow_delta_y;
        }
    }

    check_misses(game_state);

    let now = Instant::now();
    game_state
        .flash_states
        .retain(|_dir, flash| now < flash.end_time);
}

// --- Arrow Spawning Logic (Overhauled) ---
fn spawn_arrows_from_chart(state: &mut GameState) {
    if state.processed_chart.measures.is_empty() {
        return; // No chart data to process
    }

    // Determine the current chart time based on audio playback
    let current_audio_time_sec = if let Some(start_time) = state.audio_start_time {
        Instant::now().duration_since(start_time).as_secs_f32()
    } else {
        return; // Cannot determine current time
    };
    
    // The beat corresponding to current_audio_time_sec according to timing data
    // This is the "true" chart beat, not the display beat.
    let current_chart_beat_now = state.timing_data.get_beat_for_time(current_audio_time_sec);

    // Determine the latest beat we should consider spawning notes for
    let lookahead_chart_beat_limit = current_chart_beat_now + config::SPAWN_LOOKAHEAD_BEATS;

    loop { // Loop to process multiple lines if they fall into the spawn window
        if state.current_measure_idx >= state.processed_chart.measures.len() {
            break; // End of chart
        }

        let current_measure_data = &state.processed_chart.measures[state.current_measure_idx];
        if current_measure_data.is_empty() { // Should not happen with proper parsing
            state.current_measure_idx += 1;
            state.current_line_in_measure_idx = 0;
            trace!("Skipping empty measure at index {}", state.current_measure_idx -1);
            continue;
        }

        if state.current_line_in_measure_idx >= current_measure_data.len() {
            // Move to next measure
            state.current_measure_idx += 1;
            state.current_line_in_measure_idx = 0;
            trace!("Advanced to measure {}", state.current_measure_idx);
            continue; // Re-evaluate loop condition for end of chart
        }

        let num_lines_in_measure = current_measure_data.len() as f32;
        // Standard simfiles: 1 measure = 4 beats (e.g. in 4/4 time)
        let measure_base_beat = state.current_measure_idx as f32 * 4.0;
        let beat_offset_in_measure = (state.current_line_in_measure_idx as f32 / num_lines_in_measure) * 4.0;
        let target_beat_for_line = measure_base_beat + beat_offset_in_measure;

        if target_beat_for_line <= state.current_processed_beat {
             // This line was already processed or is from the past, advance to next line
            state.current_line_in_measure_idx += 1;
            continue;
        }

        if target_beat_for_line > lookahead_chart_beat_limit {
            break; // This line is too far in the future to spawn yet
        }
        
        // If we're here, the line is within the spawn window and hasn't been processed.
        let note_line = &current_measure_data[state.current_line_in_measure_idx];

        // Calculate time properties for spawning
        let time_of_line_sec = state.timing_data.get_time_for_beat(target_beat_for_line);
        let time_to_target_on_screen_sec = time_of_line_sec - current_audio_time_sec;

        if time_to_target_on_screen_sec < 0.0 { // Line is in the past relative to audio
            // Mark as missed spawn, advance, and log
            trace!("Missed spawn window for line at beat {:.2} (current audio time corresponds to beat {:.2})", target_beat_for_line, current_chart_beat_now);
            state.current_processed_beat = target_beat_for_line;
            state.current_line_in_measure_idx += 1;
            continue;
        }
        
        // Calculate spawn Y position
        // Note: ARROW_SPEED is pixels per second.
        let distance_to_travel_pixels = config::ARROW_SPEED * time_to_target_on_screen_sec;
        let spawn_y = config::TARGET_Y_POS + distance_to_travel_pixels;

        // Spawn arrows for this line
        let mut spawned_on_this_line = false;
        for (col_idx, &note_char) in note_line.iter().enumerate() {
            let direction = match col_idx {
                0 => ArrowDirection::Left, 1 => ArrowDirection::Down,
                2 => ArrowDirection::Up,   3 => ArrowDirection::Right,
                _ => continue, // Should not happen for a 4-char line
            };

            // Determine ArrowType (Tap, HoldStart, etc.)
            let arrow_type_for_render = match note_char {
                NoteChar::Tap | NoteChar::HoldStart | NoteChar::RollStart => note_char,
                _ => NoteChar::Empty, // Only spawn visual arrows for these for now
            };

            if arrow_type_for_render != NoteChar::Empty {
                let target_x_pos = state.targets.iter()
                    .find(|t| t.direction == direction)
                    .map_or(0.0, |t| t.x);

                if let Some(column_arrows) = state.arrows.get_mut(&direction) {
                    column_arrows.push(Arrow {
                        x: target_x_pos,
                        y: spawn_y,
                        direction,
                        note_char: arrow_type_for_render, // Store the NoteChar
                        target_beat: target_beat_for_line,
                    });
                    spawned_on_this_line = true;
                    // trace!(
                    //     "Spawned {:?} {:?} at y={:.1} (target_y={:.1}), target_beat={:.2} (line time: {:.2}s, current audio time: {:.2}s, diff: {:.2}s)",
                    //     direction, arrow_type_for_render, spawn_y, config::TARGET_Y_POS, target_beat_for_line, time_of_line_sec, current_audio_time_sec, time_to_target_on_screen_sec
                    // );
                }
            }
        }
        if spawned_on_this_line {
            debug!("Spawned notes for line at measure {}, line_idx {}, chart_beat {:.3}", state.current_measure_idx, state.current_line_in_measure_idx, target_beat_for_line);
        }


        state.current_processed_beat = target_beat_for_line; // Mark this beat as processed
        state.current_line_in_measure_idx += 1; // Advance to next line for next iteration/frame
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
            // current_beat is the "display beat", adjusted for AUDIO_SYNC_OFFSET_MS
            // arrow.target_beat is the "chart beat"
            let current_display_beat = state.current_beat;

            // Convert display beat to absolute time for fair comparison with target beat's time
            // This is complex if AUDIO_SYNC_OFFSET_MS implies a shift in how BPMs/Stops apply.
            // For now, let's assume current_display_beat can be directly compared against arrow.target_beat
            // after converting both to time or by finding time diff.

            // The BPM at the current display beat (or near arrow.target_beat)
            // This is an approximation, a more accurate way would be to use time differences.
            let bpm_at_arrow_approx = state.timing_data.points.iter()
                .rfind(|p| p.beat <= current_display_beat) // Use display beat as reference
                .map_or(120.0, |p| p.bpm);
            let seconds_per_beat_approx = 60.0 / bpm_at_arrow_approx;


            let mut best_hit_idx: Option<usize> = None;
            let mut min_abs_time_diff_ms = config::MAX_HIT_WINDOW_MS + 1.0;

            for (idx, arrow) in column_arrows.iter().enumerate() {
                let beat_diff = current_display_beat - arrow.target_beat;
                let time_diff_ms = beat_diff * seconds_per_beat_approx * 1000.0;
                let abs_time_diff_ms = time_diff_ms.abs();

                if abs_time_diff_ms <= config::MAX_HIT_WINDOW_MS
                    && abs_time_diff_ms < min_abs_time_diff_ms
                {
                    min_abs_time_diff_ms = abs_time_diff_ms;
                    best_hit_idx = Some(idx);
                }
            }

            if let Some(idx) = best_hit_idx {
                let hit_arrow = &column_arrows[idx]; // Keep borrow valid
                let time_diff_for_log = (current_display_beat - hit_arrow.target_beat) * seconds_per_beat_approx * 1000.0;
                let note_char_for_log = hit_arrow.note_char; // Use note_char

                let judgment = if min_abs_time_diff_ms <= config::W1_WINDOW_MS { Judgment::W1 }
                               else if min_abs_time_diff_ms <= config::W2_WINDOW_MS { Judgment::W2 }
                               else if min_abs_time_diff_ms <= config::W3_WINDOW_MS { Judgment::W3 }
                               else if min_abs_time_diff_ms <= config::W4_WINDOW_MS { Judgment::W4 }
                               else { Judgment::W4 }; // Should be covered by MAX_HIT_WINDOW_MS

                info!( "HIT! {:?} {:?} ({:.1}ms) -> {:?}", dir, note_char_for_log, time_diff_for_log, judgment );

                let flash_color = match judgment {
                    Judgment::W1 => config::FLASH_COLOR_W1, Judgment::W2 => config::FLASH_COLOR_W2,
                    Judgment::W3 => config::FLASH_COLOR_W3, Judgment::W4 => config::FLASH_COLOR_W4,
                    Judgment::Miss => unreachable!(), // Not applicable for hits
                };
                state.flash_states.insert(dir, FlashState { color: flash_color, end_time: Instant::now() + config::FLASH_DURATION });
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
    let mut missed_count = 0;

    for (_dir, column_arrows) in state.arrows.iter_mut() {
        column_arrows.retain(|arrow| {
            // Approximate BPM at arrow's target beat for miss window calculation
            let bpm_at_arrow_target = state.timing_data.points.iter()
                .rfind(|p| p.beat <= arrow.target_beat)
                .map_or(120.0, |p| p.bpm);
            let seconds_per_beat_at_target = 60.0 / bpm_at_arrow_target;
            let miss_window_beats_dynamic = (config::MISS_WINDOW_MS / 1000.0) / seconds_per_beat_at_target;

            let beat_diff = current_display_beat - arrow.target_beat; // Player is past arrow's target
            if beat_diff > miss_window_beats_dynamic {
                info!(
                    "MISSED! {:?} {:?} (TgtBeat: {:.2}, DispBeat: {:.2}, DiffBeat: {:.2} > {:.2} ({:.1}ms))",
                    arrow.direction, arrow.note_char, arrow.target_beat, current_display_beat,
                    beat_diff, miss_window_beats_dynamic, config::MISS_WINDOW_MS
                );
                missed_count += 1;
                false // Remove arrow
            } else {
                true // Keep arrow
            }
        });
    }
    if missed_count > 0 {
        trace!("Removed {} missed arrows.", missed_count);
    }
}


// --- Drawing Logic ---
pub fn draw(
    renderer: &Renderer,
    game_state: &GameState,
    assets: &AssetManager,
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
) {
    let _arrow_texture = assets.get_texture(TextureId::Arrows).expect("Arrow texture missing");
    let now = Instant::now();

    // Pulsing animation for targets/arrows based on display beat
    let frame_index = ((game_state.current_beat * 2.0).floor().abs() as usize) % 4;
    let uv_width = 1.0 / 4.0;
    let uv_x_start = frame_index as f32 * uv_width;
    let base_uv_offset = [uv_x_start, 0.0];
    let base_uv_scale = [uv_width, 1.0];

    // --- Draw Targets ---
    for target in &game_state.targets {
        let current_tint = game_state.flash_states.get(&target.direction)
            .filter(|flash| now < flash.end_time)
            .map_or(config::TARGET_TINT, |flash| flash.color);
        let rotation_angle = match target.direction {
            ArrowDirection::Left => Rad(PI / 2.0), ArrowDirection::Down => Rad(0.0),
            ArrowDirection::Up => Rad(PI), ArrowDirection::Right => Rad(-PI / 2.0),
        };
        renderer.draw_quad( device, cmd_buf, DescriptorSetId::Gameplay,
            Vector3::new(target.x, target.y, 0.0),
            (config::TARGET_SIZE, config::TARGET_SIZE), rotation_angle, current_tint,
            base_uv_offset, base_uv_scale,
        );
    }

    // --- Draw Arrows ---
    for column_arrows in game_state.arrows.values() {
        for arrow in column_arrows {
            if arrow.y < (0.0 - config::ARROW_SIZE) || arrow.y > (game_state.window_size.1 + config::ARROW_SIZE) {
                continue;
            }

            // Determine tint based on arrow.note_char (which was NoteType before)
            let arrow_tint = match arrow.note_char {
                // For now, map Tap, HoldStart, RollStart to existing Quarter/Eighth/Sixteenth tints
                // This needs refinement if visual distinction is desired.
                // A simple way for now is to use target_beat quantization for color.
                NoteChar::Tap | NoteChar::HoldStart | NoteChar::RollStart => {
                    let beat_fraction = arrow.target_beat.fract();
                    // This is a rough approximation for coloring
                    if (beat_fraction * 4.0).fract() < 0.01 { // Quarter
                        config::ARROW_TINT_QUARTER
                    } else if (beat_fraction * 8.0).fract() < 0.01 { // Eighth
                        config::ARROW_TINT_EIGHTH
                    } else { // Sixteenth (or other)
                        config::ARROW_TINT_SIXTEENTH
                    }
                }
                _ => [0.5, 0.5, 0.5, 0.5], // Default for unhandled NoteChar
            };

            let rotation_angle = match arrow.direction {
                ArrowDirection::Left => Rad(PI / 2.0), ArrowDirection::Down => Rad(0.0),
                ArrowDirection::Up => Rad(PI), ArrowDirection::Right => Rad(-PI / 2.0),
            };
            renderer.draw_quad( device, cmd_buf, DescriptorSetId::Gameplay,
                Vector3::new(arrow.x, arrow.y, 0.0),
                (config::ARROW_SIZE, config::ARROW_SIZE), rotation_angle, arrow_tint,
                base_uv_offset, base_uv_scale,
            );
        }
    }
}