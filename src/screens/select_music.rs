// ### FILE: /mnt/c/Users/PerfectTaste/Documents/Code/deadsync/src/screens/select_music.rs ###
use crate::assets::{AssetManager, FontId, SoundId, TextureId};
use crate::audio::AudioManager;
use crate::config;
use crate::graphics::renderer::{DescriptorSetId, Renderer};
use crate::state::{AppState, SelectMusicState, VirtualKeyCode};
use ash::vk;
use cgmath::{Rad, Vector3};
use log::{debug, trace};
use winit::event::{ElementState, KeyEvent};

// --- Input Handling ---
// ... (remains the same) ...
pub fn handle_input(
    key_event: &KeyEvent,
    state: &mut SelectMusicState,
    audio_manager: &AudioManager,
) -> Option<AppState> {
    if key_event.state == ElementState::Pressed && !key_event.repeat {
        if let Some(virtual_keycode) =
            crate::state::key_to_virtual_keycode(key_event.logical_key.clone())
        {
            match virtual_keycode {
                VirtualKeyCode::Up => {
                    let old_index = state.selected_index;
                    state.selected_index = if state.selected_index == 0 {
                        state.songs.len().saturating_sub(1)
                    } else {
                        state.selected_index - 1
                    };
                    if state.selected_index != old_index && !state.songs.is_empty() {
                        audio_manager.play_sfx(SoundId::MenuChange);
                    }
                    debug!("SelectMusic Up: Selected index {}", state.selected_index);
                }
                VirtualKeyCode::Down => {
                    if !state.songs.is_empty() {
                        let old_index = state.selected_index;
                        state.selected_index = (state.selected_index + 1) % state.songs.len();
                        if state.selected_index != old_index {
                            audio_manager.play_sfx(SoundId::MenuChange);
                        }
                        debug!("SelectMusic Down: Selected index {}", state.selected_index);
                    }
                }
                VirtualKeyCode::Enter => {
                    if !state.songs.is_empty() {
                        debug!(
                            "SelectMusic Enter: Selected song '{}' at index {}",
                            state.songs[state.selected_index], state.selected_index
                        );
                        audio_manager.play_sfx(SoundId::MenuStart);
                        return Some(AppState::Gameplay);
                    }
                }
                VirtualKeyCode::Escape => {
                    debug!("SelectMusic Escape: Returning to Main Menu");
                    return Some(AppState::Menu);
                }
                _ => {}
            }
        }
    }
    None
}

// --- Update Logic ---
pub fn update(_state: &mut SelectMusicState, _dt: f32) {}


// --- Drawing Logic ---
pub fn draw(
    renderer: &Renderer,
    state: &SelectMusicState,
    assets: &AssetManager,
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
) {
    let header_footer_font = assets
        .get_font(FontId::Wendy)
        .expect("Wendy (Main) font for headers not loaded");
    let list_font = assets // Font for the song titles on the wheel
        .get_font(FontId::Miso)
        .expect("Miso font for list not loaded");

    let (window_width, window_height) = renderer.window_size();
    trace!("Current window_width: {}, window_height: {}", window_width, window_height);
    let center_x = window_width / 2.0;

    // --- Bar and Text Styling Constants ---
    const TARGET_BAR_TEXT_VISUAL_PX_HEIGHT_AT_REF_RES: f32 = 36.0; 
    const OBSERVED_PX_HEIGHT_AT_REF_FOR_30PX_TARGET_OLD_METHOD: f32 = 19.0;
    const ASCENDER_POSITIONING_ADJUSTMENT_FACTOR: f32 = 0.65;
    const HEADER_FOOTER_LETTER_SPACING_FACTOR: f32 = 0.90;

    // --- Layout Box Constants (Dimensions at reference resolution) ---
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
    const CENTER_MUSIC_WHEEL_SLOT_INDEX: usize = 7; // 0-indexed, so 7 is the 8th slot

    // Gaps
    const VERTICAL_GAP_PINK_TO_UPPER_REF: f32 = 7.0;
    const HORIZONTAL_GAP_LEFT_TO_RIGHT_REF: f32 = 3.0;
    const VERTICAL_GAP_BETWEEN_LEFT_BOXES_REF: f32 = 36.0;
    const VERTICAL_GAP_TOPLEFT_TO_TOPMOST_REF: f32 = 1.0; 
    const VERTICAL_GAP_STEPARTIST_TO_ARTIST_REF: f32 = 5.0; 
    const VERTICAL_GAP_ARTIST_TO_BANNER_REF: f32 = 2.0; 
    const MUSIC_WHEEL_VERTICAL_GAP_REF: f32 = 2.0; 

    // Colors
    const UI_BOX_DARK_COLOR: [f32;4] = [30.0/255.0, 40.0/255.0, 47.0/255.0, 1.0]; 
    const MUSIC_WHEEL_BOX_COLOR: [f32;4] = [10.0/255.0, 20.0/255.0, 27.0/255.0, 1.0]; 
    const PINK_BOX_COLOR: [f32; 4] = [1.0, 71.0 / 255.0, 179.0 / 255.0, 1.0]; 
    const TOP_LEFT_BOX_COLOR: [f32; 4] = [230.0 / 255.0, 230.0 / 255.0, 250.0 / 255.0, 1.0];      
    
    const LAYOUT_BOXES_REF_RES_WIDTH: f32 = 1280.0; 
    const LAYOUT_BOXES_REF_RES_HEIGHT: f32 = 720.0;  

    // --- Calculate Scaled Dimensions and Gaps ---
    let width_scale_factor = window_width / LAYOUT_BOXES_REF_RES_WIDTH;
    let height_scale_factor = window_height / LAYOUT_BOXES_REF_RES_HEIGHT;

    // Box dimensions
    let pink_box_current_width = PINK_BOX_REF_WIDTH * width_scale_factor;
    // ... (all other box dimension calculations remain the same)
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
    let music_wheel_box_current_width = MUSIC_WHEEL_BOX_REF_WIDTH * width_scale_factor; 
    let music_wheel_box_current_height = MUSIC_WHEEL_BOX_REF_HEIGHT * height_scale_factor; 
    
    // Gaps
    let vertical_gap_pink_to_upper_current = VERTICAL_GAP_PINK_TO_UPPER_REF * height_scale_factor;
    // ... (all other gap calculations remain the same) ...
    let horizontal_gap_left_to_right_current = HORIZONTAL_GAP_LEFT_TO_RIGHT_REF * width_scale_factor;
    let vertical_gap_between_left_boxes_current = VERTICAL_GAP_BETWEEN_LEFT_BOXES_REF * height_scale_factor;
    let vertical_gap_topleft_to_topmost_current = VERTICAL_GAP_TOPLEFT_TO_TOPMOST_REF * height_scale_factor; 
    let vertical_gap_stepartist_to_artist_current = VERTICAL_GAP_STEPARTIST_TO_ARTIST_REF * height_scale_factor; 
    let vertical_gap_artist_to_banner_current = VERTICAL_GAP_ARTIST_TO_BANNER_REF * height_scale_factor; 
    let music_wheel_vertical_gap_current = MUSIC_WHEEL_VERTICAL_GAP_REF * height_scale_factor; 


    // --- Music Wheel Box and Text Drawing ---
    let total_music_boxes_height = NUM_MUSIC_WHEEL_BOXES as f32 * music_wheel_box_current_height;
    let total_music_gaps_height = (NUM_MUSIC_WHEEL_BOXES.saturating_sub(1)) as f32 * music_wheel_vertical_gap_current;
    let full_music_wheel_stack_height = total_music_boxes_height + total_music_gaps_height;
    let music_wheel_stack_top_y = (window_height - full_music_wheel_stack_height) / 2.0;

    let music_box_right_x = window_width; 
    let music_box_left_x = music_box_right_x - music_wheel_box_current_width; // For text centering
    let music_box_center_x = music_box_left_x + music_wheel_box_current_width / 2.0;

    // Text scaling for song titles on the wheel (can be different from list_font scale used for main list if needed)
    // For now, let's make it slightly smaller than the box height.
    let wheel_text_target_height = music_wheel_box_current_height * 0.7; // e.g., 70% of box height
    let wheel_text_effective_scale = wheel_text_target_height / list_font.metrics.em_size.max(1e-5);
    let wheel_text_scaled_ascender = list_font.metrics.ascender * wheel_text_effective_scale;
    // Simplified vertical centering for single line text in box:
    // baseline = box_center_y - (text_visual_center_from_baseline)
    // Assuming for Miso, ascender is a good proxy for distance from baseline to top, and descender to bottom.
    // Visual center from baseline = (scaled_ascender + scaled_descender) / 2.0
    let wheel_text_visual_center_offset = (list_font.metrics.ascender + list_font.metrics.descender) / 2.0 * wheel_text_effective_scale;


    for i in 0..NUM_MUSIC_WHEEL_BOXES {
        let current_box_top_y = music_wheel_stack_top_y + (i as f32 * (music_wheel_box_current_height + music_wheel_vertical_gap_current));
        let current_box_center_y = current_box_top_y + music_wheel_box_current_height / 2.0;
        
        // Draw background box
        renderer.draw_quad(
            device, cmd_buf, DescriptorSetId::SolidColor,
            Vector3::new(music_box_center_x, current_box_center_y, 0.0),
            (music_wheel_box_current_width, music_wheel_box_current_height), Rad(0.0), MUSIC_WHEEL_BOX_COLOR,
            [0.0,0.0], [1.0,1.0]
        );

        // Determine which song index maps to this visual slot `i`
        // This logic makes the `state.selected_index` appear in the `CENTER_MUSIC_WHEEL_SLOT_INDEX`
        let num_songs = state.songs.len();
        if num_songs > 0 {
            let song_list_index_offset = state.selected_index as i32 - CENTER_MUSIC_WHEEL_SLOT_INDEX as i32;
            let current_song_effective_index = song_list_index_offset + i as i32;

            if current_song_effective_index >= 0 && current_song_effective_index < num_songs as i32 {
                let song_name = &state.songs[current_song_effective_index as usize];
                
                let text_width_pixels = list_font.measure_text_normalized(song_name) * wheel_text_effective_scale;
                let text_x_pos = music_box_left_x + (music_wheel_box_current_width - text_width_pixels) / 2.0; // Center horizontally in box
                
                // Vertical centering of text within this specific music wheel box
                // baseline_y = box_center_y - visual_center_from_baseline
                let text_baseline_y = current_box_center_y - wheel_text_visual_center_offset;

                let text_color = if i == CENTER_MUSIC_WHEEL_SLOT_INDEX {
                    config::MENU_SELECTED_COLOR // Highlight the centered/selected song
                } else {
                    config::MENU_NORMAL_COLOR
                };

                renderer.draw_text(
                    device, cmd_buf, list_font, song_name,
                    text_x_pos, text_baseline_y, text_color,
                    wheel_text_effective_scale, None // No custom letter spacing for wheel text yet
                );
            }
        }
    }
    trace!("Music Wheel: stack_top_y={:.1}, stack_height={:.1}", music_wheel_stack_top_y, full_music_wheel_stack_height);


    // --- Header/Footer Bar Drawing ---
    let bar_height = (window_height / config::UI_REFERENCE_HEIGHT) * config::UI_BAR_REFERENCE_HEIGHT;
    // ... (Header/Footer Bar drawing remains the same) ...
    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::SolidColor,
        Vector3::new(center_x, bar_height / 2.0, 0.0),
        (window_width, bar_height), Rad(0.0), config::UI_BAR_COLOR,
        [0.0,0.0], [1.0,1.0]
    );
    let footer_y_top_edge = window_height - bar_height;
    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::SolidColor,
        Vector3::new(center_x, footer_y_top_edge + bar_height / 2.0, 0.0),
        (window_width, bar_height), Rad(0.0), config::UI_BAR_COLOR,
        [0.0,0.0], [1.0,1.0]
    );


    // --- Other Layout Boxes (Pink Box, Small Upper Right, etc.) ---
    // ... (Drawing logic for these boxes remains the same, using updated colors where specified) ...
    let pink_box_left_x = 0.0;
    let pink_box_right_x = pink_box_left_x + pink_box_current_width;
    let pink_box_top_y = footer_y_top_edge - pink_box_current_height;
    let pink_box_center_x = pink_box_left_x + pink_box_current_width / 2.0;
    let pink_box_center_y = pink_box_top_y + pink_box_current_height / 2.0;
    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::SolidColor, 
        Vector3::new(pink_box_center_x, pink_box_center_y, 0.0),
        (pink_box_current_width, pink_box_current_height), Rad(0.0), PINK_BOX_COLOR,
        [0.0,0.0], [1.0,1.0]
    );

    let small_upper_right_box_bottom_y = pink_box_top_y - vertical_gap_pink_to_upper_current;
    let small_upper_right_box_top_y = small_upper_right_box_bottom_y - small_upper_right_box_current_height;
    let small_upper_right_box_right_x = pink_box_right_x; 
    let small_upper_right_box_left_x = small_upper_right_box_right_x - small_upper_right_box_current_width;
    let small_upper_right_box_center_x = small_upper_right_box_left_x + small_upper_right_box_current_width / 2.0;
    let small_upper_right_box_center_y = small_upper_right_box_top_y + small_upper_right_box_current_height / 2.0;
    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::SolidColor,
        Vector3::new(small_upper_right_box_center_x, small_upper_right_box_center_y, 0.0),
        (small_upper_right_box_current_width, small_upper_right_box_current_height), Rad(0.0), UI_BOX_DARK_COLOR, 
        [0.0,0.0], [1.0,1.0]
    );
    
    let bottom_left_box_right_x = small_upper_right_box_left_x - horizontal_gap_left_to_right_current;
    let bottom_left_box_left_x = bottom_left_box_right_x - left_boxes_current_width;
    let bottom_left_box_bottom_y = small_upper_right_box_bottom_y; 
    let bottom_left_box_top_y = bottom_left_box_bottom_y - left_box_current_height;
    let bottom_left_box_center_x = bottom_left_box_left_x + left_boxes_current_width / 2.0;
    let bottom_left_box_center_y = bottom_left_box_top_y + left_box_current_height / 2.0;
    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::SolidColor,
        Vector3::new(bottom_left_box_center_x, bottom_left_box_center_y, 0.0),
        (left_boxes_current_width, left_box_current_height), Rad(0.0), UI_BOX_DARK_COLOR, 
        [0.0,0.0], [1.0,1.0]
    );

    let top_left_box_left_x = bottom_left_box_left_x; 
    let top_left_box_bottom_y = bottom_left_box_top_y - vertical_gap_between_left_boxes_current;
    let top_left_box_top_y = top_left_box_bottom_y - left_box_current_height; 
    let top_left_box_center_x = top_left_box_left_x + left_boxes_current_width / 2.0;
    let top_left_box_center_y = top_left_box_top_y + left_box_current_height / 2.0;
    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::SolidColor,
        Vector3::new(top_left_box_center_x, top_left_box_center_y, 0.0),
        (left_boxes_current_width, left_box_current_height), Rad(0.0), TOP_LEFT_BOX_COLOR, 
        [0.0,0.0], [1.0,1.0]
    );

    let topmost_left_box_left_x = top_left_box_left_x; 
    let topmost_left_box_bottom_y = top_left_box_top_y - vertical_gap_topleft_to_topmost_current; 
    let topmost_left_box_top_y = topmost_left_box_bottom_y - topmost_left_box_current_height;
    let topmost_left_box_center_x = topmost_left_box_left_x + topmost_left_box_current_width / 2.0;
    let topmost_left_box_center_y = topmost_left_box_top_y + topmost_left_box_current_height / 2.0;
    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::SolidColor,
        Vector3::new(topmost_left_box_center_x, topmost_left_box_center_y, 0.0),
        (topmost_left_box_current_width, topmost_left_box_current_height), Rad(0.0), PINK_BOX_COLOR, 
        [0.0,0.0], [1.0,1.0]
    );

    let artist_bpm_box_left_x = topmost_left_box_left_x; 
    let artist_bpm_box_bottom_y = topmost_left_box_top_y - vertical_gap_stepartist_to_artist_current;
    let artist_bpm_box_top_y = artist_bpm_box_bottom_y - artist_bpm_box_current_height;
    let artist_bpm_box_center_x = artist_bpm_box_left_x + artist_bpm_box_current_width / 2.0;
    let artist_bpm_box_center_y = artist_bpm_box_top_y + artist_bpm_box_current_height / 2.0;
    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::SolidColor,
        Vector3::new(artist_bpm_box_center_x, artist_bpm_box_center_y, 0.0),
        (artist_bpm_box_current_width, artist_bpm_box_current_height), Rad(0.0), UI_BOX_DARK_COLOR, 
        [0.0,0.0], [1.0,1.0]
    );

    let fallback_banner_left_x = artist_bpm_box_left_x; 
    let fallback_banner_width_to_draw = fallback_banner_current_width; 
    let fallback_banner_bottom_y = artist_bpm_box_top_y - vertical_gap_artist_to_banner_current;
    let fallback_banner_top_y = fallback_banner_bottom_y - fallback_banner_current_height;
    let fallback_banner_center_x = fallback_banner_left_x + fallback_banner_width_to_draw / 2.0;
    let fallback_banner_center_y = fallback_banner_top_y + fallback_banner_current_height / 2.0;
    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::FallbackBanner, 
        Vector3::new(fallback_banner_center_x, fallback_banner_center_y, 0.0),
        (fallback_banner_width_to_draw, fallback_banner_current_height), Rad(0.0), 
        [1.0, 1.0, 1.0, 1.0], 
        [0.0,0.0], [1.0,1.0] 
    );


    // --- Header and Footer Text Drawing ---
    // ... (Header/Footer Text drawing logic remains the same) ...
    let target_bar_text_visual_current_px_height = TARGET_BAR_TEXT_VISUAL_PX_HEIGHT_AT_REF_RES * (window_height / config::UI_REFERENCE_HEIGHT);
    let font_typographic_height_normalized = (header_footer_font.metrics.ascender - header_footer_font.metrics.descender).max(1e-5); 
    let base_scale_for_typographic_height = target_bar_text_visual_current_px_height / font_typographic_height_normalized;
    let height_adjustment_factor = if OBSERVED_PX_HEIGHT_AT_REF_FOR_30PX_TARGET_OLD_METHOD > 1e-5 { TARGET_BAR_TEXT_VISUAL_PX_HEIGHT_AT_REF_RES / OBSERVED_PX_HEIGHT_AT_REF_FOR_30PX_TARGET_OLD_METHOD } else { 1.0 };
    let hf_effective_scale = base_scale_for_typographic_height * height_adjustment_factor;
    let scaled_ascender_metric = header_footer_font.metrics.ascender * hf_effective_scale;
    let scaled_ascender_for_positioning = scaled_ascender_metric * ASCENDER_POSITIONING_ADJUSTMENT_FACTOR;
    let empty_vertical_space = (bar_height - target_bar_text_visual_current_px_height).max(0.0);
    let padding_from_bar_top_to_text_visual_top = empty_vertical_space / 2.0;
    let header_baseline_y = padding_from_bar_top_to_text_visual_top + scaled_ascender_for_positioning;
    let footer_baseline_y = footer_y_top_edge + padding_from_bar_top_to_text_visual_top + scaled_ascender_for_positioning;

    let header_text_left_padding_px = 14.0 * (window_width / config::WINDOW_WIDTH as f32);
    let header_text_str = "SELECT MUSIC";
    renderer.draw_text(
        device, cmd_buf, header_footer_font, header_text_str,
        header_text_left_padding_px, header_baseline_y, config::UI_BAR_TEXT_COLOR,
        hf_effective_scale, Some(HEADER_FOOTER_LETTER_SPACING_FACTOR)
    );
    let footer_text_str = "EVENT MODE";
    let footer_text_pixel_width = header_footer_font.measure_text_normalized(footer_text_str) * hf_effective_scale;
    renderer.draw_text(
        device, cmd_buf, header_footer_font, footer_text_str,
        center_x - (footer_text_pixel_width * HEADER_FOOTER_LETTER_SPACING_FACTOR) / 2.0, 
        footer_baseline_y, config::UI_BAR_TEXT_COLOR,
        hf_effective_scale, Some(HEADER_FOOTER_LETTER_SPACING_FACTOR)
    );

    // Note: The old central song list text drawing is removed as song titles are now on the wheel.
    // If you had other text in that central area, it would go here.
}