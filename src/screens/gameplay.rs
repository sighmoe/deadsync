use crate::assets::{AssetManager, FontId};
use crate::config;
use crate::graphics::renderer::{DescriptorSetId, Renderer};
use crate::parsing::simfile::{ChartInfo, NoteChar, ProcessedChartData, SongInfo};
use crate::state::{
    ActiveExplosion, AppState, Arrow, ArrowDirection, GameState, Judgment, TargetInfo,
    VirtualKeyCode, ALL_ARROW_DIRECTIONS, ALL_JUDGMENTS,
};
use ash::vk;
use cgmath::{Rad, Vector3};
use log::{debug, info, trace, warn};
use rand::Rng;
use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;
use std::sync::Arc;
use std::time::Instant;
use winit::keyboard::{Key, NamedKey};
use winit::{
    event::{ElementState, KeyEvent},
    keyboard::ModifiersState,
};

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
    pub stops_at_beat: Vec<(f32, f32)>,
    pub song_offset_sec: f32,
}

impl TimingData {
    pub fn get_time_for_beat(&self, target_beat: f32) -> f32 {
        if self.points.is_empty() {
            warn!("TimingData::get_time_for_beat called with empty points, defaulting to 120 BPM.");
            return self.song_offset_sec + target_beat * (60.0 / 120.0);
        }

        let mut current_calculated_duration = 0.0;
        let mut last_processed_beat = 0.0;

        let mut current_segment_bpm = self.points[0].bpm;

        for point in &self.points {
            if point.beat <= last_processed_beat && point.beat != 0.0 {
                current_segment_bpm = point.bpm;
                continue;
            }

            if target_beat < point.beat {
                if current_segment_bpm > 0.0 {
                    current_calculated_duration +=
                        (target_beat - last_processed_beat) * (60.0 / current_segment_bpm);
                } else if target_beat > last_processed_beat {
                    current_calculated_duration = f32::INFINITY;
                }
                last_processed_beat = target_beat;
                break;
            }

            if current_segment_bpm > 0.0 {
                current_calculated_duration +=
                    (point.beat - last_processed_beat) * (60.0 / current_segment_bpm);
            } else if point.beat > last_processed_beat {
                current_calculated_duration = f32::INFINITY;
            }
            last_processed_beat = point.beat;
            current_segment_bpm = point.bpm;
        }

        if target_beat > last_processed_beat {
            if current_calculated_duration.is_finite() && current_segment_bpm > 0.0 {
                current_calculated_duration +=
                    (target_beat - last_processed_beat) * (60.0 / current_segment_bpm);
            } else if current_segment_bpm <= 0.0 && target_beat > last_processed_beat {
                current_calculated_duration = f32::INFINITY;
            }
        }

        let mut time_with_bpms = self.song_offset_sec + current_calculated_duration;

        for (stop_beat, stop_duration_sec) in &self.stops_at_beat {
            if *stop_beat < target_beat {
                if time_with_bpms.is_finite() {
                    time_with_bpms += stop_duration_sec;
                }
            }
        }
        time_with_bpms
    }

    pub fn get_beat_for_time(&self, target_time_sec_relative_to_audio_zero: f32) -> f32 {
        if self.points.is_empty() {
            warn!("TimingData::get_beat_for_time called with empty points, defaulting to 120 BPM.");
            let duration_from_offset =
                target_time_sec_relative_to_audio_zero - self.song_offset_sec;
            return duration_from_offset / (60.0 / 120.0);
        }

        let mut current_beat = 0.0;
        let mut current_time_in_audio_clock = self.song_offset_sec;
        let mut current_bpm = self.points[0].bpm;

        let mut events: Vec<(f32, Option<f32>, Option<f32>)> = Vec::new();
        for p in &self.points {
            events.push((p.beat, Some(p.bpm), None));
        }
        for s in &self.stops_at_beat {
            events.push((s.0, None, Some(s.1)));
        }

        events.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| match (a.1.is_some(), b.1.is_some()) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => std::cmp::Ordering::Equal,
                })
        });

        let mut unique_events: Vec<(f32, Option<f32>, Option<f32>)> = Vec::new();
        if !events.is_empty() {
            if events[0].0 > 0.0 {
                // Accessing the first element of the tuple correctly
                unique_events.push((0.0, Some(self.points[0].bpm), None));
            }
            unique_events.push(events[0]);
            for i in 1..events.len() {
                if (events[i].0 - unique_events.last().unwrap().0).abs() > 0.0001 {
                    unique_events.push(events[i]);
                } else {
                    let last = unique_events.last_mut().unwrap();
                    if events[i].1.is_some() {
                        last.1 = events[i].1;
                    }
                    if events[i].2.is_some() && last.2.is_none() {
                        last.2 = events[i].2;
                    }
                }
            }
        } else {
            unique_events.push((0.0, Some(self.points[0].bpm), None));
        }

        for (event_beat, new_bpm_opt, stop_duration_sec_opt) in &unique_events {
            let beats_in_segment = *event_beat - current_beat; // Dereference event_beat
            let time_for_segment_beats_only = if current_bpm > 0.0 {
                beats_in_segment * (60.0 / current_bpm)
            } else {
                if beats_in_segment > 0.0 {
                    f32::INFINITY
                } else {
                    0.0
                }
            };

            let time_at_event_beat_in_audio_clock =
                current_time_in_audio_clock + time_for_segment_beats_only;

            if time_at_event_beat_in_audio_clock >= target_time_sec_relative_to_audio_zero {
                let time_diff_in_segment =
                    target_time_sec_relative_to_audio_zero - current_time_in_audio_clock;
                return current_beat
                    + if current_bpm > 0.0 {
                        time_diff_in_segment / (60.0 / current_bpm)
                    } else {
                        0.0
                    };
            }

            current_time_in_audio_clock = time_at_event_beat_in_audio_clock;
            current_beat = *event_beat; // Dereference event_beat

            if let Some(new_bpm) = new_bpm_opt {
                current_bpm = *new_bpm;
            } // Dereference new_bpm

            if let Some(stop_dur) = stop_duration_sec_opt {
                if current_time_in_audio_clock + stop_dur >= target_time_sec_relative_to_audio_zero
                {
                    return current_beat;
                }
                current_time_in_audio_clock += stop_dur;
            }
        }

        let time_diff_after_last_event =
            target_time_sec_relative_to_audio_zero - current_time_in_audio_clock;
        current_beat
            + if current_bpm > 0.0 {
                time_diff_after_last_event / (60.0 / current_bpm)
            } else {
                0.0
            }
    }
}

const MAX_DRAW_BEATS_FORWARD: f32 = 12.0;
const MAX_DRAW_BEATS_BACK: f32 = 3.0;
const GLOBAL_OFFSET_ADJUST_STEP_SEC: f32 = 0.001;
const OFFSET_FEEDBACK_DISPLAY_DURATION_SEC: f32 = 2.0;

// Helper function to create TimingData
fn create_timing_data(
    song_offset_sec: f32,
    song: &Arc<SongInfo>,
    chart_info: &ChartInfo,
) -> TimingData {
    let mut new_timing_data = TimingData {
        song_offset_sec,
        ..Default::default()
    };

    let mut combined_bpms = song.bpms_header.clone();
    if let Some(chart_bpms_str) = &chart_info.bpms_chart {
        if let Ok(chart_bpms_vec) = crate::parsing::simfile::parse_bpms(chart_bpms_str) {
            if !chart_bpms_vec.is_empty() {
                combined_bpms = chart_bpms_vec;
            }
        }
    }
    combined_bpms.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    combined_bpms.dedup_by_key(|k| k.0);
    if combined_bpms.is_empty() || combined_bpms[0].0 != 0.0 {
        let initial_bpm = combined_bpms.first().map_or(120.0, |p| p.1);
        if combined_bpms.first().map_or(true, |p| p.0 != 0.0) {
            combined_bpms.insert(0, (0.0, initial_bpm));
        }
    }

    let mut current_calc_time_from_offset = 0.0;
    let mut last_calc_beat = 0.0;
    let mut last_calc_bpm = combined_bpms[0].1;
    for (beat, bpm_val) in &combined_bpms {
        if *beat < last_calc_beat {
            continue;
        }
        if *beat > last_calc_beat && last_calc_bpm > 0.0 {
            current_calc_time_from_offset += (*beat - last_calc_beat) * (60.0 / last_calc_bpm);
        }
        new_timing_data.points.push(BeatTimePoint {
            beat: *beat,
            time_sec: song_offset_sec + current_calc_time_from_offset,
            bpm: *bpm_val,
        });
        last_calc_beat = *beat;
        last_calc_bpm = *bpm_val;
    }

    let mut combined_stops = song.stops_header.clone();
    if let Some(chart_stops_str) = &chart_info.stops_chart {
        if let Ok(chart_stops_vec) = crate::parsing::simfile::parse_stops(chart_stops_str) {
            if !chart_stops_vec.is_empty() {
                combined_stops = chart_stops_vec;
            }
        }
    }
    combined_stops.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    combined_stops.dedup_by_key(|k| k.0);
    for (beat, duration_simfile_value) in &combined_stops {
        new_timing_data
            .stops_at_beat
            .push((*beat, *duration_simfile_value));
    }
    new_timing_data
}

fn update_timing_data_with_new_offset(game_state: &mut GameState) {
    info!(
        "Updating TimingData with new global offset: {:.3}s",
        game_state.current_global_offset_sec
    );
    let effective_file_offset =
        -game_state.song_info.offset + -game_state.current_global_offset_sec;
    let chart_info = &game_state.song_info.charts[game_state.selected_chart_idx];

    let new_timing_data =
        create_timing_data(effective_file_offset, &game_state.song_info, chart_info);
    game_state.timing_data = Arc::new(new_timing_data);
}

pub fn initialize_game_state(
    win_w: f32,
    win_h: f32,
    audio_start_time: Instant,
    song: Arc<SongInfo>,
    selected_chart_idx: usize,
) -> GameState {
    info!(
        "Initializing game state for song: '{}', chart index: {}",
        song.title, selected_chart_idx
    );

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
    for dir in ALL_ARROW_DIRECTIONS.iter() {
        arrows_map.insert(*dir, Vec::new());
    }

    let current_initial_global_offset_sec = config::GLOBAL_OFFSET_SEC; // Will be stored in GameState
    let effective_file_offset = -song.offset + -current_initial_global_offset_sec;
    let chart_info = &song.charts[selected_chart_idx];

    let initial_timing_data = create_timing_data(effective_file_offset, &song, chart_info);

    let processed_chart_data = chart_info
        .processed_data
        .as_ref()
        .cloned()
        .unwrap_or_else(|| {
            warn!(
                "Chart {} for song {} has no processed data! Gameplay might be empty.",
                selected_chart_idx, song.title
            );
            ProcessedChartData::default()
        });

    let time_at_visual_start_relative_to_audio_zero = -config::GAME_LEAD_IN_DURATION_SECONDS;

    let mut initial_actual_chart_beat_at_receptors_on_visual_start =
        initial_timing_data.get_beat_for_time(time_at_visual_start_relative_to_audio_zero);

    // Display beat is now the same as actual chart beat (after global offset is applied to TimingData)
    let mut initial_display_beat_at_receptors_on_visual_start =
        initial_actual_chart_beat_at_receptors_on_visual_start;

    let effective_scroll_time_for_beat0_before_audio =
        config::GAME_LEAD_IN_DURATION_SECONDS + song.offset;
    const MIN_GUARANTEED_SCROLL_TIME_FOR_CHART_START_NOTES: f32 =
        config::GAME_LEAD_IN_DURATION_SECONDS;

    if effective_scroll_time_for_beat0_before_audio
        < MIN_GUARANTEED_SCROLL_TIME_FOR_CHART_START_NOTES
    {
        let time_deficit = MIN_GUARANTEED_SCROLL_TIME_FOR_CHART_START_NOTES
            - effective_scroll_time_for_beat0_before_audio;

        let bpm_at_chart_beat_0 = initial_timing_data
            .points
            .iter()
            .rfind(|p| p.beat <= 0.0)
            .map_or_else(
                || {
                    initial_timing_data
                        .points
                        .first()
                        .map_or(120.0, |p_first| p_first.bpm)
                },
                |p_at_zero| p_at_zero.bpm,
            );

        if bpm_at_chart_beat_0 > 0.0 {
            let beat_adjustment_for_deficit = time_deficit / (60.0 / bpm_at_chart_beat_0);

            info!(
                "Adjusting initial beats for scroll lead-in. Deficit: {:.2}s, Beat Adj: {:.2}. Song Offset: {:.2}s",
                time_deficit, beat_adjustment_for_deficit, song.offset
            );
            initial_actual_chart_beat_at_receptors_on_visual_start -= beat_adjustment_for_deficit;
            initial_display_beat_at_receptors_on_visual_start -= beat_adjustment_for_deficit;
        }
    }

    let mut judgment_counts = HashMap::new();
    for judgment_type in ALL_JUDGMENTS.iter() {
        judgment_counts.insert(*judgment_type, 0);
    }

    GameState {
        targets,
        arrows: arrows_map,
        pressed_keys: HashSet::new(),
        current_beat: initial_display_beat_at_receptors_on_visual_start,
        current_chart_beat_actual: initial_actual_chart_beat_at_receptors_on_visual_start,
        window_size: (win_w, win_h),
        active_explosions: HashMap::new(),
        audio_start_time: Some(audio_start_time),
        song_info: song,
        selected_chart_idx,
        timing_data: Arc::new(initial_timing_data),
        processed_chart: Arc::new(processed_chart_data),
        current_measure_idx: 0,
        current_line_in_measure_idx: 0,
        current_processed_beat: initial_actual_chart_beat_at_receptors_on_visual_start
            - MAX_DRAW_BEATS_BACK
            - 1.0,
        judgment_counts,
        lead_in_timer: config::GAME_LEAD_IN_DURATION_SECONDS,
        music_started: false,
        is_esc_held: false,
        esc_held_since: None,
        is_enter_held: false,
        enter_held_since: None,
        current_global_offset_sec: current_initial_global_offset_sec,
        offset_feedback_message: None,
        offset_feedback_duration_remaining: 0.0,
    }
}

pub fn handle_input(key_event: &KeyEvent, game_state: &mut GameState, modifiers: ModifiersState) {
    if key_event.state == ElementState::Pressed && !key_event.repeat {
        // Handle Shift+F11/F12 for global offset adjustment
        if modifiers.shift_key() {
            let old_offset = game_state.current_global_offset_sec;
            let mut offset_changed = false;

            match key_event.logical_key {
                Key::Named(NamedKey::F11) => {
                    game_state.current_global_offset_sec -= GLOBAL_OFFSET_ADJUST_STEP_SEC;
                    offset_changed = true;
                    info!(
                        "Global offset decreased to: {:.3}s",
                        game_state.current_global_offset_sec
                    );
                }
                Key::Named(NamedKey::F12) => {
                    game_state.current_global_offset_sec += GLOBAL_OFFSET_ADJUST_STEP_SEC;
                    offset_changed = true;
                    info!(
                        "Global offset increased to: {:.3}s",
                        game_state.current_global_offset_sec
                    );
                }
                _ => {}
            }

            if offset_changed {
                update_timing_data_with_new_offset(game_state);
                game_state.offset_feedback_message = Some(format!(
                    "Global Offset from {:.3}s to {:.3}s (notes {})",
                    old_offset,
                    game_state.current_global_offset_sec,
                    if game_state.current_global_offset_sec > old_offset {
                        "earlier"
                    } else {
                        "later"
                    }
                ));
                game_state.offset_feedback_duration_remaining =
                    OFFSET_FEEDBACK_DISPLAY_DURATION_SEC;
                return; // Consume the input if it was an offset change
            }
        }

        // Existing Escape handling (without Shift)
        if let Some(VirtualKeyCode::Escape) =
            crate::state::key_to_virtual_keycode(key_event.logical_key.clone())
        {
            if !modifiers.shift_key() {
                // Ensure shift is not held for this
                info!("Escape pressed in gameplay, returning to Select Music.");
            }
        }
    }

    if let Some(virtual_keycode) =
        crate::state::key_to_virtual_keycode(key_event.logical_key.clone())
    {
        match virtual_keycode {
            VirtualKeyCode::Escape => match key_event.state {
                ElementState::Pressed => {
                    if !game_state.is_esc_held && !key_event.repeat {
                        game_state.is_esc_held = true;
                        game_state.esc_held_since = Some(Instant::now());
                        debug!("Gameplay: Escape key pressed, timer started.");
                    }
                }
                ElementState::Released => {
                    if game_state.is_esc_held {
                        game_state.is_esc_held = false;
                        game_state.esc_held_since = None;
                        debug!("Gameplay: Escape key released, timer reset.");
                    }
                }
            },
            VirtualKeyCode::Enter => match key_event.state {
                ElementState::Pressed => {
                    if !game_state.is_enter_held && !key_event.repeat {
                        game_state.is_enter_held = true;
                        game_state.enter_held_since = Some(Instant::now());
                        debug!("Gameplay: Enter key pressed, timer started.");
                    }
                }
                ElementState::Released => {
                    if game_state.is_enter_held {
                        game_state.is_enter_held = false;
                        game_state.enter_held_since = None;
                        debug!("Gameplay: Enter key released, timer reset.");
                    }
                }
            },
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
}

pub fn update(game_state: &mut GameState, dt: f32, _rng: &mut impl Rng) -> Option<AppState> {
    let mut next_state: Option<AppState> = None;

    if game_state.is_esc_held {
        if let Some(held_since) = game_state.esc_held_since {
            if Instant::now().duration_since(held_since) >= config::HOLD_TO_ACTION_DURATION {
                info!(
                    "Gameplay: Escape held for {:?}. Transitioning to SelectMusic.",
                    config::HOLD_TO_ACTION_DURATION
                );
                next_state = Some(AppState::SelectMusic);
                game_state.is_esc_held = false;
                game_state.esc_held_since = None;
            }
        }
    }

    if game_state.is_enter_held {
        if let Some(held_since) = game_state.enter_held_since {
            if Instant::now().duration_since(held_since) >= config::HOLD_TO_ACTION_DURATION {
                info!(
                    "Gameplay: Enter held for {:?}. Transitioning to ScoreScreen.",
                    config::HOLD_TO_ACTION_DURATION
                );
                next_state = Some(AppState::ScoreScreen);
                game_state.is_enter_held = false;
                game_state.enter_held_since = None;
            }
        }
    }

    if game_state.offset_feedback_duration_remaining > 0.0 {
        game_state.offset_feedback_duration_remaining -= dt;
        if game_state.offset_feedback_duration_remaining <= 0.0 {
            game_state.offset_feedback_message = None;
            game_state.offset_feedback_duration_remaining = 0.0;
        }
    }

    if !game_state.music_started && game_state.lead_in_timer > 0.0 {
        game_state.lead_in_timer -= dt;
    }

    if let Some(expected_audio_zero_instant) = game_state.audio_start_time {
        let current_time_relative_to_audio_zero = if Instant::now() >= expected_audio_zero_instant {
            Instant::now()
                .duration_since(expected_audio_zero_instant)
                .as_secs_f32()
        } else {
            -(expected_audio_zero_instant
                .duration_since(Instant::now())
                .as_secs_f32())
        };

        // current_beat (display beat) is now the same as current_chart_beat_actual
        game_state.current_chart_beat_actual = game_state
            .timing_data
            .get_beat_for_time(current_time_relative_to_audio_zero);
        game_state.current_beat = game_state.current_chart_beat_actual;
    }

    spawn_arrows_from_chart(game_state);

    let current_display_beat = game_state.current_beat;
    let current_display_time_sec = game_state
        .timing_data
        .get_time_for_beat(current_display_beat);
    let target_receptor_y = game_state.targets.first().map_or(0.0, |t| t.y);
    let px_per_sec_scroll_speed =
        config::ARROW_SPEED * (game_state.window_size.1 / config::GAMEPLAY_REF_HEIGHT);

    for (_direction, column_arrows) in game_state.arrows.iter_mut() {
        for arrow in column_arrows.iter_mut() {
            let arrow_target_display_time_sec =
                game_state.timing_data.get_time_for_beat(arrow.target_beat);
            let time_difference_to_display_target_sec =
                arrow_target_display_time_sec - current_display_time_sec;
            arrow.y =
                target_receptor_y + time_difference_to_display_target_sec * px_per_sec_scroll_speed;
        }
    }

    check_misses(game_state);

    let now = Instant::now();
    game_state
        .active_explosions
        .retain(|_dir, explosion| now < explosion.end_time);

    next_state
}

fn spawn_arrows_from_chart(state: &mut GameState) {
    if state.processed_chart.measures.is_empty() {
        return;
    }

    let first_actual_chart_beat_to_render = state.current_chart_beat_actual - MAX_DRAW_BEATS_BACK;
    let last_actual_chart_beat_to_render = state.current_chart_beat_actual + MAX_DRAW_BEATS_FORWARD;

    let mut measure_scan_start_idx = state.current_measure_idx;
    let estimated_measure_for_first_render_beat =
        (first_actual_chart_beat_to_render / 4.0).floor() as isize;

    if estimated_measure_for_first_render_beat > state.current_measure_idx as isize {
        measure_scan_start_idx = (estimated_measure_for_first_render_beat.saturating_sub(1)
            as usize)
            .min(state.processed_chart.measures.len().saturating_sub(1));
    } else if estimated_measure_for_first_render_beat < 0 {
        measure_scan_start_idx = 0;
    }

    'measure_loop: for measure_idx in measure_scan_start_idx..state.processed_chart.measures.len() {
        let current_measure_data = &state.processed_chart.measures[measure_idx];
        if current_measure_data.is_empty() {
            if measure_idx == state.current_measure_idx {
                state.current_measure_idx = measure_idx + 1;
                state.current_line_in_measure_idx = 0;
            }
            continue;
        }

        let measure_base_actual_chart_beat = measure_idx as f32 * 4.0;

        let mut line_scan_start_idx = 0;
        if measure_idx == state.current_measure_idx {
            line_scan_start_idx = state.current_line_in_measure_idx;
        } else if measure_base_actual_chart_beat + 4.0 < first_actual_chart_beat_to_render {
            let measure_end_beat = measure_base_actual_chart_beat + 4.0;
            if measure_end_beat > state.current_processed_beat {
                state.current_processed_beat = measure_end_beat;
            }
            continue;
        }

        for line_idx in line_scan_start_idx..current_measure_data.len() {
            let num_lines_in_measure = current_measure_data.len() as f32;
            let beat_offset_in_measure_for_line = (line_idx as f32 / num_lines_in_measure) * 4.0;
            let target_actual_chart_beat_for_line =
                measure_base_actual_chart_beat + beat_offset_in_measure_for_line;

            if target_actual_chart_beat_for_line <= state.current_processed_beat {
                if measure_idx == state.current_measure_idx
                    && line_idx == state.current_line_in_measure_idx
                {
                    state.current_line_in_measure_idx = line_idx + 1;
                }
                continue;
            }

            if target_actual_chart_beat_for_line > last_actual_chart_beat_to_render {
                if measure_idx == state.current_measure_idx {
                    state.current_line_in_measure_idx = line_idx;
                }
                if measure_idx as f32 * 4.0 > last_actual_chart_beat_to_render {
                    break 'measure_loop;
                }
                break;
            }

            if target_actual_chart_beat_for_line >= first_actual_chart_beat_to_render {
                let note_line_data = &current_measure_data[line_idx];
                // Arrow's display target beat is now the same as its actual chart beat
                let arrow_display_target_beat = target_actual_chart_beat_for_line;
                let current_display_time_sec =
                    state.timing_data.get_time_for_beat(state.current_beat);
                let arrow_target_display_time_sec = state
                    .timing_data
                    .get_time_for_beat(arrow_display_target_beat);
                let time_difference_to_display_target_sec =
                    arrow_target_display_time_sec - current_display_time_sec;

                let target_receptor_y = state.targets.first().map_or(0.0, |t| t.y);
                let px_per_sec_scroll_speed =
                    config::ARROW_SPEED * (state.window_size.1 / config::GAMEPLAY_REF_HEIGHT);
                let initial_y = target_receptor_y
                    + time_difference_to_display_target_sec * px_per_sec_scroll_speed;

                for (col_idx, &note_char_val) in note_line_data.iter().enumerate() {
                    let direction = match col_idx {
                        0 => ArrowDirection::Left,
                        1 => ArrowDirection::Down,
                        2 => ArrowDirection::Up,
                        3 => ArrowDirection::Right,
                        _ => continue,
                    };
                    let arrow_type_for_render = match note_char_val {
                        NoteChar::Tap | NoteChar::HoldStart | NoteChar::RollStart => note_char_val,
                        _ => NoteChar::Empty,
                    };

                    if arrow_type_for_render != NoteChar::Empty {
                        let target_x_pos = state
                            .targets
                            .iter()
                            .find(|t| t.direction == direction)
                            .map_or(0.0, |t| t.x);
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

        if measure_idx == state.current_measure_idx
            && state.current_line_in_measure_idx >= current_measure_data.len()
        {
            state.current_measure_idx = measure_idx + 1;
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
                let bpm_at_arrow_target = state
                    .timing_data
                    .points
                    .iter()
                    .rfind(|p| p.beat <= arrow.target_beat)
                    .map_or(120.0, |p| p.bpm);

                let seconds_per_beat_at_target = if bpm_at_arrow_target > 0.0 {
                    60.0 / bpm_at_arrow_target
                } else {
                    0.5
                };

                let beat_diff = current_display_beat - arrow.target_beat;
                let time_diff_ms = beat_diff * seconds_per_beat_at_target * 1000.0;
                let abs_time_diff_ms = time_diff_ms.abs();

                if abs_time_diff_ms <= config::MAX_HIT_WINDOW_MS
                    && abs_time_diff_ms < min_abs_time_diff_ms
                {
                    min_abs_time_diff_ms = abs_time_diff_ms;
                    best_hit_idx = Some(idx);
                }
            }

            if let Some(idx_to_remove) = best_hit_idx {
                let hit_arrow = column_arrows[idx_to_remove].clone();
                let time_diff_for_log = min_abs_time_diff_ms
                    * if (current_display_beat - hit_arrow.target_beat) < 0.0 {
                        -1.0
                    } else {
                        1.0
                    };
                let note_char_for_log = hit_arrow.note_char;

                let judgment = if min_abs_time_diff_ms <= config::W1_WINDOW_MS {
                    Judgment::W1
                } else if min_abs_time_diff_ms <= config::W2_WINDOW_MS {
                    Judgment::W2
                } else if min_abs_time_diff_ms <= config::W3_WINDOW_MS {
                    Judgment::W3
                } else if min_abs_time_diff_ms <= config::W4_WINDOW_MS {
                    Judgment::W4
                } else {
                    Judgment::W5
                };

                *state.judgment_counts.entry(judgment).or_insert(0) += 1;

                info!(
                    "HIT! {:?} {:?} (TargetDispBeat: {:.3}, TimeDiff: {:.1}ms) -> {:?} (Count: {})",
                    dir,
                    note_char_for_log,
                    hit_arrow.target_beat,
                    time_diff_for_log,
                    judgment,
                    state.judgment_counts[&judgment]
                );

                let explosion_end_time = Instant::now() + config::EXPLOSION_DURATION;
                state.active_explosions.insert(
                    dir,
                    ActiveExplosion {
                        judgment,
                        direction: dir,
                        end_time: explosion_end_time,
                    },
                );
                column_arrows.remove(idx_to_remove);
            } else {
                debug!( "Input {:?} registered, but no arrow within {:.1}ms hit window (CurrDispBeat: {:.2}).", keycode, config::MAX_HIT_WINDOW_MS, current_display_beat );
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
                info!( "MISSED! {:?} {:?} (TargetDispBeat: {:.3}) (CurrDispBeat: {:.2}, DiffBeat: {:.2} > DynamicMissWindowBeats: {:.2} ({:.1}ms)) (Miss Count: {})",
                       arrow.direction, arrow.note_char, arrow.target_beat, current_display_beat, beat_diff, miss_window_beats_dynamic, config::MISS_WINDOW_MS, state.judgment_counts[&Judgment::Miss] );
                missed_count_this_frame += 1;
                false
            } else {
                true
            }
        });
    }
    if missed_count_this_frame > 0 {
        trace!("Removed {} missed arrows.", missed_count_this_frame);
    }
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
    let miso_label_baseline_y =
        line_top_y + (miso_font.metrics.ascender * label_scale) + scaled_label_nudge;

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
        if *digit_char == '0' && !first_non_zero_found && idx < count_chars.len() - 1 {
            is_bright = false;
        } else {
            is_bright = true;
            if *digit_char != '0' {
                first_non_zero_found = true;
            }
        }
        let color = if is_bright { bright_color } else { dim_color };

        renderer.draw_text(
            device,
            cmd_buf,
            wendy_font,
            &digit_str,
            current_digit_pen_x + actual_pre_spacing,
            wendy_zero_baseline_y,
            color,
            zero_scale,
            None,
        );

        let mut actual_post_spacing = general_digit_spacing;
        if *digit_char == '1' {
            actual_post_spacing = digit_one_post_extra_space;
        }

        let advance_for_this_digit_segment = actual_pre_spacing + digit_width + actual_post_spacing;
        current_digit_pen_x += advance_for_this_digit_segment;
        total_width_of_drawn_digits_and_their_spacing += advance_for_this_digit_segment;
    }

    let label_start_x = initial_pen_x_for_digits
        + total_width_of_drawn_digits_and_their_spacing
        + (config::JUDGMENT_ZERO_TO_LABEL_SPACING_REF * width_scale);

    renderer.draw_text(
        device,
        cmd_buf,
        miso_font,
        label_text,
        label_start_x,
        miso_label_baseline_y,
        bright_color,
        label_scale,
        None,
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
    let desired_explosion_on_screen_width =
        desired_on_screen_width * config::EXPLOSION_SIZE_MULTIPLIER;
    let desired_explosion_on_screen_height =
        desired_on_screen_height * config::EXPLOSION_SIZE_MULTIPLIER;
    let banner_width = config::GAMEPLAY_BANNER_WIDTH_REF * width_scale;
    let banner_height = config::GAMEPLAY_BANNER_HEIGHT_REF * height_scale;
    let banner_right_margin = config::GAMEPLAY_BANNER_RIGHT_MARGIN_REF * width_scale;
    let banner_top_margin = config::GAMEPLAY_BANNER_TOP_MARGIN_REF * height_scale;
    let banner_pos_x = win_w - banner_right_margin - banner_width / 2.0;
    let banner_pos_y = banner_top_margin + banner_height / 2.0;
    renderer.draw_quad(
        device,
        cmd_buf,
        DescriptorSetId::DynamicBanner,
        Vector3::new(banner_pos_x, banner_pos_y, 0.0),
        (banner_width, banner_height),
        Rad(0.0),
        [1.0, 1.0, 1.0, 1.0],
        [0.0, 0.0],
        [1.0, 1.0],
    );
    let health_meter_width = config::HEALTH_METER_WIDTH_REF * width_scale;
    let health_meter_height = config::HEALTH_METER_HEIGHT_REF * height_scale;
    let health_meter_border_thickness =
        config::HEALTH_METER_BORDER_THICKNESS_REF * width_scale.min(height_scale);
    let health_meter_outer_left_x = config::HEALTH_METER_LEFT_MARGIN_REF * width_scale;
    let health_meter_outer_top_y = config::HEALTH_METER_TOP_MARGIN_REF * height_scale;
    renderer.draw_quad(
        device,
        cmd_buf,
        DescriptorSetId::SolidColor,
        Vector3::new(
            health_meter_outer_left_x + health_meter_width / 2.0,
            health_meter_outer_top_y + health_meter_height / 2.0,
            0.0,
        ),
        (health_meter_width, health_meter_height),
        Rad(0.0),
        config::HEALTH_METER_BORDER_COLOR,
        [0.0, 0.0],
        [1.0, 1.0],
    );
    let hm_inner_width = (health_meter_width - 2.0 * health_meter_border_thickness).max(0.0);
    let hm_inner_height = (health_meter_height - 2.0 * health_meter_border_thickness).max(0.0);
    let hm_inner_left_x = health_meter_outer_left_x + health_meter_border_thickness;
    let hm_inner_top_y = health_meter_outer_top_y + health_meter_border_thickness;
    if hm_inner_width > 0.0 && hm_inner_height > 0.0 {
        renderer.draw_quad(
            device,
            cmd_buf,
            DescriptorSetId::SolidColor,
            Vector3::new(
                hm_inner_left_x + hm_inner_width / 2.0,
                hm_inner_top_y + hm_inner_height / 2.0,
                0.0,
            ),
            (hm_inner_width, hm_inner_height),
            Rad(0.0),
            config::HEALTH_METER_EMPTY_COLOR,
            [0.0, 0.0],
            [1.0, 1.0],
        );
        let current_health_percentage = 0.5;
        let hm_fill_width = (hm_inner_width * current_health_percentage).max(0.0);
        if hm_fill_width > 0.0 {
            renderer.draw_quad(
                device,
                cmd_buf,
                DescriptorSetId::SolidColor,
                Vector3::new(
                    hm_inner_left_x + hm_fill_width / 2.0,
                    hm_inner_top_y + hm_inner_height / 2.0,
                    0.0,
                ),
                (hm_fill_width, hm_inner_height),
                Rad(0.0),
                config::HEALTH_METER_FILL_COLOR,
                [0.0, 0.0],
                [1.0, 1.0],
            );
        }
    }
    let duration_meter_width = config::DURATION_METER_WIDTH_REF * width_scale;
    let duration_meter_height = config::DURATION_METER_HEIGHT_REF * height_scale;
    let duration_meter_border_thickness =
        config::DURATION_METER_BORDER_THICKNESS_REF * width_scale.min(height_scale);
    let duration_meter_outer_left_x = config::DURATION_METER_LEFT_MARGIN_REF * width_scale;
    let duration_meter_outer_top_y = config::DURATION_METER_TOP_MARGIN_REF * height_scale;
    let duration_meter_outer_bottom_y = duration_meter_outer_top_y + duration_meter_height;
    renderer.draw_quad(
        device,
        cmd_buf,
        DescriptorSetId::SolidColor,
        Vector3::new(
            duration_meter_outer_left_x + duration_meter_width / 2.0,
            duration_meter_outer_top_y + duration_meter_height / 2.0,
            0.0,
        ),
        (duration_meter_width, duration_meter_height),
        Rad(0.0),
        config::DURATION_METER_BORDER_COLOR,
        [0.0, 0.0],
        [1.0, 1.0],
    );
    let dm_inner_width = (duration_meter_width - 2.0 * duration_meter_border_thickness).max(0.0);
    let dm_inner_height = (duration_meter_height - 2.0 * duration_meter_border_thickness).max(0.0);
    let dm_inner_left_x = duration_meter_outer_left_x + duration_meter_border_thickness;
    let dm_inner_top_y = duration_meter_outer_top_y + duration_meter_border_thickness;
    if dm_inner_width > 0.0 && dm_inner_height > 0.0 {
        renderer.draw_quad(
            device,
            cmd_buf,
            DescriptorSetId::SolidColor,
            Vector3::new(
                dm_inner_left_x + dm_inner_width / 2.0,
                dm_inner_top_y + dm_inner_height / 2.0,
                0.0,
            ),
            (dm_inner_width, dm_inner_height),
            Rad(0.0),
            config::DURATION_METER_EMPTY_COLOR,
            [0.0, 0.0],
            [1.0, 1.0],
        );
        let total_duration_sec = game_state.song_info.charts[game_state.selected_chart_idx]
            .calculated_length_sec
            .unwrap_or(0.0);
        let current_elapsed_song_time_sec =
            if total_duration_sec > 0.0 && game_state.audio_start_time.is_some() {
                (game_state
                    .timing_data
                    .get_time_for_beat(game_state.current_chart_beat_actual)
                    - game_state.timing_data.song_offset_sec)
                    .max(0.0)
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
                device,
                cmd_buf,
                DescriptorSetId::SolidColor,
                Vector3::new(
                    dm_inner_left_x + dm_fill_width / 2.0,
                    dm_inner_top_y + dm_inner_height / 2.0,
                    0.0,
                ),
                (dm_fill_width, dm_inner_height),
                Rad(0.0),
                config::DURATION_METER_FILL_COLOR,
                [0.0, 0.0],
                [1.0, 1.0],
            );
        }
        if let Some(font) = assets.get_font(FontId::Miso) {
            let song_title = &game_state.song_info.title;
            let target_text_visual_height = dm_inner_height * 0.80;
            let font_typographic_height_norm =
                (font.metrics.ascender - font.metrics.descender).max(1e-5);
            let text_scale = target_text_visual_height / font_typographic_height_norm;
            let text_width_pixels = font.measure_text_normalized(song_title) * text_scale;
            let text_x = dm_inner_left_x + (dm_inner_width - text_width_pixels) / 2.0;
            let mut text_baseline_y = (dm_inner_top_y + dm_inner_height / 2.0)
                - (font.metrics.ascender + font.metrics.descender) / 2.0 * text_scale;
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
                None,
            );
        }
    }
    if let (Some(wendy_font_ref), Some(miso_font_ref)) = (
        assets.get_font(FontId::Wendy),
        assets.get_font(FontId::Miso),
    ) {
        let mut current_line_visual_top_y = duration_meter_outer_bottom_y
            + (config::JUDGMENT_TEXT_LINE_TOP_OFFSET_FROM_DURATION_METER_REF * height_scale);
        let judgment_start_x = config::JUDGMENT_ZERO_LEFT_START_OFFSET_REF * width_scale;
        let zero_target_visual_height = config::JUDGMENT_ZERO_VISUAL_HEIGHT_REF * height_scale;
        let wendy_font_typographic_height_norm =
            (wendy_font_ref.metrics.ascender - wendy_font_ref.metrics.descender).max(1e-5);
        let wendy_zero_scale = zero_target_visual_height / wendy_font_typographic_height_norm;
        let label_target_visual_height = config::JUDGMENT_LABEL_VISUAL_HEIGHT_REF * height_scale;
        let miso_font_typographic_height_norm =
            (miso_font_ref.metrics.ascender - miso_font_ref.metrics.descender).max(1e-5);
        let miso_label_scale = label_target_visual_height / miso_font_typographic_height_norm;
        let judgment_lines_data = [
            (
                Judgment::W1,
                "FANTASTIC",
                config::JUDGMENT_W1_DIM_COLOR,
                config::JUDGMENT_W1_BRIGHT_COLOR,
            ),
            (
                Judgment::W2,
                "PERFECT",
                config::JUDGMENT_W2_DIM_COLOR,
                config::JUDGMENT_W2_BRIGHT_COLOR,
            ),
            (
                Judgment::W3,
                "GREAT",
                config::JUDGMENT_W3_DIM_COLOR,
                config::JUDGMENT_W3_BRIGHT_COLOR,
            ),
            (
                Judgment::W4,
                "DECENT",
                config::JUDGMENT_W4_DIM_COLOR,
                config::JUDGMENT_W4_BRIGHT_COLOR,
            ),
            (
                Judgment::W5,
                "WAY OFF",
                config::JUDGMENT_W5_DIM_COLOR,
                config::JUDGMENT_W5_BRIGHT_COLOR,
            ),
            (
                Judgment::Miss,
                "MISS",
                config::JUDGMENT_MISS_DIM_COLOR,
                config::JUDGMENT_MISS_BRIGHT_COLOR,
            ),
        ];
        let line_visual_height_for_spacing =
            zero_target_visual_height.max(label_target_visual_height);
        let vertical_spacing_between_lines =
            config::JUDGMENT_LINE_VERTICAL_SPACING_REF * height_scale;
        for (judgment_type, label, dim_color, bright_color) in judgment_lines_data.iter() {
            let count = game_state
                .judgment_counts
                .get(judgment_type)
                .copied()
                .unwrap_or(0);
            draw_judgment_line(
                renderer,
                device,
                cmd_buf,
                assets,
                current_line_visual_top_y,
                judgment_start_x,
                wendy_zero_scale,
                miso_label_scale,
                label,
                count,
                *dim_color,
                *bright_color,
                height_scale,
                width_scale,
            );
            current_line_visual_top_y +=
                line_visual_height_for_spacing + vertical_spacing_between_lines;
        }
    }

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
            ArrowDirection::Up | ArrowDirection::Down => {
                (desired_on_screen_width, desired_on_screen_height)
            }
            ArrowDirection::Left | ArrowDirection::Right => {
                (desired_on_screen_height, desired_on_screen_width)
            }
        };
        renderer.draw_quad(
            device,
            cmd_buf,
            DescriptorSetId::Gameplay,
            Vector3::new(target.x, target.y, 0.0),
            quad_size_for_draw_quad,
            rotation_angle,
            config::TARGET_TINT,
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
            let frame_index_arrow =
                (((-beat_diff_for_anim * 2.0).floor().abs() as i32 % 4) + 4) % 4;
            let uv_width_arrow = 1.0 / 4.0;
            let uv_x_start_arrow = frame_index_arrow as f32 * uv_width_arrow;
            let arrow_uv_offset = [uv_x_start_arrow, 0.0];
            let arrow_uv_scale = [uv_width_arrow, 1.0];

            const BEAT_EPSILON_DRAW: f32 = 0.002;
            let beat_fraction = arrow.target_beat.fract();
            let normalized_fraction = {
                let mut nf = beat_fraction;
                if nf < -BEAT_EPSILON_DRAW {
                    nf += 1.0;
                }
                if nf.abs() < BEAT_EPSILON_DRAW || (nf - 1.0).abs() < BEAT_EPSILON_DRAW {
                    0.0
                } else {
                    nf
                }
            };

            let arrow_tint = if (normalized_fraction - 0.0).abs() < BEAT_EPSILON_DRAW {
                config::ARROW_TINT_4TH
            } else if (normalized_fraction - 0.5).abs() < BEAT_EPSILON_DRAW {
                config::ARROW_TINT_8TH
            } else if (normalized_fraction - 0.25).abs() < BEAT_EPSILON_DRAW
                || (normalized_fraction - 0.75).abs() < BEAT_EPSILON_DRAW
            {
                config::ARROW_TINT_16TH
            } else if (normalized_fraction - 1.0 / 3.0).abs() < BEAT_EPSILON_DRAW
                || (normalized_fraction - 2.0 / 3.0).abs() < BEAT_EPSILON_DRAW
            {
                config::ARROW_TINT_12TH_24TH_48TH
            } else if (normalized_fraction - 1.0 / 8.0).abs() < BEAT_EPSILON_DRAW
                || (normalized_fraction - 3.0 / 8.0).abs() < BEAT_EPSILON_DRAW
                || (normalized_fraction - 5.0 / 8.0).abs() < BEAT_EPSILON_DRAW
                || (normalized_fraction - 7.0 / 8.0).abs() < BEAT_EPSILON_DRAW
            {
                config::ARROW_TINT_32ND
            } else if (normalized_fraction - 1.0 / 6.0).abs() < BEAT_EPSILON_DRAW
                || (normalized_fraction - 5.0 / 6.0).abs() < BEAT_EPSILON_DRAW
            {
                config::ARROW_TINT_12TH_24TH_48TH
            } else if (normalized_fraction - 1.0 / 12.0).abs() < BEAT_EPSILON_DRAW
                || (normalized_fraction - 5.0 / 12.0).abs() < BEAT_EPSILON_DRAW
                || (normalized_fraction - 7.0 / 12.0).abs() < BEAT_EPSILON_DRAW
                || (normalized_fraction - 11.0 / 12.0).abs() < BEAT_EPSILON_DRAW
            {
                config::ARROW_TINT_12TH_24TH_48TH
            } else if {
                let val_times_16_approx = normalized_fraction * 16.0;
                let val_times_16_rounded = val_times_16_approx.round();
                (val_times_16_approx - val_times_16_rounded).abs() < BEAT_EPSILON_DRAW * 16.0
                    && (val_times_16_rounded as i32 % 2 != 0)
            } {
                config::ARROW_TINT_64TH
            } else {
                config::ARROW_TINT_OTHER
            };

            let rotation_angle = match arrow.direction {
                ArrowDirection::Left => Rad(PI / 2.0),
                ArrowDirection::Down => Rad(0.0),
                ArrowDirection::Up => Rad(PI),
                ArrowDirection::Right => Rad(-PI / 2.0),
            };
            let quad_size_for_draw_quad = match arrow.direction {
                ArrowDirection::Up | ArrowDirection::Down => {
                    (desired_on_screen_width, desired_on_screen_height)
                }
                ArrowDirection::Left | ArrowDirection::Right => {
                    (desired_on_screen_height, desired_on_screen_width)
                }
            };
            renderer.draw_quad(
                device,
                cmd_buf,
                DescriptorSetId::Gameplay,
                Vector3::new(arrow.x, arrow.y, 0.0),
                quad_size_for_draw_quad,
                rotation_angle,
                arrow_tint,
                arrow_uv_offset,
                arrow_uv_scale,
            );
        }
    }

    let now = Instant::now();
    for (direction, explosion) in &game_state.active_explosions {
        if now < explosion.end_time {
            if let Some(explosion_set_id) = DescriptorSetId::from_judgment(explosion.judgment) {
                if let Some(target_info) = game_state
                    .targets
                    .iter()
                    .find(|t| t.direction == *direction)
                {
                    let explosion_rotation_angle = match target_info.direction {
                        ArrowDirection::Left => Rad(PI / 2.0),
                        ArrowDirection::Down => Rad(0.0),
                        ArrowDirection::Up => Rad(PI),
                        ArrowDirection::Right => Rad(-PI / 2.0),
                    };
                    let quad_size_for_draw_quad = match target_info.direction {
                        ArrowDirection::Up | ArrowDirection::Down => (
                            desired_explosion_on_screen_width,
                            desired_explosion_on_screen_height,
                        ),
                        ArrowDirection::Left | ArrowDirection::Right => (
                            desired_explosion_on_screen_height,
                            desired_explosion_on_screen_width,
                        ),
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

    let center_x = win_w / 2.0; // For centering text

    if game_state.is_esc_held && game_state.esc_held_since.is_some() {
        if let Some(font) = assets.get_font(FontId::Miso) {
            let text = "Continue holding ESC to give up.";
            let target_visual_height = config::HOLD_TEXT_VISUAL_HEIGHT_REF * height_scale;
            let font_typographic_height_norm =
                (font.metrics.ascender - font.metrics.descender).max(1e-5);
            let text_scale = target_visual_height / font_typographic_height_norm;

            let text_width_pixels = font.measure_text_normalized(text) * text_scale;
            let text_x = center_x - text_width_pixels / 2.0;

            let text_bottom_edge_y = win_h - (config::HOLD_TEXT_BOTTOM_MARGIN_REF * height_scale);
            let baseline_y = text_bottom_edge_y - (font.metrics.descender * text_scale); // Baseline relative to bottom edge

            renderer.draw_text(
                device,
                cmd_buf,
                font,
                text,
                text_x,
                baseline_y,
                [1.0, 1.0, 1.0, 1.0], // White text
                text_scale,
                None,
            );
        }
    }

    if game_state.is_enter_held && game_state.enter_held_since.is_some() {
        if let Some(font) = assets.get_font(FontId::Miso) {
            let text = "Continue holding Enter to give up.";
            let target_visual_height = config::HOLD_TEXT_VISUAL_HEIGHT_REF * height_scale;
            let font_typographic_height_norm =
                (font.metrics.ascender - font.metrics.descender).max(1e-5);
            let text_scale = target_visual_height / font_typographic_height_norm;

            let text_width_pixels = font.measure_text_normalized(text) * text_scale;
            let text_x = center_x - text_width_pixels / 2.0;

            let text_bottom_edge_y = win_h - (config::HOLD_TEXT_BOTTOM_MARGIN_REF * height_scale);
            let baseline_y = text_bottom_edge_y - (font.metrics.descender * text_scale); // Baseline relative to bottom edge

            renderer.draw_text(
                device,
                cmd_buf,
                font,
                text,
                text_x,
                baseline_y,
                [1.0, 1.0, 1.0, 1.0], // White text
                text_scale,
                None,
            );
        }
    }

    // Draw global offset feedback message
    if let Some(feedback_msg) = &game_state.offset_feedback_message {
        if game_state.offset_feedback_duration_remaining > 0.0 {
            if let Some(font) = assets.get_font(FontId::Miso) {
                let target_visual_height = 20.0 * height_scale;
                let font_typographic_height_norm =
                    (font.metrics.ascender - font.metrics.descender).max(1e-5);
                let text_scale = target_visual_height / font_typographic_height_norm;

                let text_width_pixels = font.measure_text_normalized(feedback_msg) * text_scale;
                let text_x = center_x - text_width_pixels / 2.0;

                let bottom_margin_pixels = 125.0 * height_scale;
                let text_bottom_edge_y = win_h - bottom_margin_pixels;
                let baseline_y = text_bottom_edge_y - (font.metrics.descender * text_scale);

                renderer.draw_text(
                    device,
                    cmd_buf,
                    font,
                    feedback_msg,
                    text_x,
                    baseline_y,
                    [1.0, 1.0, 1.0, 1.0], // White text
                    text_scale,
                    None,
                );
            }
        }
    }
}
