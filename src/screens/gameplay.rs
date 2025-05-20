// src/screens/gameplay.rs
use crate::assets::{AssetManager, TextureId};
use crate::config;
use crate::graphics::renderer::{DescriptorSetId, Renderer};
use crate::parsing::simfile::{ChartInfo, NoteChar, ProcessedChartData, SongInfo};
use crate::state::{
    ActiveExplosion,
    AppState, Arrow, ArrowDirection, GameState, Judgment, TargetInfo,
    VirtualKeyCode, ALL_ARROW_DIRECTIONS,
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
    audio_start_time: Instant,
    song: Arc<SongInfo>,
    selected_chart_idx: usize,
) -> GameState {
    info!( "Initializing game state for song: '{}', chart index: {}", song.title, selected_chart_idx );

    let width_scale = win_w / config::GAMEPLAY_REF_WIDTH;
    let height_scale = win_h / config::GAMEPLAY_REF_HEIGHT;
    
    // These represent the desired *on-screen* dimensions after any rotation.
    let desired_on_screen_width = config::TARGET_VISUAL_SIZE_REF * width_scale; 
    let desired_on_screen_height = config::TARGET_VISUAL_SIZE_REF * height_scale;

    // Horizontal dimension for layout: how wide is the "lane" or "cell" for each arrow.
    // This should be based on the desired on-screen width of an unrotated arrow (like Up/Down).
    let arrow_lane_width = desired_on_screen_width; 
    let current_target_spacing = config::TARGET_SPACING_REF * width_scale; 

    let current_target_top_margin = config::TARGET_TOP_MARGIN_REF * height_scale;
    let current_first_target_left_margin = config::FIRST_TARGET_LEFT_MARGIN_REF * width_scale;

    // Y position is center of the target, based on its desired on-screen height.
    let target_center_y = current_target_top_margin + desired_on_screen_height / 2.0;
    
    // X position of the *center of the first lane*.
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
    for dir in ALL_ARROW_DIRECTIONS.iter() {
        arrows_map.insert(*dir, Vec::new());
    }

    let mut temp_timing_data = TimingData { song_offset_sec: song.offset, ..Default::default() };
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
        temp_timing_data.points.push(BeatTimePoint { beat: 0.0, time_sec: song.offset, bpm: 120.0 });
    } else {
        let mut current_time = song.offset;
        let mut last_b_beat = 0.0;
        let mut last_b_bpm = combined_bpms[0].1;
        if combined_bpms[0].0 != 0.0 { 
            temp_timing_data.points.push(BeatTimePoint { beat: 0.0, time_sec: song.offset, bpm: last_b_bpm});
        }
        for (beat, bpm) in &combined_bpms {
            if *beat < last_b_beat { continue; } 
            if *beat > last_b_beat { current_time += (*beat - last_b_beat) * (60.0 / last_b_bpm); }
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

    let initial_bpm_at_zero = temp_timing_data.points.iter()
        .find(|p| p.beat == 0.0)
        .map_or_else(
            || temp_timing_data.points.first().map_or(120.0, |p|p.bpm),
            |p| p.bpm
        );
    let initial_display_beat_offset = (config::AUDIO_SYNC_OFFSET_MS as f32 / 1000.0) / (60.0 / initial_bpm_at_zero);

    GameState {
        targets,
        arrows: arrows_map,
        pressed_keys: HashSet::new(),
        current_beat: -initial_display_beat_offset,
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
    if let Some(start_time) = game_state.audio_start_time {
        let current_raw_time_sec = Instant::now().duration_since(start_time).as_secs_f32();
        let chart_beat = game_state.timing_data.get_beat_for_time(current_raw_time_sec);
        
        let initial_bpm_at_zero = game_state.timing_data.points.iter()
            .find(|p| p.beat == 0.0)
            .map_or_else(
                || game_state.timing_data.points.first().map_or(120.0, |p|p.bpm),
                |p| p.bpm
            );
        let initial_display_beat_offset = (config::AUDIO_SYNC_OFFSET_MS as f32 / 1000.0) / (60.0 / initial_bpm_at_zero);
        
        game_state.current_beat = chart_beat - initial_display_beat_offset;
        trace!("Current Raw Time: {:.3}s, Chart Beat: {:.4}, Display Beat: {:.4}", current_raw_time_sec, chart_beat, game_state.current_beat);
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
    let current_audio_time_sec = if let Some(start_time) = state.audio_start_time {
        Instant::now().duration_since(start_time).as_secs_f32()
    } else { return; };
    let current_chart_beat_now = state.timing_data.get_beat_for_time(current_audio_time_sec);
    let lookahead_chart_beat_limit = current_chart_beat_now + config::SPAWN_LOOKAHEAD_BEATS;

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
        
        let time_of_line_sec = state.timing_data.get_time_for_beat(target_beat_for_line);
        let time_to_target_on_screen_sec = time_of_line_sec - current_audio_time_sec;

        if time_to_target_on_screen_sec < 0.0 {
            trace!("Missed spawn window for line at beat {:.2} (current audio time corresponds to beat {:.2})", target_beat_for_line, current_chart_beat_now);
            state.current_processed_beat = target_beat_for_line;
            state.current_line_in_measure_idx += 1;
            continue;
        }
        
        let target_y_pos = state.targets.first().map_or(0.0, |t| t.y);
        let current_arrow_speed = config::ARROW_SPEED * (state.window_size.1 / config::GAMEPLAY_REF_HEIGHT);
        let distance_to_travel_pixels = current_arrow_speed * time_to_target_on_screen_sec;
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
        if spawned_on_this_line { debug!("Spawned notes for line at measure {}, line_idx {}, chart_beat {:.3}", state.current_measure_idx, state.current_line_in_measure_idx, target_beat_for_line); }
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
            let seconds_per_beat_approx = 60.0 / bpm_at_arrow_approx;

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

                info!( "HIT! {:?} {:?} ({:.1}ms) -> {:?}", dir, note_char_for_log, time_diff_for_log, judgment );

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
    let mut missed_count = 0;
    for (_dir, column_arrows) in state.arrows.iter_mut() {
        column_arrows.retain(|arrow| {
            let bpm_at_arrow_target = state.timing_data.points.iter().rfind(|p| p.beat <= arrow.target_beat).map_or(120.0, |p| p.bpm);
            let seconds_per_beat_at_target = 60.0 / bpm_at_arrow_target;
            let miss_window_beats_dynamic = (config::MISS_WINDOW_MS / 1000.0) / seconds_per_beat_at_target;
            let beat_diff = current_display_beat - arrow.target_beat;
            if beat_diff > miss_window_beats_dynamic {
                info!( "MISSED! {:?} {:?} (TgtBeat: {:.2}, DispBeat: {:.2}, DiffBeat: {:.2} > {:.2} ({:.1}ms))", arrow.direction, arrow.note_char, arrow.target_beat, current_display_beat, beat_diff, miss_window_beats_dynamic, config::MISS_WINDOW_MS );
                missed_count += 1;
                false
            } else { true }
        });
    }
    if missed_count > 0 { trace!("Removed {} missed arrows.", missed_count); }
}

// --- Drawing Logic ---
pub fn draw(
    renderer: &Renderer,
    game_state: &GameState,
    _assets: &AssetManager, 
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
) {
    let (win_w, win_h) = renderer.window_size();
    let width_scale = win_w / config::GAMEPLAY_REF_WIDTH;
    let height_scale = win_h / config::GAMEPLAY_REF_HEIGHT;
    
    // These are the *desired final on-screen dimensions* for an unrotated (Up/Down) arrow.
    let desired_on_screen_width = config::TARGET_VISUAL_SIZE_REF * width_scale;
    let desired_on_screen_height = config::TARGET_VISUAL_SIZE_REF * height_scale;
    
    // Explosions will also use these as their base, scaled by the multiplier.
    let desired_explosion_on_screen_width = desired_on_screen_width * config::EXPLOSION_SIZE_MULTIPLIER; 
    let desired_explosion_on_screen_height = desired_on_screen_height * config::EXPLOSION_SIZE_MULTIPLIER;


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

    let inner_width = (health_meter_width - 2.0 * health_meter_border_thickness).max(0.0);
    let inner_height = (health_meter_height - 2.0 * health_meter_border_thickness).max(0.0);
    let inner_left_x = health_meter_outer_left_x + health_meter_border_thickness;
    let inner_top_y = health_meter_outer_top_y + health_meter_border_thickness;

    if inner_width > 0.0 && inner_height > 0.0 {
        renderer.draw_quad(
            device, cmd_buf, DescriptorSetId::SolidColor,
            Vector3::new(
                inner_left_x + inner_width / 2.0,
                inner_top_y + inner_height / 2.0,
                0.0
            ),
            (inner_width, inner_height),
            Rad(0.0),
            config::HEALTH_METER_EMPTY_COLOR,
            [0.0, 0.0], [1.0, 1.0]
        );

        let current_health_percentage = 0.5; 
        let fill_width = (inner_width * current_health_percentage).max(0.0);
        if fill_width > 0.0 {
            renderer.draw_quad(
                device, cmd_buf, DescriptorSetId::SolidColor,
                Vector3::new(
                    inner_left_x + fill_width / 2.0, 
                    inner_top_y + inner_height / 2.0,
                    0.0
                ),
                (fill_width, inner_height),
                Rad(0.0),
                config::HEALTH_METER_FILL_COLOR,
                [0.0, 0.0], [1.0, 1.0]
            );
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

    // Draw Targets
    for target in &game_state.targets {
        let rotation_angle = match target.direction {
            ArrowDirection::Left => Rad(PI / 2.0),
            ArrowDirection::Down => Rad(0.0),
            ArrowDirection::Up => Rad(PI),
            ArrowDirection::Right => Rad(-PI / 2.0),
        };

        // Determine pre-rotation scale for draw_quad
        let quad_size_for_draw_quad = match target.direction {
            ArrowDirection::Up | ArrowDirection::Down => {
                (desired_on_screen_width, desired_on_screen_height)
            }
            ArrowDirection::Left | ArrowDirection::Right => {
                // Swap: local width becomes screen height, local height becomes screen width
                (desired_on_screen_height, desired_on_screen_width) 
            }
        };

        renderer.draw_quad( device, cmd_buf, DescriptorSetId::Gameplay, 
            Vector3::new(target.x, target.y, 0.0),
            quad_size_for_draw_quad, 
            rotation_angle, config::TARGET_TINT, 
            target_uv_offset, 
            target_uv_scale,
        );
    }

    // Draw Arrows 
    for (_direction, column_arrows) in &game_state.arrows {
        for arrow in column_arrows {
            let culling_margin_w = desired_on_screen_width; // Use max potential screen extent for culling
            let culling_margin_h = desired_on_screen_height;
            if arrow.y < (0.0 - culling_margin_h) || arrow.y > (win_h + culling_margin_h) { // Culling based on Y and screen height
                continue;
            }
            
            let measure_idx_for_arrow = (arrow.target_beat / 4.0).floor() as usize;
            let mut arrow_tint = config::ARROW_TINT_OTHER;
            // ... (tint logic remains the same) ...
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

            // Determine pre-rotation scale for draw_quad for this arrow
            let quad_size_for_draw_quad = match arrow.direction {
                ArrowDirection::Up | ArrowDirection::Down => {
                    (desired_on_screen_width, desired_on_screen_height)
                }
                ArrowDirection::Left | ArrowDirection::Right => {
                    (desired_on_screen_height, desired_on_screen_width)
                }
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

                    // Determine pre-rotation scale for draw_quad for this explosion
                    let quad_size_for_draw_quad = match target_info.direction {
                        ArrowDirection::Up | ArrowDirection::Down => {
                            (desired_explosion_on_screen_width, desired_explosion_on_screen_height)
                        }
                        ArrowDirection::Left | ArrowDirection::Right => {
                            (desired_explosion_on_screen_height, desired_explosion_on_screen_width)
                        }
                    };

                    renderer.draw_quad(
                        device,
                        cmd_buf,
                        explosion_set_id,
                        Vector3::new(target_info.x, target_info.y, 0.0),
                        quad_size_for_draw_quad, 
                        explosion_rotation_angle,
                        [1.0, 1.0, 1.0, 1.0], // Tint
                        [0.0, 0.0], // UV offset
                        [1.0, 1.0], // UV scale
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