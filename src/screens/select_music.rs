// src/screens/select_music.rs
use crate::assets::{AssetManager, FontId, SoundId};
use crate::audio::AudioManager;
use crate::config;
use crate::graphics::renderer::{DescriptorSetId, Renderer};
use crate::parsing::simfile::SongInfo;
use crate::state::{AppState, SelectMusicState, VirtualKeyCode, MusicWheelEntry, NavDirection};
use ash::vk;
use cgmath::{Rad, Vector3, InnerSpace};
use log::debug;
use std::f32::consts::PI;
use std::sync::Arc;
use std::time::{Instant, Duration};
use winit::event::{ElementState, KeyEvent};

fn lerp_color(color_a: [f32; 4], color_b: [f32; 4], t: f32) -> [f32; 4] {
    [
        color_a[0] * (1.0 - t) + color_b[0] * t,
        color_a[1] * (1.0 - t) + color_b[1] * t,
        color_a[2] * (1.0 - t) + color_b[2] * t,
        color_a[3] * (1.0 - t) + color_b[3] * t,
    ]
}

fn format_duration_flexible(total_seconds_f: f32) -> String {
    if total_seconds_f < 0.0 {
        return "0:00".to_string();
    }
    let total_seconds = total_seconds_f.round() as u32;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else if minutes >= 10 || minutes == 0 {
        format!("{:02}:{:02}", minutes, seconds)
    }
    else {
        format!("{}:{:02}", minutes, seconds)
    }
}


pub fn handle_input(
    key_event: &KeyEvent,
    state: &mut SelectMusicState,
    audio_manager: &AudioManager,
) -> (Option<AppState>, bool) {
    let mut selection_changed_this_frame = false;

    if let Some(virtual_keycode) =
        crate::state::key_to_virtual_keycode(key_event.logical_key.clone())
    {
        let num_entries = state.entries.len();

        match key_event.state {
            ElementState::Pressed => {
                if !key_event.repeat {
                    match virtual_keycode {
                        VirtualKeyCode::Left | VirtualKeyCode::Up => {
                            if num_entries > 0 {
                                let old_index = state.selected_index;
                                state.selected_index = if state.selected_index == 0 {
                                    num_entries - 1
                                } else {
                                    state.selected_index - 1
                                };
                                if state.selected_index != old_index {
                                    audio_manager.play_sfx(SoundId::MenuChange);
                                    selection_changed_this_frame = true;
                                    state.selection_animation_timer = 0.0;
                                }
                                state.nav_key_held_direction = Some(NavDirection::Up);
                                state.nav_key_held_since = Some(Instant::now());
                                state.nav_key_last_scrolled_at = Some(Instant::now());
                            }
                        }
                        VirtualKeyCode::Right | VirtualKeyCode::Down => {
                            if num_entries > 0 {
                                let old_index = state.selected_index;
                                state.selected_index = (state.selected_index + 1) % num_entries;
                                if state.selected_index != old_index {
                                    audio_manager.play_sfx(SoundId::MenuChange);
                                    selection_changed_this_frame = true;
                                    state.selection_animation_timer = 0.0;
                                }
                                state.nav_key_held_direction = Some(NavDirection::Down);
                                state.nav_key_held_since = Some(Instant::now());
                                state.nav_key_last_scrolled_at = Some(Instant::now());
                            }
                        }
                        VirtualKeyCode::Enter => {
                            if num_entries > 0 {
                                if let Some(entry_clone) = state.entries.get(state.selected_index).cloned() {
                                    match entry_clone {
                                        MusicWheelEntry::Song(selected_song_arc) => {
                                            audio_manager.play_sfx(SoundId::MenuStart);
                                            return (Some(AppState::Gameplay), selection_changed_this_frame);
                                        }
                                        MusicWheelEntry::PackHeader { name: pack_name_str, .. } => {
                                            audio_manager.play_sfx(SoundId::MenuChange);
                                            if state.expanded_pack_name.as_ref() == Some(&pack_name_str) {
                                                state.expanded_pack_name = None;
                                            } else {
                                                state.expanded_pack_name = Some(pack_name_str.clone());
                                            }
                                            selection_changed_this_frame = true;
                                            state.selection_animation_timer = 0.0;
                                        }
                                    }
                                }
                            }
                        }
                        VirtualKeyCode::Escape => {
                            return (Some(AppState::Menu), selection_changed_this_frame);
                        }
                    }
                }
            }
            ElementState::Released => {
                match virtual_keycode {
                    VirtualKeyCode::Left | VirtualKeyCode::Up => {
                        if state.nav_key_held_direction == Some(NavDirection::Up) {
                            state.nav_key_held_direction = None;
                            state.nav_key_held_since = None;
                            state.nav_key_last_scrolled_at = None;
                        }
                    }
                    VirtualKeyCode::Right | VirtualKeyCode::Down => {
                        if state.nav_key_held_direction == Some(NavDirection::Down) {
                            state.nav_key_held_direction = None;
                            state.nav_key_held_since = None;
                            state.nav_key_last_scrolled_at = None;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    (None, selection_changed_this_frame)
}

pub fn update(state: &mut SelectMusicState, dt: f32, audio_manager: &AudioManager) -> bool {
    let mut selection_changed_by_update = false;
    const ANIMATION_CYCLE_DURATION: f32 = 1.0;
    state.selection_animation_timer += dt;
    if state.selection_animation_timer > ANIMATION_CYCLE_DURATION {
        state.selection_animation_timer -= ANIMATION_CYCLE_DURATION;
    }

    const INITIAL_HOLD_DELAY: Duration = Duration::from_millis(300);
    const REPEAT_SCROLL_INTERVAL: Duration = Duration::from_millis(70);

    if let (Some(direction), Some(held_since), Some(last_scrolled_at)) = (
        state.nav_key_held_direction,
        state.nav_key_held_since,
        state.nav_key_last_scrolled_at,
    ) {
        let now = Instant::now();
        if now.duration_since(held_since) > INITIAL_HOLD_DELAY {
            if now.duration_since(last_scrolled_at) >= REPEAT_SCROLL_INTERVAL {
                let num_entries = state.entries.len();
                if num_entries > 0 {
                    let old_index = state.selected_index;
                    match direction {
                        NavDirection::Up => {
                            state.selected_index = if state.selected_index == 0 {
                                num_entries - 1
                            } else {
                                state.selected_index - 1
                            };
                        }
                        NavDirection::Down => {
                            state.selected_index = (state.selected_index + 1) % num_entries;
                        }
                    }
                    if state.selected_index != old_index {
                         audio_manager.play_sfx(SoundId::MenuChange);
                         selection_changed_by_update = true;
                         state.selection_animation_timer = 0.0;
                    }
                    state.nav_key_last_scrolled_at = Some(now);
                }
            }
        }
    }
    selection_changed_by_update
}


pub fn draw(
    renderer: &Renderer,
    state: &SelectMusicState,
    assets: &AssetManager,
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
) {
    let header_footer_font = assets.get_font(FontId::Wendy).expect("Wendy font missing");
    let list_font = assets.get_font(FontId::Miso).expect("Miso font missing");
    let (window_width, window_height) = renderer.window_size();
    let center_x = window_width / 2.0;

    // Layout constants (reference values)
    const TARGET_BAR_TEXT_VISUAL_PX_HEIGHT_AT_REF_RES: f32 = 34.0;
    const OBSERVED_PX_HEIGHT_AT_REF_FOR_30PX_TARGET_OLD_METHOD: f32 = 19.0; // empirical adjustment factor
    const ASCENDER_POSITIONING_ADJUSTMENT_FACTOR: f32 = 0.65; // empirical adjustment factor
    const HEADER_FOOTER_LETTER_SPACING_FACTOR: f32 = 0.90;

    // Box dimensions at reference resolution
    const PINK_BOX_REF_WIDTH: f32 = 625.0;
    const PINK_BOX_REF_HEIGHT: f32 = 90.0;
    const SMALL_UPPER_RIGHT_BOX_REF_WIDTH: f32 = 48.0;
    const SMALL_UPPER_RIGHT_BOX_REF_HEIGHT: f32 = 228.0;
    const LEFT_BOXES_REF_WIDTH: f32 = 429.0;
    const LEFT_BOX_REF_HEIGHT: f32 = 96.0;
    const TOPMOST_LEFT_BOX_REF_WIDTH: f32 = 263.0;
    const TOPMOST_LEFT_BOX_REF_HEIGHT: f32 = 26.0;
    const ARTIST_BPM_BOX_REF_WIDTH: f32 = 480.0;
    const ARTIST_BPM_BOX_REF_HEIGHT: f32 = 75.0;
    const FALLBACK_BANNER_REF_WIDTH: f32 = 480.0;
    const FALLBACK_BANNER_REF_HEIGHT: f32 = 188.0;

    // Music wheel layout
    const MUSIC_WHEEL_BOX_REF_WIDTH: f32 = 591.0;
    const MUSIC_WHEEL_BOX_REF_HEIGHT: f32 = 46.0;
    const NUM_MUSIC_WHEEL_BOXES: usize = 15;
    const CENTER_MUSIC_WHEEL_SLOT_INDEX: usize = 7;

    // Gaps at reference resolution
    const VERTICAL_GAP_PINK_TO_UPPER_REF: f32 = 7.0;
    const HORIZONTAL_GAP_LEFT_TO_RIGHT_REF: f32 = 3.0;
    const VERTICAL_GAP_BETWEEN_LEFT_BOXES_REF: f32 = 36.0;
    const VERTICAL_GAP_TOPLEFT_TO_TOPMOST_REF: f32 = 1.0;
    const VERTICAL_GAP_ARTIST_TO_BANNER_REF: f32 = 2.0;
    const MUSIC_WHEEL_VERTICAL_GAP_REF: f32 = 2.0;

    // Scale factors based on current window size vs reference resolution
    let width_scale_factor = window_width / config::LAYOUT_BOXES_REF_RES_WIDTH;
    let height_scale_factor = window_height / config::LAYOUT_BOXES_REF_RES_HEIGHT;

    // Scaled values for text nudging
    let bar_text_vertical_nudge_current = config::BAR_TEXT_VERTICAL_NUDGE_PX_AT_REF_RES * height_scale_factor;
    let music_wheel_text_vertical_nudge_current = config::MUSIC_WHEEL_TEXT_VERTICAL_NUDGE_PX_AT_REF_RES * height_scale_factor;

    // Scaled dimensions for UI elements
    let music_wheel_box_current_width = MUSIC_WHEEL_BOX_REF_WIDTH * width_scale_factor;
    let music_wheel_box_current_height = MUSIC_WHEEL_BOX_REF_HEIGHT * height_scale_factor;
    let music_wheel_vertical_gap_current = MUSIC_WHEEL_VERTICAL_GAP_REF * height_scale_factor;

    let bar_height = (window_height / config::UI_REFERENCE_HEIGHT) * config::UI_BAR_REFERENCE_HEIGHT;
    let footer_y_top_edge = window_height - bar_height;

    let pink_box_current_width = PINK_BOX_REF_WIDTH * width_scale_factor;
    let pink_box_current_height = PINK_BOX_REF_HEIGHT * height_scale_factor;
    let small_upper_right_box_current_width = SMALL_UPPER_RIGHT_BOX_REF_WIDTH * width_scale_factor;
    let small_upper_right_box_current_height = SMALL_UPPER_RIGHT_BOX_REF_HEIGHT * height_scale_factor;
    let left_boxes_current_width = LEFT_BOXES_REF_WIDTH * width_scale_factor;
    let left_box_current_height = LEFT_BOX_REF_HEIGHT * height_scale_factor;
    let topmost_left_box_current_width = TOPMOST_LEFT_BOX_REF_WIDTH * width_scale_factor;
    let topmost_left_box_current_height = TOPMOST_LEFT_BOX_REF_HEIGHT * height_scale_factor;
    let artist_bpm_box_current_width = ARTIST_BPM_BOX_REF_WIDTH * width_scale_factor;
    let artist_bpm_box_current_height = ARTIST_BPM_BOX_REF_HEIGHT * height_scale_factor;
    let fallback_banner_current_width = FALLBACK_BANNER_REF_WIDTH * width_scale_factor;
    let fallback_banner_current_height = FALLBACK_BANNER_REF_HEIGHT * height_scale_factor;

    // Scaled gaps
    let vertical_gap_pink_to_upper_current = VERTICAL_GAP_PINK_TO_UPPER_REF * height_scale_factor;
    let horizontal_gap_left_to_right_current = HORIZONTAL_GAP_LEFT_TO_RIGHT_REF * width_scale_factor;
    let vertical_gap_between_left_boxes_current = VERTICAL_GAP_BETWEEN_LEFT_BOXES_REF * height_scale_factor;
    let vertical_gap_topleft_to_topmost_current = VERTICAL_GAP_TOPLEFT_TO_TOPMOST_REF * height_scale_factor;
    let vertical_gap_artist_to_banner_current = VERTICAL_GAP_ARTIST_TO_BANNER_REF * height_scale_factor;
    let song_text_left_padding_current = config::MUSIC_WHEEL_SONG_TEXT_LEFT_PADDING_REF * width_scale_factor;
    let vertical_gap_topmost_to_artist_box_current = config::VERTICAL_GAP_TOPMOST_TO_ARTIST_BOX_REF * height_scale_factor;

    // Scaled text sizes for detail area
    let detail_header_text_target_current_px_height = config::DETAIL_HEADER_TEXT_TARGET_PX_HEIGHT_AT_REF_RES * height_scale_factor;
    let detail_value_text_target_current_px_height = config::DETAIL_VALUE_TEXT_TARGET_PX_HEIGHT_AT_REF_RES * height_scale_factor;
    // Scaled padding for detail area
    let artist_header_left_padding_current = config::ARTIST_HEADER_LEFT_PADDING_REF * width_scale_factor;
    let artist_header_top_padding_current = config::ARTIST_HEADER_TOP_PADDING_REF * height_scale_factor;
    let bpm_header_left_padding_current = config::BPM_HEADER_LEFT_PADDING_REF * width_scale_factor;
    let header_to_value_horizontal_gap_current = config::HEADER_TO_VALUE_HORIZONTAL_GAP_REF * width_scale_factor;
    let bpm_to_length_horizontal_gap_current = config::BPM_TO_LENGTH_HORIZONTAL_GAP_REF * width_scale_factor;
    let artist_to_bpm_vertical_gap_current = config::ARTIST_TO_BPM_VERTICAL_GAP_REF * height_scale_factor;

    // Calculate music wheel stack position
    let total_music_boxes_height = NUM_MUSIC_WHEEL_BOXES as f32 * music_wheel_box_current_height;
    let total_music_gaps_height = (NUM_MUSIC_WHEEL_BOXES.saturating_sub(1)) as f32 * music_wheel_vertical_gap_current;
    let full_music_wheel_stack_height = total_music_boxes_height + total_music_gaps_height;
    let music_wheel_stack_top_y = (window_height - full_music_wheel_stack_height) / 2.0;

    // Calculate music wheel box positions
    let music_box_right_x = window_width; // Align to the right edge of the window
    let music_box_left_x = music_box_right_x - music_wheel_box_current_width;
    let music_box_center_x = music_box_left_x + music_wheel_box_current_width / 2.0;

    // Font scaling for music wheel text
    let wheel_text_current_target_visual_height = config::MUSIC_WHEEL_TEXT_TARGET_PX_HEIGHT_AT_REF_RES * height_scale_factor;
    let wheel_font_typographic_height_normalized = (list_font.metrics.ascender - list_font.metrics.descender).max(1e-5); // Avoid div by zero
    let wheel_text_effective_scale = wheel_text_current_target_visual_height / wheel_font_typographic_height_normalized;

    // Selection animation
    const ANIMATION_CYCLE_DURATION: f32 = 1.0; // Duration of one full sin wave cycle
    let anim_t_unscaled = (state.selection_animation_timer / ANIMATION_CYCLE_DURATION) * PI * 2.0;
    let anim_t = (anim_t_unscaled.sin() + 1.0) / 2.0; // Results in a value from 0.0 to 1.0, oscillating

    // --- Draw Music Wheel Items ---
    for i in 0..NUM_MUSIC_WHEEL_BOXES {
        let current_box_top_y = music_wheel_stack_top_y + (i as f32 * (music_wheel_box_current_height + music_wheel_vertical_gap_current));
        let current_box_center_y = current_box_top_y + music_wheel_box_current_height / 2.0;

        let mut display_text = "".to_string();
        let mut current_box_color;
        let mut current_text_color = config::SONG_TEXT_COLOR; // Default text color
        let mut text_x_pos = music_box_left_x; // Default X, adjusted below for centering/padding

        let num_entries = state.entries.len();
        let is_selected_slot = i == CENTER_MUSIC_WHEEL_SLOT_INDEX;

        if num_entries > 0 {
            // Calculate the actual index in the `state.entries` list for this wheel slot
            let list_index_isize = (state.selected_index as isize + i as isize - CENTER_MUSIC_WHEEL_SLOT_INDEX as isize + num_entries as isize) % num_entries as isize;
            let list_index = if list_index_isize < 0 { (list_index_isize + num_entries as isize) as usize } else { list_index_isize as usize };

            if let Some(entry) = state.entries.get(list_index) {
                match entry {
                    MusicWheelEntry::Song(song_info_arc) => {
                        display_text = song_info_arc.title.clone();
                        current_box_color = if is_selected_slot { lerp_color(config::MUSIC_WHEEL_BOX_COLOR, config::SELECTED_SONG_BOX_COLOR, anim_t) } else { config::MUSIC_WHEEL_BOX_COLOR };
                        current_text_color = config::SONG_TEXT_COLOR;
                        text_x_pos = music_box_left_x + song_text_left_padding_current;
                    }
                    MusicWheelEntry::PackHeader { name: pack_name, color: pack_text_color_val } => {
                        display_text = pack_name.clone();
                        current_box_color = if is_selected_slot { lerp_color(config::PACK_HEADER_BOX_COLOR, config::SELECTED_PACK_HEADER_BOX_COLOR, anim_t) } else { config::PACK_HEADER_BOX_COLOR };
                        current_text_color = *pack_text_color_val;
                        // Center pack header text
                        let text_width_pixels = list_font.measure_text_normalized(&display_text) * wheel_text_effective_scale;
                        text_x_pos = music_box_left_x + (music_wheel_box_current_width - text_width_pixels) / 2.0;
                    }
                }
            } else {
                // Should not happen if list_index logic is correct, but good fallback
                current_box_color = config::MUSIC_WHEEL_BOX_COLOR;
            }
        } else {
            // No entries, draw empty boxes
            current_box_color = config::MUSIC_WHEEL_BOX_COLOR;
        }

        // Draw the box
        renderer.draw_quad(
            device, cmd_buf, DescriptorSetId::SolidColor,
            Vector3::new(music_box_center_x, current_box_center_y, 0.0),
            (music_wheel_box_current_width, music_wheel_box_current_height),
            Rad(0.0), current_box_color,
            [0.0,0.0], [1.0,1.0]
        );

        // Draw text if any
        if !display_text.is_empty() {
            let current_box_center_y_for_text = current_box_top_y + music_wheel_box_current_height / 2.0;
            // Calculate baseline for vertical centering of text within the box
            let scaled_ascender = list_font.metrics.ascender * wheel_text_effective_scale;
            let scaled_descender = list_font.metrics.descender * wheel_text_effective_scale; // Typically negative
            let mut text_baseline_y = current_box_center_y_for_text + (scaled_ascender + scaled_descender) / 2.0; // Center point for text
            text_baseline_y += music_wheel_text_vertical_nudge_current; // Apply nudge

            renderer.draw_text(
                device, cmd_buf, list_font, &display_text,
                text_x_pos, text_baseline_y,
                current_text_color, wheel_text_effective_scale, None
            );
        }
    }

    // --- Draw Header and Footer Bars ---
    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::SolidColor,
        Vector3::new(center_x, bar_height / 2.0, 0.0), // Center of header bar
        (window_width, bar_height), Rad(0.0), config::UI_BAR_COLOR,
        [0.0,0.0], [1.0,1.0]
    );
    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::SolidColor,
        Vector3::new(center_x, footer_y_top_edge + bar_height / 2.0, 0.0), // Center of footer bar
        (window_width, bar_height), Rad(0.0), config::UI_BAR_COLOR,
        [0.0,0.0], [1.0,1.0]
    );

    // --- Draw Pink Box (bottom left) ---
    let pink_box_left_x = 0.0; // Align to left edge
    let pink_box_top_y = footer_y_top_edge - pink_box_current_height;
    let pink_box_center_x = pink_box_left_x + pink_box_current_width / 2.0;
    let pink_box_center_y = pink_box_top_y + pink_box_current_height / 2.0;
    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::SolidColor,
        Vector3::new(pink_box_center_x, pink_box_center_y, 0.0),
        (pink_box_current_width, pink_box_current_height),
        Rad(0.0), config::PINK_BOX_COLOR,
        [0.0,0.0], [1.0,1.0]
    );

    // --- Draw Small Upper Right Box (relative to pink box) ---
    let small_upper_right_box_top_y = pink_box_top_y - vertical_gap_pink_to_upper_current - small_upper_right_box_current_height;
    let small_upper_right_box_left_x = pink_box_left_x + pink_box_current_width - small_upper_right_box_current_width; // Aligned to the right edge of the pink box
    let small_upper_right_box_center_x = small_upper_right_box_left_x + small_upper_right_box_current_width / 2.0;
    let small_upper_right_box_center_y = small_upper_right_box_top_y + small_upper_right_box_current_height / 2.0;
    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::SolidColor,
        Vector3::new(small_upper_right_box_center_x, small_upper_right_box_center_y, 0.0),
        (small_upper_right_box_current_width, small_upper_right_box_current_height),
        Rad(0.0), config::UI_BOX_DARK_COLOR,
        [0.0,0.0], [1.0,1.0]
    );

    // --- Draw Bottom Left Box (to the left of small_upper_right_box) ---
    let bottom_left_box_top_y = pink_box_top_y - vertical_gap_pink_to_upper_current - left_box_current_height; // Align top edge with small_upper_right_box
    let bottom_left_box_left_x = small_upper_right_box_left_x - horizontal_gap_left_to_right_current - left_boxes_current_width;
    let bottom_left_box_center_x = bottom_left_box_left_x + left_boxes_current_width / 2.0;
    let bottom_left_box_center_y = bottom_left_box_top_y + left_box_current_height / 2.0;
    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::SolidColor,
        Vector3::new(bottom_left_box_center_x, bottom_left_box_center_y, 0.0),
        (left_boxes_current_width, left_box_current_height),
        Rad(0.0), config::UI_BOX_DARK_COLOR,
        [0.0,0.0], [1.0,1.0]
    );

    // --- Top Left Box (Density Graph Area) ---
    let graph_area_left_x = bottom_left_box_left_x; // Align with box below
    let graph_area_width = left_boxes_current_width;
    let graph_area_height = left_box_current_height; // Use same height as other left boxes
    let graph_area_top_y = bottom_left_box_top_y - vertical_gap_between_left_boxes_current - graph_area_height;
    let graph_area_center_x = graph_area_left_x + graph_area_width / 2.0;
    let graph_area_center_y = graph_area_top_y + graph_area_height / 2.0;

    // Draw background for the graph area (always, even if no graph texture)
    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::SolidColor,
        Vector3::new(graph_area_center_x, graph_area_center_y, 0.0),
        (graph_area_width, graph_area_height), Rad(0.0), config::UI_BOX_DARK_COLOR,
        [0.0,0.0], [1.0,1.0]
    );

    // If a graph texture exists (i.e., a song is selected and graph was generated), draw it on top of the background
    if state.current_graph_texture.is_some() {
        renderer.draw_quad(
            device, cmd_buf, DescriptorSetId::NpsGraph,
            Vector3::new(graph_area_center_x, graph_area_center_y, 0.0),
            (graph_area_width, graph_area_height), // Stretch texture to fit the box
            Rad(0.0), [1.0, 1.0, 1.0, 1.0], // White tint for opaque texture
            [0.0, 0.0], [1.0, 1.0] // Full UVs
        );
    }


    // --- Draw Topmost Left Box (above density graph) ---
    let topmost_left_box_top_y = graph_area_top_y - vertical_gap_topleft_to_topmost_current - topmost_left_box_current_height;
    let topmost_left_box_left_x = graph_area_left_x; // Align with graph area
    let topmost_left_box_center_x = topmost_left_box_left_x + topmost_left_box_current_width / 2.0;
    let topmost_left_box_center_y = topmost_left_box_top_y + topmost_left_box_current_height / 2.0;
    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::SolidColor,
        Vector3::new(topmost_left_box_center_x, topmost_left_box_center_y, 0.0),
        (topmost_left_box_current_width, topmost_left_box_current_height),
        Rad(0.0), config::PINK_BOX_COLOR,
        [0.0,0.0], [1.0,1.0]
    );

    // --- Draw Artist/BPM Box (above topmost left box) ---
    let artist_bpm_box_left_x = topmost_left_box_left_x; // Align left edge
    let artist_bpm_box_actual_top_y = topmost_left_box_top_y - vertical_gap_topmost_to_artist_box_current - artist_bpm_box_current_height;
    let artist_bpm_box_center_x = artist_bpm_box_left_x + artist_bpm_box_current_width / 2.0;
    let artist_bpm_box_center_y = artist_bpm_box_actual_top_y + artist_bpm_box_current_height / 2.0;
    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::SolidColor,
        Vector3::new(artist_bpm_box_center_x, artist_bpm_box_center_y, 0.0),
        (artist_bpm_box_current_width, artist_bpm_box_current_height),
        Rad(0.0), config::UI_BOX_DARK_COLOR,
        [0.0,0.0], [1.0,1.0]
    );

    // --- Draw Fallback/Dynamic Banner ---
    let fallback_banner_left_x = artist_bpm_box_left_x; // Align left edge
    let fallback_banner_width_to_draw = fallback_banner_current_width; // Could be dynamic later
    let fallback_banner_actual_top_y = artist_bpm_box_actual_top_y - vertical_gap_artist_to_banner_current - fallback_banner_current_height;
    let fallback_banner_center_x = fallback_banner_left_x + fallback_banner_width_to_draw / 2.0;
    let fallback_banner_center_y = fallback_banner_actual_top_y + fallback_banner_current_height / 2.0;
    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::DynamicBanner, // Use DynamicBanner set ID
        Vector3::new(fallback_banner_center_x, fallback_banner_center_y, 0.0),
        (fallback_banner_width_to_draw, fallback_banner_current_height),
        Rad(0.0), [1.0, 1.0, 1.0, 1.0], // Tint (usually white for textures)
        [0.0,0.0], [1.0,1.0] // Full UVs
    );

    // --- Draw Header and Footer Text ---
    // Calculate effective scale for header/footer font to match target visual height
    let hf_target_visual_current_px_height = TARGET_BAR_TEXT_VISUAL_PX_HEIGHT_AT_REF_RES * height_scale_factor;
    let hf_font_typographic_height_normalized = (header_footer_font.metrics.ascender - header_footer_font.metrics.descender).max(1e-5);
    let base_scale_for_typographic_height = hf_target_visual_current_px_height / hf_font_typographic_height_normalized;
    // Apply empirical adjustment for visual height vs typographic height
    let height_adjustment_factor = if OBSERVED_PX_HEIGHT_AT_REF_FOR_30PX_TARGET_OLD_METHOD > 1e-5 {
        TARGET_BAR_TEXT_VISUAL_PX_HEIGHT_AT_REF_RES / OBSERVED_PX_HEIGHT_AT_REF_FOR_30PX_TARGET_OLD_METHOD
    } else { 1.0 };
    let hf_effective_scale = base_scale_for_typographic_height * height_adjustment_factor;

    // Position text vertically within bars
    let hf_scaled_ascender_metric = header_footer_font.metrics.ascender * hf_effective_scale;
    let hf_scaled_ascender_for_positioning = hf_scaled_ascender_metric * ASCENDER_POSITIONING_ADJUSTMENT_FACTOR;
    let hf_empty_vertical_space = (bar_height - hf_target_visual_current_px_height).max(0.0);
    let hf_padding_from_bar_top_to_text_visual_top = hf_empty_vertical_space / 2.0;

    let mut header_baseline_y = hf_padding_from_bar_top_to_text_visual_top + hf_scaled_ascender_for_positioning;
    let mut footer_baseline_y = footer_y_top_edge + hf_padding_from_bar_top_to_text_visual_top + hf_scaled_ascender_for_positioning;
    header_baseline_y += bar_text_vertical_nudge_current; // Apply nudge
    footer_baseline_y += bar_text_vertical_nudge_current; // Apply nudge

    let header_text_left_padding_px = 14.0 * width_scale_factor;
    let header_text_str = "SELECT MUSIC";
    renderer.draw_text(
        device, cmd_buf, header_footer_font, header_text_str,
        header_text_left_padding_px, header_baseline_y,
        config::UI_BAR_TEXT_COLOR, hf_effective_scale, Some(HEADER_FOOTER_LETTER_SPACING_FACTOR)
    );

    let footer_text_str = "EVENT MODE";
    let footer_text_glyph_width = header_footer_font.measure_text_normalized(footer_text_str) * hf_effective_scale;
    let num_chars = footer_text_str.chars().count();
    let footer_text_visual_width = if num_chars > 1 {
        footer_text_glyph_width * (1.0 + (HEADER_FOOTER_LETTER_SPACING_FACTOR - 1.0) * ((num_chars -1 ) as f32 / num_chars as f32) )
    } else { footer_text_glyph_width };
    renderer.draw_text(
        device, cmd_buf, header_footer_font, footer_text_str,
        center_x - footer_text_visual_width / 2.0, footer_baseline_y,
        config::UI_BAR_TEXT_COLOR, hf_effective_scale, Some(HEADER_FOOTER_LETTER_SPACING_FACTOR)
    );

    // --- Draw Song Details (Artist, BPM, Length) if a song is selected ---
    if let Some(MusicWheelEntry::Song(selected_song_arc)) = state.entries.get(state.selected_index) {
        let detail_header_font_typographic_h_norm = (list_font.metrics.ascender - list_font.metrics.descender).max(1e-5);
        let detail_header_effective_scale = detail_header_text_target_current_px_height / detail_header_font_typographic_h_norm;
        let detail_value_effective_scale = detail_value_text_target_current_px_height / detail_header_font_typographic_h_norm;

        // First row: Artist
        let first_row_visual_top_y_from_box_top = artist_header_top_padding_current;

        let artist_header_str = "ARTIST";
        let artist_header_width = list_font.measure_text_normalized(artist_header_str) * detail_header_effective_scale;
        let artist_header_x = artist_bpm_box_left_x + artist_header_left_padding_current;
        let artist_header_baseline_y = artist_bpm_box_actual_top_y + first_row_visual_top_y_from_box_top + (list_font.metrics.ascender * detail_header_effective_scale) + music_wheel_text_vertical_nudge_current;
        renderer.draw_text(
            device, cmd_buf, list_font, artist_header_str,
            artist_header_x, artist_header_baseline_y,
            config::DETAIL_HEADER_TEXT_COLOR, detail_header_effective_scale, None
        );

        let artist_value_str = &selected_song_arc.artist;
        let artist_value_x = artist_header_x + artist_header_width + header_to_value_horizontal_gap_current;
        let artist_value_baseline_y = artist_bpm_box_actual_top_y + first_row_visual_top_y_from_box_top + (list_font.metrics.ascender * detail_value_effective_scale) + music_wheel_text_vertical_nudge_current;
        renderer.draw_text(
            device, cmd_buf, list_font, artist_value_str,
            artist_value_x, artist_value_baseline_y,
            config::SONG_TEXT_COLOR, detail_value_effective_scale, None
        );

        // Second row: BPM and Length
        let advance_for_next_line = detail_value_text_target_current_px_height; // Using value height for line spacing

        let bpm_header_str = "BPM";
        let bpm_header_width = list_font.measure_text_normalized(bpm_header_str) * detail_header_effective_scale;
        let bpm_header_x = artist_bpm_box_left_x + bpm_header_left_padding_current; // Re-using for alignment
        let bpm_header_baseline_y = artist_value_baseline_y + advance_for_next_line + artist_to_bpm_vertical_gap_current;
        renderer.draw_text(
            device, cmd_buf, list_font, bpm_header_str,
            bpm_header_x, bpm_header_baseline_y,
            config::DETAIL_HEADER_TEXT_COLOR, detail_header_effective_scale, None
        );

        let bpm_value_str = if selected_song_arc.bpms_header.len() == 1 {
                                format!("{:.0}", selected_song_arc.bpms_header[0].1)
                            } else if !selected_song_arc.bpms_header.is_empty() {
                                let min_bpm = selected_song_arc.bpms_header.iter().map(|&(_, bpm)| bpm).fold(f32::INFINITY, f32::min);
                                let max_bpm = selected_song_arc.bpms_header.iter().map(|&(_, bpm)| bpm).fold(f32::NEG_INFINITY, f32::max);
                                if (min_bpm - max_bpm).abs() < 0.1 { // Consider them same if very close
                                    format!("{:.0}", min_bpm)
                                } else {
                                    format!("{:.0}-{:.0}", min_bpm, max_bpm)
                                }
                            } else {
                                "???".to_string()
                            };
        let bpm_value_x = bpm_header_x + bpm_header_width + header_to_value_horizontal_gap_current;
        let bpm_value_baseline_y = bpm_header_baseline_y; // Same baseline as its header
        renderer.draw_text(
            device, cmd_buf, list_font, &bpm_value_str,
            bpm_value_x, bpm_value_baseline_y,
            config::SONG_TEXT_COLOR, detail_value_effective_scale, None
        );

        let length_header_str = "LENGTH";
        let length_value_str = selected_song_arc.charts.iter()
            .find_map(|c| c.calculated_length_sec)
            .map_or_else(
                || "??:??".to_string(),
                |secs| format_duration_flexible(secs)
            );

        let length_header_width = list_font.measure_text_normalized(length_header_str) * detail_header_effective_scale;
        let length_header_x = bpm_header_x + bpm_header_width + bpm_to_length_horizontal_gap_current; // Horizontal offset from BPM group
        let length_value_x = length_header_x + length_header_width + header_to_value_horizontal_gap_current;
        let length_header_baseline_y = bpm_header_baseline_y; // Same row as BPM
        let length_value_baseline_y = bpm_value_baseline_y;   // Same row as BPM value

        renderer.draw_text(
            device, cmd_buf, list_font, length_header_str,
            length_header_x, length_header_baseline_y,
            config::DETAIL_HEADER_TEXT_COLOR, detail_header_effective_scale, None
        );
        renderer.draw_text(
            device, cmd_buf, list_font, &length_value_str,
            length_value_x, length_value_baseline_y,
            config::SONG_TEXT_COLOR, detail_value_effective_scale, None
        );
    }
}