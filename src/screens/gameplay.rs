use crate::assets::{AssetManager, FontId, TextureId};
use crate::config;
use crate::graphics::renderer::{DescriptorSetId, Renderer};
use crate::parsing::simfile::{ChartInfo, NoteChar, ProcessedChartData, SongInfo};
use crate::state::{
    ActiveExplosion, AppState, Arrow, ArrowDirection, GameState, Judgment, TargetInfo,
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


// --- TimingData struct and its impl ---
#[derive(Debug, Clone, Default)]
pub struct BeatTimePoint {
    pub beat: f32,
    pub time_sec: f32,
    pub bpm: f32,
}
#[derive(Debug, Clone, Default)]
pub struct TimingData {
    pub points: Vec<BeatTimePoint>,
    pub stops_at_beat: Vec<(f32, f32)>, // (beat, duration_in_seconds)
    pub song_offset_sec: f32,
}

impl TimingData {
    pub fn get_time_for_beat(&self, target_beat: f32) -> f32 {
        if self.points.is_empty() {
            return self.song_offset_sec + target_beat * (60.0 / 120.0); // Default to 120 BPM
        }

        let mut current_time_sec = 0.0;
        let mut last_beat = 0.0;
        
        // Determine the BPM at beat 0 if no explicit point exists
        let mut last_bpm = self.points.first()
            .map_or(120.0, |p| if p.beat == 0.0 { p.bpm } else {
                // If first BPM change is after beat 0, assume the song starts with that BPM
                // or a default if the list is empty (handled above).
                self.points.first().map_or(120.0, |fp| fp.bpm)
            });

        // If the first BPM change isn't at beat 0, add time for the segment from beat 0
        // to the first BPM change using the determined initial BPM.
        // This logic is implicitly handled by the loop if points are correctly ordered and start from beat 0.
        // It's crucial that self.points contains a point at beat 0.0 if the first #BPM tag is not at 0.0.
        // Let's assume `initialize_game_state` ensures this.

        for point in &self.points {
            if point.beat <= last_beat { // Handle multiple events at the same beat (take last BPM)
                last_bpm = point.bpm;
                continue;
            }
            if target_beat >= point.beat { // If target_beat is past or at this BPM change
                if last_bpm > 0.0 {
                    current_time_sec += (point.beat - last_beat) * (60.0 / last_bpm);
                }
                last_beat = point.beat;
                last_bpm = point.bpm;
            } else { // Target is within the segment from last_beat to point.beat
                if last_bpm > 0.0 {
                    current_time_sec += (target_beat - last_beat) * (60.0 / last_bpm);
                }
                last_beat = target_beat; // We've reached the target beat
                break;
            }
        }
        // If target_beat is after all BPM points in the list
        if target_beat > last_beat && last_bpm > 0.0 {
            current_time_sec += (target_beat - last_beat) * (60.0 / last_bpm);
        }

        let mut time_with_bpms = self.song_offset_sec + current_time_sec;

        // Add stop durations that occurred *before* the target_beat
        for (stop_beat, stop_duration_sec) in &self.stops_at_beat {
            if *stop_beat < target_beat {
                time_with_bpms += stop_duration_sec;
            }
        }
        time_with_bpms
    }

    pub fn get_beat_for_time(&self, target_time_sec: f32) -> f32 {
        if self.points.is_empty() {
             return (target_time_sec - self.song_offset_sec).max(0.0) / (60.0 / 120.0); // Default 120 BPM
        }

        let mut accumulated_stop_duration_at_event = 0.0;
        let mut beat_at_last_event = 0.0;
        let mut time_at_last_event_excluding_stops = self.song_offset_sec; // Time of beat 0 (effective start)
        
        let mut events: Vec<(f32, Option<f32>, Option<f32>)> = Vec::new(); // (beat, Option<new_bpm>, Option<stop_duration_sec>)
        for p in &self.points { events.push((p.beat, Some(p.bpm), None)); }
        for s in &self.stops_at_beat { events.push((s.0, None, Some(s.1))); }
        
        // Sort events by beat, then by type (BPM changes processed before stops at same beat)
        events.sort_by(|a,b| {
            a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                match (a.1.is_some(), b.1.is_some()) { // BPM first
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => std::cmp::Ordering::Equal,
                }
            })
        });
        
        let mut unique_events: Vec<(f32, Option<f32>, Option<f32>)> = Vec::new();
        if !events.is_empty() {
            unique_events.push(events[0]);
            for i in 1..events.len() {
                if (events[i].0 - events[i-1].0).abs() > 0.0001 { 
                    unique_events.push(events[i]);
                } else { 
                    let last = unique_events.last_mut().unwrap();
                    if events[i].1.is_some() { last.1 = events[i].1; } 
                    if events[i].2.is_some() && last.2.is_none() { last.2 = events[i].2; } 
                }
            }
        }

        let mut current_bpm = self.points.first().map_or(120.0, |p| p.bpm); 

        for (event_beat, new_bpm_opt, stop_duration_sec_opt) in &unique_events {
            let time_segment_duration = if current_bpm > 0.0 {
                (*event_beat - beat_at_last_event) * (60.0 / current_bpm)
            } else {
                if *event_beat > beat_at_last_event { f32::INFINITY } else { 0.0 }
            };

            let time_at_event_beat_excluding_stops_current_segment = time_at_last_event_excluding_stops + time_segment_duration;
            let time_at_event_beat_including_prior_stops = time_at_event_beat_excluding_stops_current_segment + accumulated_stop_duration_at_event;

            if time_at_event_beat_including_prior_stops >= target_time_sec {
                let time_into_segment_effective = target_time_sec - (time_at_last_event_excluding_stops + accumulated_stop_duration_at_event);
                return beat_at_last_event + if current_bpm > 0.0 {
                    (time_into_segment_effective / (60.0 / current_bpm)).max(0.0)
                } else {
                    0.0 
                };
            }
            
            time_at_last_event_excluding_stops = time_at_event_beat_excluding_stops_current_segment;
            beat_at_last_event = *event_beat;
            if let Some(new_bpm) = new_bpm_opt { current_bpm = *new_bpm; }
            if let Some(stop_dur) = stop_duration_sec_opt { accumulated_stop_duration_at_event += stop_dur; }
        }

        let time_after_last_event_effective = target_time_sec - (time_at_last_event_excluding_stops + accumulated_stop_duration_at_event);
        beat_at_last_event + if current_bpm > 0.0 {
            (time_after_last_event_effective / (60.0 / current_bpm)).max(0.0)
        } else {
            0.0
        }
    }
}

// Gameplay constants for rendering window
const MAX_DRAW_BEATS_FORWARD: f32 = 12.0; // How many actual chart beats ahead to consider
const MAX_DRAW_BEATS_BACK: f32 = 3.0;   // How many actual chart beats behind to consider

pub fn initialize_game_state(
    win_w: f32,
    win_h: f32,
    audio_start_time: Instant,
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
    
    let effective_file_offset = -song.offset; // Simfile offset: positive means music starts later / chart starts earlier

    let mut temp_timing_data = TimingData { song_offset_sec: effective_file_offset, ..Default::default() };
    let chart_info = &song.charts[selected_chart_idx];
    let mut combined_bpms = song.bpms_header.clone();
    if let Some(chart_bpms_str) = &chart_info.bpms_chart {
        if let Ok(chart_bpms_vec) = crate::parsing::simfile::parse_bpms(chart_bpms_str) {
            if !chart_bpms_vec.is_empty() { // Only use chart BPMs if they exist
                combined_bpms = chart_bpms_vec; // Chart BPMs override song BPMs
            }
        }
    }
    combined_bpms.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    combined_bpms.dedup_by_key(|k| k.0);

    if combined_bpms.is_empty() || combined_bpms[0].0 != 0.0 {
        // Ensure there's a BPM at beat 0. If the first defined BPM is later,
        // use its value for beat 0. If no BPMs, default to 120.
        let initial_bpm = combined_bpms.first().map_or(120.0, |p| p.1);
        if combined_bpms.first().map_or(true, |p| p.0 != 0.0) {
             combined_bpms.insert(0, (0.0, initial_bpm));
        }
    }
    
    // Populate TimingData.points (time_sec is relative to song_offset_sec)
    let mut current_calc_time_from_offset = 0.0; // Time elapsed since effective_file_offset due to BPMs
    let mut last_calc_beat = 0.0;
    let mut last_calc_bpm = combined_bpms[0].1; // BPM at last_calc_beat

    for (beat, bpm_val) in &combined_bpms {
        if *beat < last_calc_beat { continue; } // Should be sorted
        if *beat > last_calc_beat { // If there's a gap, calculate time for that segment
            if last_calc_bpm > 0.0 {
                current_calc_time_from_offset += (*beat - last_calc_beat) * (60.0 / last_calc_bpm);
            }
        }
        temp_timing_data.points.push(BeatTimePoint {
            beat: *beat,
            time_sec: effective_file_offset + current_calc_time_from_offset, // Absolute time in audio file
            bpm: *bpm_val,
        });
        last_calc_beat = *beat;
        last_calc_bpm = *bpm_val;
    }


    let mut combined_stops = song.stops_header.clone();
     if let Some(chart_stops_str) = &chart_info.stops_chart {
         if let Ok(chart_stops_vec) = crate::parsing::simfile::parse_stops(chart_stops_str) {
             if !chart_stops_vec.is_empty() { // Chart stops override song stops
                combined_stops = chart_stops_vec;
             }
         }
     }
    combined_stops.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    combined_stops.dedup_by_key(|k| k.0); 

    for (beat, duration_simfile_value) in &combined_stops {
        let bpm_at_stop_start = temp_timing_data.points.iter()
            .rfind(|p| p.beat <= *beat) 
            .map_or_else(
                || temp_timing_data.points.first().map_or(120.0, |p| p.bpm), 
                |p| p.bpm
            );

        if bpm_at_stop_start <= 0.0 {
            warn!("Stop at beat {} has invalid BPM ({}), duration might be incorrect.", beat, bpm_at_stop_start);
            temp_timing_data.stops_at_beat.push((*beat, *duration_simfile_value));
        } else {
            let duration_sec = duration_simfile_value * (60.0 / bpm_at_stop_start);
            temp_timing_data.stops_at_beat.push((*beat, duration_sec));
        }
    }


    let processed_chart_data = chart_info.processed_data.as_ref().cloned().unwrap_or_else(|| {
        warn!("Chart {} for song {} has no processed data! Gameplay might be empty.", selected_chart_idx, song.title);
        ProcessedChartData::default()
    });

    let time_at_visual_start_relative_to_audio_zero = -config::GAME_LEAD_IN_DURATION_SECONDS;
    let initial_actual_chart_beat = temp_timing_data.get_beat_for_time(time_at_visual_start_relative_to_audio_zero);

    let bpm_at_initial_actual_chart_beat = temp_timing_data.points.iter()
        .rfind(|p| p.beat <= initial_actual_chart_beat)
        .map_or(120.0, |p| p.bpm);

    let display_beat_offset_due_to_audio_sync = if bpm_at_initial_actual_chart_beat > 0.0 {
        (config::AUDIO_SYNC_OFFSET_MS as f32 / 1000.0) / (60.0 / bpm_at_initial_actual_chart_beat)
    } else {
        0.0
    };
    
    let mut judgment_counts = HashMap::new();
    for judgment_type in ALL_JUDGMENTS.iter() {
        judgment_counts.insert(*judgment_type, 0);
    }

    GameState {
        targets,
        arrows: arrows_map,
        pressed_keys: HashSet::new(),
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
        current_processed_beat: -1.0, 
        judgment_counts,
        lead_in_timer: config::GAME_LEAD_IN_DURATION_SECONDS,
        music_started: false,
    }
}

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


pub fn update(game_state: &mut GameState, dt: f32, _rng: &mut impl Rng) {
    if !game_state.music_started && game_state.lead_in_timer > 0.0 {
        game_state.lead_in_timer -= dt;
    }

    if let Some(start_time) = game_state.audio_start_time {
        let current_time_relative_to_audio_zero = if Instant::now() >= start_time {
            Instant::now().duration_since(start_time).as_secs_f32()
        } else {
            -(start_time.duration_since(Instant::now()).as_secs_f32())
        };

        game_state.current_chart_beat_actual = game_state.timing_data.get_beat_for_time(current_time_relative_to_audio_zero);

        let bpm_at_current_actual_chart_beat = game_state.timing_data.points.iter()
            .rfind(|p| p.beat <= game_state.current_chart_beat_actual)
            .map_or(120.0, |p| p.bpm);

        let display_beat_offset_due_to_audio_sync = if bpm_at_current_actual_chart_beat > 0.0 {
            (config::AUDIO_SYNC_OFFSET_MS as f32 / 1000.0) / (60.0 / bpm_at_current_actual_chart_beat)
        } else {
            0.0
        };
        game_state.current_beat = game_state.current_chart_beat_actual - display_beat_offset_due_to_audio_sync;
    }

    spawn_arrows_from_chart(game_state);

    let current_display_beat = game_state.current_beat;
    let current_display_time_sec = game_state.timing_data.get_time_for_beat(current_display_beat);
    let target_receptor_y = game_state.targets.first().map_or(0.0, |t| t.y);
    let px_per_sec_scroll_speed = config::ARROW_SPEED * (game_state.window_size.1 / config::GAMEPLAY_REF_HEIGHT);

    for (_direction, column_arrows) in game_state.arrows.iter_mut() {
        for arrow in column_arrows.iter_mut() {
            let arrow_target_display_time_sec = game_state.timing_data.get_time_for_beat(arrow.target_beat);
            let time_difference_to_display_target_sec = arrow_target_display_time_sec - current_display_time_sec;
            arrow.y = target_receptor_y + time_difference_to_display_target_sec * px_per_sec_scroll_speed;
        }
    }

    check_misses(game_state);

    let now = Instant::now();
    game_state.active_explosions.retain(|_dir, explosion| now < explosion.end_time);
}

fn spawn_arrows_from_chart(state: &mut GameState) {
    if state.processed_chart.measures.is_empty() { return; }

    let first_actual_chart_beat_to_render = state.current_chart_beat_actual - MAX_DRAW_BEATS_BACK;
    let last_actual_chart_beat_to_render = state.current_chart_beat_actual + MAX_DRAW_BEATS_FORWARD;
    
    let bpm_at_current_actual_chart_beat = state.timing_data.points.iter()
        .rfind(|p| p.beat <= state.current_chart_beat_actual)
        .map_or(120.0, |p| p.bpm);
    let display_beat_offset_due_to_audio_sync = if bpm_at_current_actual_chart_beat > 0.0 {
        (config::AUDIO_SYNC_OFFSET_MS as f32 / 1000.0) / (60.0 / bpm_at_current_actual_chart_beat)
    } else {
        0.0
    };

    let mut measure_loop_start_idx = state.current_measure_idx;
    let estimated_measure_for_first_render_beat = (first_actual_chart_beat_to_render / 4.0).floor() as usize;
    if estimated_measure_for_first_render_beat > state.current_measure_idx {
         measure_loop_start_idx = estimated_measure_for_first_render_beat.saturating_sub(1).min(state.processed_chart.measures.len().saturating_sub(1));
    }

    'measure_loop: for measure_idx in measure_loop_start_idx..state.processed_chart.measures.len() {
        let current_measure_data = &state.processed_chart.measures[measure_idx];
        if current_measure_data.is_empty() {
            if measure_idx == state.current_measure_idx {
                state.current_measure_idx += 1;
                state.current_line_in_measure_idx = 0;
            }
            continue;
        }

        let measure_base_actual_chart_beat = measure_idx as f32 * 4.0;
        let mut line_loop_start_idx = 0;
        if measure_idx == state.current_measure_idx {
            line_loop_start_idx = state.current_line_in_measure_idx;
        } else if measure_base_actual_chart_beat + 4.0 < first_actual_chart_beat_to_render {
            let measure_end_beat = measure_base_actual_chart_beat + 4.0;
            if measure_end_beat > state.current_processed_beat {
                 state.current_processed_beat = measure_end_beat;
            }
            continue;
        }

        for line_idx in line_loop_start_idx..current_measure_data.len() {
            let num_lines_in_measure = current_measure_data.len() as f32;
            let beat_offset_in_measure_for_line = (line_idx as f32 / num_lines_in_measure) * 4.0;
            let target_actual_chart_beat_for_line = measure_base_actual_chart_beat + beat_offset_in_measure_for_line;

            if target_actual_chart_beat_for_line <= state.current_processed_beat {
                if measure_idx == state.current_measure_idx && line_idx == state.current_line_in_measure_idx {
                     state.current_line_in_measure_idx +=1;
                }
                continue;
            }

            if target_actual_chart_beat_for_line > last_actual_chart_beat_to_render {
                 if measure_idx == state.current_measure_idx {
                     state.current_line_in_measure_idx = line_idx;
                 }
                 if measure_idx >= ((state.current_chart_beat_actual + MAX_DRAW_BEATS_FORWARD) / 4.0).floor() as usize {
                     break 'measure_loop; 
                 }
                 break; 
            }
            
            if target_actual_chart_beat_for_line >= first_actual_chart_beat_to_render {
                let note_line_data = &current_measure_data[line_idx];
                let arrow_display_target_beat = target_actual_chart_beat_for_line - display_beat_offset_due_to_audio_sync;

                let current_display_time_sec = state.timing_data.get_time_for_beat(state.current_beat);
                let arrow_target_display_time_sec = state.timing_data.get_time_for_beat(arrow_display_target_beat);
                let time_difference_to_display_target_sec = arrow_target_display_time_sec - current_display_time_sec;
                let target_receptor_y = state.targets.first().map_or(0.0, |t| t.y);
                let px_per_sec_scroll_speed = config::ARROW_SPEED * (state.window_size.1 / config::GAMEPLAY_REF_HEIGHT);
                let initial_y = target_receptor_y + time_difference_to_display_target_sec * px_per_sec_scroll_speed;

                for (col_idx, &note_char_val) in note_line_data.iter().enumerate() {
                    let direction = match col_idx {
                        0 => ArrowDirection::Left, 1 => ArrowDirection::Down,
                        2 => ArrowDirection::Up, 3 => ArrowDirection::Right,
                        _ => continue,
                    };
                    let arrow_type_for_render = match note_char_val {
                        NoteChar::Tap | NoteChar::HoldStart | NoteChar::RollStart => note_char_val,
                        _ => NoteChar::Empty,
                    };

                    if arrow_type_for_render != NoteChar::Empty {
                        let target_x_pos = state.targets.iter().find(|t| t.direction == direction).map_or(0.0, |t| t.x);
                        if let Some(column_arrows) = state.arrows.get_mut(&direction) {
                            column_arrows.push(Arrow {
                                x: target_x_pos,
                                y: initial_y,
                                direction,
                                note_char: arrow_type_for_render,
                                target_beat: arrow_display_target_beat, 
                            });
                        }
                    }
                }
            }
            state.current_processed_beat = target_actual_chart_beat_for_line;
            if measure_idx == state.current_measure_idx {
                state.current_line_in_measure_idx = line_idx + 1;
            }
        }

        if measure_idx == state.current_measure_idx && state.current_line_in_measure_idx >= current_measure_data.len() {
            state.current_measure_idx += 1;
            state.current_line_in_measure_idx = 0;
        }
    }
}


pub fn check_hits_on_press(state: &mut GameState, keycode: VirtualKeyCode) {
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

            let mut best_hit_idx: Option<usize> = None;
            let mut min_abs_time_diff_ms = config::MAX_HIT_WINDOW_MS + 1.0;

            for (idx, arrow) in column_arrows.iter().enumerate() {
                let bpm_at_arrow_target = state.timing_data.points.iter()
                    .rfind(|p| p.beat <= arrow.target_beat) 
                    .map_or(120.0, |p| p.bpm); 
                
                let seconds_per_beat_at_target = if bpm_at_arrow_target > 0.0 { 60.0 / bpm_at_arrow_target } else { 0.5 };

                let beat_diff = current_display_beat - arrow.target_beat;
                let time_diff_ms = beat_diff * seconds_per_beat_at_target * 1000.0;
                let abs_time_diff_ms = time_diff_ms.abs();

                if abs_time_diff_ms <= config::MAX_HIT_WINDOW_MS && abs_time_diff_ms < min_abs_time_diff_ms {
                    min_abs_time_diff_ms = abs_time_diff_ms;
                    best_hit_idx = Some(idx);
                }
            }

            if let Some(idx_to_remove) = best_hit_idx { 
                let hit_arrow = column_arrows[idx_to_remove].clone(); 
                 let bpm_at_arrow_target = state.timing_data.points.iter()
                    .rfind(|p| p.beat <= hit_arrow.target_beat)
                    .map_or(120.0, |p| p.bpm);
                let seconds_per_beat_at_target = if bpm_at_arrow_target > 0.0 { 60.0 / bpm_at_arrow_target } else { 0.5 };
                let time_diff_for_log = (current_display_beat - hit_arrow.target_beat) * seconds_per_beat_at_target * 1000.0;
                let note_char_for_log = hit_arrow.note_char;

                let judgment = if min_abs_time_diff_ms <= config::W1_WINDOW_MS { Judgment::W1 }
                               else if min_abs_time_diff_ms <= config::W2_WINDOW_MS { Judgment::W2 }
                               else if min_abs_time_diff_ms <= config::W3_WINDOW_MS { Judgment::W3 }
                               else if min_abs_time_diff_ms <= config::W4_WINDOW_MS { Judgment::W4 }
                               else { Judgment::W5 };

                *state.judgment_counts.entry(judgment).or_insert(0) += 1;

                info!( "HIT! {:?} {:?} (Beat: {:.3}, {:.1}ms) -> {:?} (Count: {})", dir, note_char_for_log, hit_arrow.target_beat, time_diff_for_log, judgment, state.judgment_counts[&judgment] );
                
                let explosion_end_time = Instant::now() + config::EXPLOSION_DURATION;
                state.active_explosions.insert(dir, ActiveExplosion {
                    judgment,
                    direction: dir,
                    end_time: explosion_end_time,
                });
                column_arrows.remove(idx_to_remove);
            } else {
                 debug!( "Input {:?} registered, but no arrow within {:.1}ms hit window (Display Beat: {:.2}).", keycode, config::MAX_HIT_WINDOW_MS, current_display_beat );
            }
        }
    }
}

fn check_misses(state: &mut GameState) {
    let current_display_beat = state.current_beat;
    let mut missed_count_this_frame = 0;
    for (_dir, column_arrows) in state.arrows.iter_mut() {
        column_arrows.retain(|arrow| {
            let bpm_at_arrow_target = state.timing_data.points.iter()
                .rfind(|p| p.beat <= arrow.target_beat) 
                .map_or(120.0, |p| p.bpm);
            
            let seconds_per_beat_at_target = if bpm_at_arrow_target > 0.0 { 60.0 / bpm_at_arrow_target } else { 0.5 };
            let miss_window_beats_dynamic = (config::MISS_WINDOW_MS / 1000.0) / seconds_per_beat_at_target;
            
            let beat_diff = current_display_beat - arrow.target_beat; 
            
            if beat_diff > miss_window_beats_dynamic { 
                *state.judgment_counts.entry(Judgment::Miss).or_insert(0) += 1;
                info!( "MISSED! {:?} {:?} (Beat: {:.3}) (TgtDispBeat: {:.2}, CurrDispBeat: {:.2}, DiffBeat: {:.2} > {:.2} ({:.1}ms)) (Miss Count: {})",
                       arrow.direction, arrow.note_char, arrow.target_beat, arrow.target_beat, current_display_beat, beat_diff, miss_window_beats_dynamic, config::MISS_WINDOW_MS, state.judgment_counts[&Judgment::Miss] );
                missed_count_this_frame += 1;
                false 
            } else { 
                true 
            }
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
    initial_pen_x_for_digits: f32,
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

    let label_start_x = initial_pen_x_for_digits + total_width_of_drawn_digits_and_their_spacing +
                        (config::JUDGMENT_ZERO_TO_LABEL_SPACING_REF * width_scale);

    renderer.draw_text(
        device, cmd_buf, miso_font, label_text,
        label_start_x, miso_label_baseline_y,
        bright_color, label_scale, None
    );
}

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
    let banner_width = config::GAMEPLAY_BANNER_WIDTH_REF * width_scale;
    let banner_height = config::GAMEPLAY_BANNER_HEIGHT_REF * height_scale;
    let banner_right_margin = config::GAMEPLAY_BANNER_RIGHT_MARGIN_REF * width_scale;
    let banner_top_margin = config::GAMEPLAY_BANNER_TOP_MARGIN_REF * height_scale;
    let banner_pos_x = win_w - banner_right_margin - banner_width / 2.0;
    let banner_pos_y = banner_top_margin + banner_height / 2.0;
    renderer.draw_quad( device, cmd_buf, DescriptorSetId::DynamicBanner, Vector3::new(banner_pos_x, banner_pos_y, 0.0), (banner_width, banner_height), Rad(0.0), [1.0, 1.0, 1.0, 1.0], [0.0, 0.0], [1.0, 1.0], );
    let health_meter_width = config::HEALTH_METER_WIDTH_REF * width_scale;
    let health_meter_height = config::HEALTH_METER_HEIGHT_REF * height_scale;
    let health_meter_border_thickness = config::HEALTH_METER_BORDER_THICKNESS_REF * width_scale.min(height_scale);
    let health_meter_outer_left_x = config::HEALTH_METER_LEFT_MARGIN_REF * width_scale;
    let health_meter_outer_top_y = config::HEALTH_METER_TOP_MARGIN_REF * height_scale;
    renderer.draw_quad( device, cmd_buf, DescriptorSetId::SolidColor, Vector3::new( health_meter_outer_left_x + health_meter_width / 2.0, health_meter_outer_top_y + health_meter_height / 2.0, 0.0 ), (health_meter_width, health_meter_height), Rad(0.0), config::HEALTH_METER_BORDER_COLOR, [0.0, 0.0], [1.0, 1.0] );
    let hm_inner_width = (health_meter_width - 2.0 * health_meter_border_thickness).max(0.0);
    let hm_inner_height = (health_meter_height - 2.0 * health_meter_border_thickness).max(0.0);
    let hm_inner_left_x = health_meter_outer_left_x + health_meter_border_thickness;
    let hm_inner_top_y = health_meter_outer_top_y + health_meter_border_thickness;
    if hm_inner_width > 0.0 && hm_inner_height > 0.0 {
        renderer.draw_quad( device, cmd_buf, DescriptorSetId::SolidColor, Vector3::new( hm_inner_left_x + hm_inner_width / 2.0, hm_inner_top_y + hm_inner_height / 2.0, 0.0 ), (hm_inner_width, hm_inner_height), Rad(0.0), config::HEALTH_METER_EMPTY_COLOR, [0.0, 0.0], [1.0, 1.0] );
        let current_health_percentage = 0.5;
        let hm_fill_width = (hm_inner_width * current_health_percentage).max(0.0);
        if hm_fill_width > 0.0 {
            renderer.draw_quad( device, cmd_buf, DescriptorSetId::SolidColor, Vector3::new( hm_inner_left_x + hm_fill_width / 2.0, hm_inner_top_y + hm_inner_height / 2.0, 0.0 ), (hm_fill_width, hm_inner_height), Rad(0.0), config::HEALTH_METER_FILL_COLOR, [0.0, 0.0], [1.0, 1.0] );
        }
    }
    let duration_meter_width = config::DURATION_METER_WIDTH_REF * width_scale;
    let duration_meter_height = config::DURATION_METER_HEIGHT_REF * height_scale;
    let duration_meter_border_thickness = config::DURATION_METER_BORDER_THICKNESS_REF * width_scale.min(height_scale);
    let duration_meter_outer_left_x = config::DURATION_METER_LEFT_MARGIN_REF * width_scale;
    let duration_meter_outer_top_y = config::DURATION_METER_TOP_MARGIN_REF * height_scale;
    let duration_meter_outer_bottom_y = duration_meter_outer_top_y + duration_meter_height;
    renderer.draw_quad( device, cmd_buf, DescriptorSetId::SolidColor, Vector3::new( duration_meter_outer_left_x + duration_meter_width / 2.0, duration_meter_outer_top_y + duration_meter_height / 2.0, 0.0 ), (duration_meter_width, duration_meter_height), Rad(0.0), config::DURATION_METER_BORDER_COLOR, [0.0, 0.0], [1.0, 1.0] );
    let dm_inner_width = (duration_meter_width - 2.0 * duration_meter_border_thickness).max(0.0);
    let dm_inner_height = (duration_meter_height - 2.0 * duration_meter_border_thickness).max(0.0);
    let dm_inner_left_x = duration_meter_outer_left_x + duration_meter_border_thickness;
    let dm_inner_top_y = duration_meter_outer_top_y + duration_meter_border_thickness;
    if dm_inner_width > 0.0 && dm_inner_height > 0.0 {
        renderer.draw_quad( device, cmd_buf, DescriptorSetId::SolidColor, Vector3::new( dm_inner_left_x + dm_inner_width / 2.0, dm_inner_top_y + dm_inner_height / 2.0, 0.0 ), (dm_inner_width, dm_inner_height), Rad(0.0), config::DURATION_METER_EMPTY_COLOR, [0.0, 0.0], [1.0, 1.0] );
        let total_duration_sec = game_state.song_info.charts[game_state.selected_chart_idx] .calculated_length_sec.unwrap_or(0.0);
        let current_elapsed_song_time_sec = if total_duration_sec > 0.0 && game_state.audio_start_time.is_some() { (game_state.timing_data.get_time_for_beat(game_state.current_chart_beat_actual) - game_state.timing_data.song_offset_sec).max(0.0) } else { 0.0 };
        let progress_percentage = if total_duration_sec > 0.01 { (current_elapsed_song_time_sec / total_duration_sec).clamp(0.0, 1.0) } else { 0.0 };
        let dm_fill_width = (dm_inner_width * progress_percentage).max(0.0);
        if dm_fill_width > 0.0 {
            renderer.draw_quad( device, cmd_buf, DescriptorSetId::SolidColor, Vector3::new( dm_inner_left_x + dm_fill_width / 2.0, dm_inner_top_y + dm_inner_height / 2.0, 0.0 ), (dm_fill_width, dm_inner_height), Rad(0.0), config::DURATION_METER_FILL_COLOR, [0.0, 0.0], [1.0, 1.0] );
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
            renderer.draw_text( device, cmd_buf, font, song_title, text_x.max(dm_inner_left_x), text_baseline_y, config::UI_BAR_TEXT_COLOR, text_scale, None );
        }
    }
    if let (Some(wendy_font_ref), Some(miso_font_ref)) = (assets.get_font(FontId::Wendy), assets.get_font(FontId::Miso)) {
        let mut current_line_visual_top_y = duration_meter_outer_bottom_y + (config::JUDGMENT_TEXT_LINE_TOP_OFFSET_FROM_DURATION_METER_REF * height_scale);
        let judgment_start_x = config::JUDGMENT_ZERO_LEFT_START_OFFSET_REF * width_scale;
        let zero_target_visual_height = config::JUDGMENT_ZERO_VISUAL_HEIGHT_REF * height_scale;
        let wendy_font_typographic_height_norm = (wendy_font_ref.metrics.ascender - wendy_font_ref.metrics.descender).max(1e-5);
        let wendy_zero_scale = zero_target_visual_height / wendy_font_typographic_height_norm;
        let label_target_visual_height = config::JUDGMENT_LABEL_VISUAL_HEIGHT_REF * height_scale;
        let miso_font_typographic_height_norm = (miso_font_ref.metrics.ascender - miso_font_ref.metrics.descender).max(1e-5);
        let miso_label_scale = label_target_visual_height / miso_font_typographic_height_norm;
        let judgment_lines_data = [ (Judgment::W1, "FANTASTIC", config::JUDGMENT_W1_DIM_COLOR, config::JUDGMENT_W1_BRIGHT_COLOR), (Judgment::W2, "PERFECT", config::JUDGMENT_W2_DIM_COLOR, config::JUDGMENT_W2_BRIGHT_COLOR), (Judgment::W3, "GREAT", config::JUDGMENT_W3_DIM_COLOR, config::JUDGMENT_W3_BRIGHT_COLOR), (Judgment::W4, "DECENT", config::JUDGMENT_W4_DIM_COLOR, config::JUDGMENT_W4_BRIGHT_COLOR), (Judgment::W5, "WAY OFF", config::JUDGMENT_W5_DIM_COLOR, config::JUDGMENT_W5_BRIGHT_COLOR), (Judgment::Miss,"MISS", config::JUDGMENT_MISS_DIM_COLOR, config::JUDGMENT_MISS_BRIGHT_COLOR), ];
        let line_visual_height_for_spacing = zero_target_visual_height.max(label_target_visual_height);
        let vertical_spacing_between_lines = config::JUDGMENT_LINE_VERTICAL_SPACING_REF * height_scale;
        for (judgment_type, label, dim_color, bright_color) in judgment_lines_data.iter() {
            let count = game_state.judgment_counts.get(judgment_type).copied().unwrap_or(0);
            draw_judgment_line( renderer, device, cmd_buf, assets, current_line_visual_top_y, judgment_start_x, wendy_zero_scale, miso_label_scale, label, count, *dim_color, *bright_color, height_scale, width_scale );
            current_line_visual_top_y += line_visual_height_for_spacing + vertical_spacing_between_lines;
        }
    }


    // --- Draw Targets & Arrows ---
    let frame_index_receptor = ((game_state.current_beat * 2.0).floor().abs() as usize) % 4;
    let uv_width_receptor = 1.0 / 4.0;
    let uv_x_start_receptor = frame_index_receptor as f32 * uv_width_receptor;
    let target_uv_offset = [uv_x_start_receptor, 0.0];
    let target_uv_scale = [uv_width_receptor, 1.0];

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
        renderer.draw_quad(
            device, cmd_buf, DescriptorSetId::Gameplay,
            Vector3::new(target.x, target.y, 0.0),
            quad_size_for_draw_quad,
            rotation_angle, config::TARGET_TINT,
            target_uv_offset,
            target_uv_scale,
        );
    }

    for (_direction, column_arrows) in &game_state.arrows {
        for arrow in column_arrows {
            let culling_margin_y_bottom = desired_on_screen_height * 1.5; 
            let culling_margin_y_top = win_h + desired_on_screen_height * 0.5; 
            if arrow.y < (0.0 - culling_margin_y_bottom) || arrow.y > culling_margin_y_top {
                continue;
            }
            
            let beat_diff_for_anim = arrow.target_beat - game_state.current_beat;
            let frame_index_arrow = (((-beat_diff_for_anim * 2.0).floor().abs() as i32 % 4) + 4)%4; 
            let uv_width_arrow = 1.0 / 4.0;
            let uv_x_start_arrow = frame_index_arrow as f32 * uv_width_arrow;
            let arrow_uv_offset = [uv_x_start_arrow, 0.0];
            let arrow_uv_scale = [uv_width_arrow, 1.0];
            
            // --- Arrow Tinting Logic ---
            const BEAT_EPSILON_DRAW: f32 = 0.002; // Epsilon for float comparisons of beat fractions
            let beat_fraction = arrow.target_beat.fract();
            let normalized_fraction = {
                let mut nf = beat_fraction;
                if nf < -BEAT_EPSILON_DRAW { nf += 1.0; } // Handle negative fractions by wrapping around
                // Snap to 0.0 if very close to 0.0 or 1.0
                if nf.abs() < BEAT_EPSILON_DRAW || (nf - 1.0).abs() < BEAT_EPSILON_DRAW { 0.0 } else { nf }
            };

            let arrow_tint = if (normalized_fraction - 0.0).abs() < BEAT_EPSILON_DRAW { // 4th (X.000)
                config::ARROW_TINT_4TH
            } else if (normalized_fraction - 0.5).abs() < BEAT_EPSILON_DRAW { // 8th (X.500)
                config::ARROW_TINT_8TH
            } else if (normalized_fraction - 0.25).abs() < BEAT_EPSILON_DRAW || // 16th (X.250)
                      (normalized_fraction - 0.75).abs() < BEAT_EPSILON_DRAW { // 16th (X.750)
                config::ARROW_TINT_16TH
            } else if (normalized_fraction - 1.0/3.0).abs() < BEAT_EPSILON_DRAW || // 12th (X.333)
                      (normalized_fraction - 2.0/3.0).abs() < BEAT_EPSILON_DRAW { // 12th (X.666)
                config::ARROW_TINT_12TH_24TH_48TH // Also for 24th and 48th triplets offshoots
            } else if (normalized_fraction - 1.0/8.0).abs() < BEAT_EPSILON_DRAW ||  // 32nd (X.125)
                      (normalized_fraction - 3.0/8.0).abs() < BEAT_EPSILON_DRAW ||  // 32nd (X.375)
                      (normalized_fraction - 5.0/8.0).abs() < BEAT_EPSILON_DRAW ||  // 32nd (X.625)
                      (normalized_fraction - 7.0/8.0).abs() < BEAT_EPSILON_DRAW {  // 32nd (X.875)
                config::ARROW_TINT_32ND
            } else if (normalized_fraction - 1.0/6.0).abs() < BEAT_EPSILON_DRAW ||  // 24th (X.166)
                      (normalized_fraction - 5.0/6.0).abs() < BEAT_EPSILON_DRAW {  // 24th (X.833)
                config::ARROW_TINT_12TH_24TH_48TH // Group with 12ths
            } else if (normalized_fraction - 1.0/12.0).abs() < BEAT_EPSILON_DRAW || // 48th (X.083)
                      (normalized_fraction - 5.0/12.0).abs() < BEAT_EPSILON_DRAW || // 48th (X.416)
                      (normalized_fraction - 7.0/12.0).abs() < BEAT_EPSILON_DRAW || // 48th (X.583)
                      (normalized_fraction - 11.0/12.0).abs() < BEAT_EPSILON_DRAW { // 48th (X.916)
                config::ARROW_TINT_12TH_24TH_48TH // Group with 12ths
            } else if { // 64ths: k/16 where k is odd
                let val_times_16_approx = normalized_fraction * 16.0;
                let val_times_16_rounded = val_times_16_approx.round();
                // Check if it's close to a 16th fraction
                (val_times_16_approx - val_times_16_rounded).abs() < BEAT_EPSILON_DRAW * 16.0 &&
                // And if that 16th fraction has an odd numerator (e.g. 1/16, 3/16, 5/16...)
                (val_times_16_rounded as i32 % 2 != 0)
            } {
                config::ARROW_TINT_64TH
            } else { // Fallback for other fractions (e.g., 96ths, 192nds or complex non-standard)
                config::ARROW_TINT_OTHER
            };

            // Log for debugging quantization and tint
            // debug!("Arrow Beat (Disp): {:.3}, Frac: {:.3}, Tint: {:?}", arrow.target_beat, normalized_fraction, arrow_tint);

            let rotation_angle = match arrow.direction {
                ArrowDirection::Left => Rad(PI / 2.0), ArrowDirection::Down => Rad(0.0),
                ArrowDirection::Up => Rad(PI), ArrowDirection::Right => Rad(-PI / 2.0),
            };
            let quad_size_for_draw_quad = match arrow.direction {
                ArrowDirection::Up | ArrowDirection::Down => (desired_on_screen_width, desired_on_screen_height),
                ArrowDirection::Left | ArrowDirection::Right => (desired_on_screen_height, desired_on_screen_width),
            };
            renderer.draw_quad(
                device, cmd_buf, DescriptorSetId::Gameplay,
                Vector3::new(arrow.x, arrow.y, 0.0),
                quad_size_for_draw_quad,
                rotation_angle, arrow_tint,
                arrow_uv_offset, 
                arrow_uv_scale,
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