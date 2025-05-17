use crate::assets::{AssetManager, FontId, SoundId};
use crate::audio::AudioManager;
use crate::config;
use crate::graphics::renderer::{DescriptorSetId, Renderer};
use crate::state::{AppState, SelectMusicState, VirtualKeyCode, MusicWheelEntry};
use ash::vk;
use cgmath::{Rad, Vector3};
use log::debug;
use winit::event::{ElementState, KeyEvent};

pub fn handle_input(
    key_event: &KeyEvent,
    state: &mut SelectMusicState,
    audio_manager: &AudioManager,
) -> (Option<AppState>, bool) { 
    let mut selection_changed = false;

    if key_event.state == ElementState::Pressed && !key_event.repeat {
        if let Some(virtual_keycode) =
            crate::state::key_to_virtual_keycode(key_event.logical_key.clone())
        {
            let num_entries = state.entries.len(); 

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
                            selection_changed = true;
                        }
                        debug!("SelectMusic {:?}: Selected index {}", virtual_keycode, state.selected_index);
                    }
                }
                VirtualKeyCode::Right | VirtualKeyCode::Down => { 
                    if num_entries > 0 {
                        let old_index = state.selected_index;
                        state.selected_index = (state.selected_index + 1) % num_entries;
                        if state.selected_index != old_index {
                            audio_manager.play_sfx(SoundId::MenuChange);
                            selection_changed = true;
                        }
                        debug!("SelectMusic {:?}: Selected index {}", virtual_keycode, state.selected_index);
                    }
                }
                VirtualKeyCode::Enter => {
                    if num_entries > 0 {
                        if let Some(entry_clone) = state.entries.get(state.selected_index).cloned() {
                            match entry_clone {
                                MusicWheelEntry::Song(selected_song_arc) => {
                                    debug!(
                                        "SelectMusic Enter: Attempting to start song '{}' at index {}",
                                        selected_song_arc.title, state.selected_index
                                    );
                                    audio_manager.play_sfx(SoundId::MenuStart);
                                    return (Some(AppState::Gameplay), selection_changed); 
                                }
                                MusicWheelEntry::PackHeader(pack_name) => {
                                    audio_manager.play_sfx(SoundId::MenuChange); 
                                    if state.expanded_pack_name.as_ref() == Some(&pack_name) {
                                        state.expanded_pack_name = None; 
                                        debug!("Collapsing pack: {}", pack_name);
                                    } else {
                                        state.expanded_pack_name = Some(pack_name.clone()); 
                                        debug!("Expanding pack: {}", pack_name);
                                    }
                                    selection_changed = true; 
                                }
                            }
                        }
                    }
                }
                VirtualKeyCode::Escape => {
                    debug!("SelectMusic Escape: Returning to Main Menu");
                    return (Some(AppState::Menu), selection_changed); 
                }
                _ => {}
            }
        }
    }
    (None, selection_changed)
}

pub fn update(_state: &mut SelectMusicState, _dt: f32) {}


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

    // --- Constants (same as before) ---
    const TARGET_BAR_TEXT_VISUAL_PX_HEIGHT_AT_REF_RES: f32 = 36.0;
    const OBSERVED_PX_HEIGHT_AT_REF_FOR_30PX_TARGET_OLD_METHOD: f32 = 19.0;
    const ASCENDER_POSITIONING_ADJUSTMENT_FACTOR: f32 = 0.65;
    const HEADER_FOOTER_LETTER_SPACING_FACTOR: f32 = 0.90;
    const MUSIC_WHEEL_TEXT_TARGET_PX_HEIGHT_AT_REF_RES: f32 = 22.0;
    const TEXT_VERTICAL_NUDGE_PX_AT_REF_RES: f32 = 2.0;
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
    const MUSIC_WHEEL_BOX_REF_WIDTH: f32 = 591.0;
    const MUSIC_WHEEL_BOX_REF_HEIGHT: f32 = 46.0;
    const NUM_MUSIC_WHEEL_BOXES: usize = 15;
    const CENTER_MUSIC_WHEEL_SLOT_INDEX: usize = 7;
    const VERTICAL_GAP_PINK_TO_UPPER_REF: f32 = 7.0;
    const HORIZONTAL_GAP_LEFT_TO_RIGHT_REF: f32 = 3.0;
    const VERTICAL_GAP_BETWEEN_LEFT_BOXES_REF: f32 = 36.0;
    const VERTICAL_GAP_TOPLEFT_TO_TOPMOST_REF: f32 = 1.0;
    const VERTICAL_GAP_STEPARTIST_TO_ARTIST_REF: f32 = 5.0;
    const VERTICAL_GAP_ARTIST_TO_BANNER_REF: f32 = 2.0;
    const MUSIC_WHEEL_VERTICAL_GAP_REF: f32 = 2.0;

    // --- Scaled Dimensions (same as before) ---
    let width_scale_factor = window_width / config::LAYOUT_BOXES_REF_RES_WIDTH;
    let height_scale_factor = window_height / config::LAYOUT_BOXES_REF_RES_HEIGHT;

    let text_vertical_nudge_current = TEXT_VERTICAL_NUDGE_PX_AT_REF_RES * height_scale_factor;
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
    let vertical_gap_pink_to_upper_current = VERTICAL_GAP_PINK_TO_UPPER_REF * height_scale_factor;
    let horizontal_gap_left_to_right_current = HORIZONTAL_GAP_LEFT_TO_RIGHT_REF * width_scale_factor;
    let vertical_gap_between_left_boxes_current = VERTICAL_GAP_BETWEEN_LEFT_BOXES_REF * height_scale_factor;
    let vertical_gap_topleft_to_topmost_current = VERTICAL_GAP_TOPLEFT_TO_TOPMOST_REF * height_scale_factor;
    let vertical_gap_stepartist_to_artist_current = VERTICAL_GAP_STEPARTIST_TO_ARTIST_REF * height_scale_factor;
    let vertical_gap_artist_to_banner_current = VERTICAL_GAP_ARTIST_TO_BANNER_REF * height_scale_factor;

    // --- Music Wheel Box and Text Drawing (main part to change) ---
    let total_music_boxes_height = NUM_MUSIC_WHEEL_BOXES as f32 * music_wheel_box_current_height;
    let total_music_gaps_height = (NUM_MUSIC_WHEEL_BOXES.saturating_sub(1)) as f32 * music_wheel_vertical_gap_current;
    let full_music_wheel_stack_height = total_music_boxes_height + total_music_gaps_height;
    let music_wheel_stack_top_y = (window_height - full_music_wheel_stack_height) / 2.0;
    let music_box_right_x = window_width;
    let music_box_left_x = music_box_right_x - music_wheel_box_current_width;
    let music_box_center_x = music_box_left_x + music_wheel_box_current_width / 2.0;

    let wheel_text_current_target_visual_height = MUSIC_WHEEL_TEXT_TARGET_PX_HEIGHT_AT_REF_RES * height_scale_factor;
    let wheel_font_typographic_height_normalized = (list_font.metrics.ascender - list_font.metrics.descender).max(1e-5);
    let wheel_text_effective_scale = wheel_text_current_target_visual_height / wheel_font_typographic_height_normalized;

    for i in 0..NUM_MUSIC_WHEEL_BOXES {
        let current_box_top_y = music_wheel_stack_top_y + (i as f32 * (music_wheel_box_current_height + music_wheel_vertical_gap_current));
        let current_box_center_y = current_box_top_y + music_wheel_box_current_height / 2.0;

        let mut display_text = "".to_string();
        let mut current_box_color = config::MUSIC_WHEEL_BOX_COLOR;
        let current_text_color = if i == CENTER_MUSIC_WHEEL_SLOT_INDEX { // text color still depends on selection
            config::MENU_SELECTED_COLOR
        } else {
            config::MENU_NORMAL_COLOR
        };

        let num_entries = state.entries.len();

        if num_entries > 0 {
            let list_index_isize = (state.selected_index as isize
                + i as isize
                - CENTER_MUSIC_WHEEL_SLOT_INDEX as isize
                + num_entries as isize) 
                % num_entries as isize; 
            
            let list_index = if list_index_isize < 0 {
                (list_index_isize + num_entries as isize) as usize
            } else {
                list_index_isize as usize
            };

            if let Some(entry) = state.entries.get(list_index) {
                match entry {
                    MusicWheelEntry::Song(song_info_arc) => {
                        display_text = song_info_arc.title.clone();
                        // Box color remains default MUSIC_WHEEL_BOX_COLOR
                    }
                    MusicWheelEntry::PackHeader(pack_name) => {
                        // REMOVED [+] and [-] indicators
                        display_text = format!("PACK: {}", pack_name);
                        current_box_color = config::PACK_HEADER_BOX_COLOR;
                    }
                }
            }
        }

        renderer.draw_quad(
            device, cmd_buf, DescriptorSetId::SolidColor,
            Vector3::new(music_box_center_x, current_box_center_y, 0.0),
            (music_wheel_box_current_width, music_wheel_box_current_height), Rad(0.0), current_box_color,
            [0.0,0.0], [1.0,1.0]
        );

        if !display_text.is_empty() {
            let text_width_pixels = list_font.measure_text_normalized(&display_text) * wheel_text_effective_scale;
            let text_x_pos = music_box_left_x + (music_wheel_box_current_width - text_width_pixels) / 2.0;

            let current_visual_height = wheel_font_typographic_height_normalized * wheel_text_effective_scale;
            let visual_text_top_y = current_box_center_y - (current_visual_height / 2.0);
            let mut text_baseline_y = visual_text_top_y + (list_font.metrics.ascender * wheel_text_effective_scale);
            
            text_baseline_y += text_vertical_nudge_current;

            renderer.draw_text(
                device, cmd_buf, list_font, &display_text,
                text_x_pos, text_baseline_y, current_text_color,
                wheel_text_effective_scale, None
            );
        }
    }

    // --- Draw other layout elements (Quads for UI boxes) ---
    // (This section remains the same as before)
    renderer.draw_quad( device, cmd_buf, DescriptorSetId::SolidColor, Vector3::new(center_x, bar_height / 2.0, 0.0), (window_width, bar_height), Rad(0.0), config::UI_BAR_COLOR, [0.0,0.0], [1.0,1.0]);
    renderer.draw_quad( device, cmd_buf, DescriptorSetId::SolidColor, Vector3::new(center_x, footer_y_top_edge + bar_height / 2.0, 0.0), (window_width, bar_height), Rad(0.0), config::UI_BAR_COLOR, [0.0,0.0], [1.0,1.0]);
    
    let pink_box_left_x = 0.0;
    let pink_box_right_x = pink_box_left_x + pink_box_current_width;
    let pink_box_top_y = footer_y_top_edge - pink_box_current_height;
    let pink_box_center_x = pink_box_left_x + pink_box_current_width / 2.0;
    let pink_box_center_y = pink_box_top_y + pink_box_current_height / 2.0;
    renderer.draw_quad(device, cmd_buf, DescriptorSetId::SolidColor, Vector3::new(pink_box_center_x, pink_box_center_y, 0.0), (pink_box_current_width, pink_box_current_height), Rad(0.0), config::PINK_BOX_COLOR, [0.0,0.0], [1.0,1.0] );
    
    let small_upper_right_box_bottom_y = pink_box_top_y - vertical_gap_pink_to_upper_current;
    let small_upper_right_box_top_y = small_upper_right_box_bottom_y - small_upper_right_box_current_height;
    let small_upper_right_box_right_x = pink_box_right_x;
    let small_upper_right_box_left_x = small_upper_right_box_right_x - small_upper_right_box_current_width;
    let small_upper_right_box_center_x = small_upper_right_box_left_x + small_upper_right_box_current_width / 2.0;
    let small_upper_right_box_center_y = small_upper_right_box_top_y + small_upper_right_box_current_height / 2.0;
    renderer.draw_quad(device, cmd_buf, DescriptorSetId::SolidColor, Vector3::new(small_upper_right_box_center_x, small_upper_right_box_center_y, 0.0), (small_upper_right_box_current_width, small_upper_right_box_current_height), Rad(0.0), config::UI_BOX_DARK_COLOR, [0.0,0.0], [1.0,1.0] );
    
    let bottom_left_box_right_x = small_upper_right_box_left_x - horizontal_gap_left_to_right_current;
    let bottom_left_box_left_x = bottom_left_box_right_x - left_boxes_current_width;
    let bottom_left_box_bottom_y = small_upper_right_box_bottom_y;
    let bottom_left_box_top_y = bottom_left_box_bottom_y - left_box_current_height;
    let bottom_left_box_center_x = bottom_left_box_left_x + left_boxes_current_width / 2.0;
    let bottom_left_box_center_y = bottom_left_box_top_y + left_box_current_height / 2.0;
    renderer.draw_quad(device, cmd_buf, DescriptorSetId::SolidColor, Vector3::new(bottom_left_box_center_x, bottom_left_box_center_y, 0.0), (left_boxes_current_width, left_box_current_height), Rad(0.0), config::UI_BOX_DARK_COLOR, [0.0,0.0], [1.0,1.0] );
    
    let top_left_box_left_x = bottom_left_box_left_x;
    let top_left_box_bottom_y = bottom_left_box_top_y - vertical_gap_between_left_boxes_current;
    let top_left_box_top_y = top_left_box_bottom_y - left_box_current_height;
    let top_left_box_center_x = top_left_box_left_x + left_boxes_current_width / 2.0;
    let top_left_box_center_y = top_left_box_top_y + left_box_current_height / 2.0;
    renderer.draw_quad(device, cmd_buf, DescriptorSetId::SolidColor, Vector3::new(top_left_box_center_x, top_left_box_center_y, 0.0), (left_boxes_current_width, left_box_current_height), Rad(0.0), config::TOP_LEFT_BOX_COLOR, [0.0,0.0], [1.0,1.0] );
    
    let topmost_left_box_left_x = top_left_box_left_x;
    let topmost_left_box_bottom_y = top_left_box_top_y - vertical_gap_topleft_to_topmost_current;
    let topmost_left_box_top_y = topmost_left_box_bottom_y - topmost_left_box_current_height;
    let topmost_left_box_center_x = topmost_left_box_left_x + topmost_left_box_current_width / 2.0;
    let topmost_left_box_center_y = topmost_left_box_top_y + topmost_left_box_current_height / 2.0;
    renderer.draw_quad(device, cmd_buf, DescriptorSetId::SolidColor, Vector3::new(topmost_left_box_center_x, topmost_left_box_center_y, 0.0), (topmost_left_box_current_width, topmost_left_box_current_height), Rad(0.0), config::PINK_BOX_COLOR, [0.0,0.0], [1.0,1.0] );
    
    let artist_bpm_box_left_x = topmost_left_box_left_x;
    let artist_bpm_box_bottom_y = topmost_left_box_top_y - vertical_gap_stepartist_to_artist_current;
    let artist_bpm_box_top_y = artist_bpm_box_bottom_y - artist_bpm_box_current_height;
    let artist_bpm_box_center_x = artist_bpm_box_left_x + artist_bpm_box_current_width / 2.0;
    let artist_bpm_box_center_y = artist_bpm_box_top_y + artist_bpm_box_current_height / 2.0;
    renderer.draw_quad(device, cmd_buf, DescriptorSetId::SolidColor, Vector3::new(artist_bpm_box_center_x, artist_bpm_box_center_y, 0.0), (artist_bpm_box_current_width, artist_bpm_box_current_height), Rad(0.0), config::UI_BOX_DARK_COLOR, [0.0,0.0], [1.0,1.0] );
    
    let fallback_banner_left_x = artist_bpm_box_left_x;
    let fallback_banner_width_to_draw = fallback_banner_current_width;
    let fallback_banner_bottom_y = artist_bpm_box_top_y - vertical_gap_artist_to_banner_current;
    let fallback_banner_top_y = fallback_banner_bottom_y - fallback_banner_current_height;
    let fallback_banner_center_x = fallback_banner_left_x + fallback_banner_width_to_draw / 2.0;
    let fallback_banner_center_y = fallback_banner_top_y + fallback_banner_current_height / 2.0;
    
    renderer.draw_quad( device, cmd_buf, DescriptorSetId::DynamicBanner, Vector3::new(fallback_banner_center_x, fallback_banner_center_y, 0.0), (fallback_banner_width_to_draw, fallback_banner_current_height), Rad(0.0), [1.0, 1.0, 1.0, 1.0], [0.0,0.0], [1.0,1.0] );

    // --- Header and Footer Text Drawing (same as before) ---
    let hf_target_visual_current_px_height = TARGET_BAR_TEXT_VISUAL_PX_HEIGHT_AT_REF_RES * height_scale_factor;
    let hf_font_typographic_height_normalized = (header_footer_font.metrics.ascender - header_footer_font.metrics.descender).max(1e-5);

    let base_scale_for_typographic_height = hf_target_visual_current_px_height / hf_font_typographic_height_normalized;
    let height_adjustment_factor = if OBSERVED_PX_HEIGHT_AT_REF_FOR_30PX_TARGET_OLD_METHOD > 1e-5 {
        TARGET_BAR_TEXT_VISUAL_PX_HEIGHT_AT_REF_RES / OBSERVED_PX_HEIGHT_AT_REF_FOR_30PX_TARGET_OLD_METHOD
    } else {
        1.0
    };
    let hf_effective_scale = base_scale_for_typographic_height * height_adjustment_factor;

    let hf_scaled_ascender_metric = header_footer_font.metrics.ascender * hf_effective_scale;
    let hf_scaled_ascender_for_positioning = hf_scaled_ascender_metric * ASCENDER_POSITIONING_ADJUSTMENT_FACTOR;
    let hf_empty_vertical_space = (bar_height - hf_target_visual_current_px_height).max(0.0);
    let hf_padding_from_bar_top_to_text_visual_top = hf_empty_vertical_space / 2.0;

    let mut header_baseline_y = hf_padding_from_bar_top_to_text_visual_top + hf_scaled_ascender_for_positioning;
    let mut footer_baseline_y = footer_y_top_edge + hf_padding_from_bar_top_to_text_visual_top + hf_scaled_ascender_for_positioning;

    header_baseline_y += text_vertical_nudge_current;
    footer_baseline_y += text_vertical_nudge_current;

    let header_text_left_padding_px = 14.0 * width_scale_factor;
    let header_text_str = "SELECT MUSIC";
    renderer.draw_text( device, cmd_buf, header_footer_font, header_text_str, header_text_left_padding_px, header_baseline_y, config::UI_BAR_TEXT_COLOR, hf_effective_scale, Some(HEADER_FOOTER_LETTER_SPACING_FACTOR) );

    let footer_text_str = "EVENT MODE";
    let footer_text_glyph_width = header_footer_font.measure_text_normalized(footer_text_str) * hf_effective_scale;
    let num_chars = footer_text_str.chars().count();
    let footer_text_visual_width = if num_chars > 1 {
        footer_text_glyph_width * (1.0 + (HEADER_FOOTER_LETTER_SPACING_FACTOR - 1.0) * ((num_chars -1 ) as f32 / num_chars as f32) )
    } else {
        footer_text_glyph_width
    };
    renderer.draw_text( device, cmd_buf, header_footer_font, footer_text_str, center_x - footer_text_visual_width / 2.0, footer_baseline_y, config::UI_BAR_TEXT_COLOR, hf_effective_scale, Some(HEADER_FOOTER_LETTER_SPACING_FACTOR) );

    // --- Draw Artist/BPM (same as before) ---
    if let Some(MusicWheelEntry::Song(selected_song_arc)) = state.entries.get(state.selected_index) {
        let detail_text_target_px_height = MUSIC_WHEEL_TEXT_TARGET_PX_HEIGHT_AT_REF_RES * 0.8 * height_scale_factor;
        let detail_font_typographic_height_normalized = (list_font.metrics.ascender - list_font.metrics.descender).max(1e-5);
        let detail_text_effective_scale = detail_text_target_px_height / detail_font_typographic_height_normalized;
        
        let artist_text_full = format!("Artist: {}", selected_song_arc.artist);
        let artist_bpm_box_padding_x = 10.0 * width_scale_factor;
        let artist_text_x_pos = artist_bpm_box_left_x + artist_bpm_box_padding_x;

        let artist_visual_height = detail_font_typographic_height_normalized * detail_text_effective_scale;
        let artist_visual_top_y = artist_bpm_box_top_y + (artist_bpm_box_current_height / 2.0 - artist_visual_height) / 2.0 ;
        let artist_text_baseline_y = artist_visual_top_y + (list_font.metrics.ascender * detail_text_effective_scale) + text_vertical_nudge_current;

        renderer.draw_text(
            device, cmd_buf, list_font, &artist_text_full,
            artist_text_x_pos, artist_text_baseline_y, config::MENU_NORMAL_COLOR,
            detail_text_effective_scale, None
        );

        let bpm_text_full = if selected_song_arc.bpms_header.len() == 1 {
            format!("BPM: {:.0}", selected_song_arc.bpms_header[0].1)
        } else if !selected_song_arc.bpms_header.is_empty() {
            let min_bpm = selected_song_arc.bpms_header.iter().map(|&(_, bpm)| bpm).fold(f32::INFINITY, f32::min);
            let max_bpm = selected_song_arc.bpms_header.iter().map(|&(_, bpm)| bpm).fold(f32::NEG_INFINITY, f32::max);
            if (min_bpm - max_bpm).abs() < 0.1 {
                format!("BPM: {:.0}", min_bpm)
            } else {
                format!("BPM: {:.0} - {:.0}", min_bpm, max_bpm)
            }
        } else {
            "BPM: ???".to_string()
        };
        let bpm_text_x_pos = artist_bpm_box_left_x + artist_bpm_box_padding_x;
        let bpm_visual_top_y = artist_bpm_box_top_y + artist_bpm_box_current_height / 2.0 + (artist_bpm_box_current_height / 2.0 - artist_visual_height) / 2.0;
        let bpm_text_baseline_y = bpm_visual_top_y + (list_font.metrics.ascender * detail_text_effective_scale) + text_vertical_nudge_current;

        renderer.draw_text(
            device, cmd_buf, list_font, &bpm_text_full,
            bpm_text_x_pos, bpm_text_baseline_y, config::MENU_NORMAL_COLOR,
            detail_text_effective_scale, None
        );
    }
}