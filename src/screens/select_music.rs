// ### FILE: /mnt/c/Users/PerfectTaste/Documents/Code/deadsync/src/screens/select_music.rs ###
use crate::assets::{AssetManager, FontId, SoundId};
use crate::audio::AudioManager;
use crate::config;
use crate::graphics::renderer::{DescriptorSetId, Renderer};
use crate::state::{AppState, SelectMusicState, VirtualKeyCode};
use ash::vk;
use cgmath::{Rad, Vector3};
use log::{debug, trace};
use winit::event::{ElementState, KeyEvent};

// --- Input Handling ---
// ... (no changes to input handling) ...
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
    let list_font = assets
        .get_font(FontId::Miso)
        .expect("Miso font for list not loaded");

    let (window_width, window_height) = renderer.window_size();
    let center_x = window_width / 2.0;

    // --- Bar and Text Styling Constants ---
    const TARGET_BAR_TEXT_VISUAL_PX_HEIGHT_AT_REF_RES: f32 = 32.0; 
    const OBSERVED_PX_HEIGHT_AT_REF_FOR_30PX_TARGET_OLD_METHOD: f32 = 19.0;
    const ASCENDER_POSITIONING_ADJUSTMENT_FACTOR: f32 = 0.7;
    const HEADER_FOOTER_LETTER_SPACING_FACTOR: f32 = 0.95; // NEW: e.g., 10% tighter

    // ... (bar drawing and other calculations remain the same) ...
    // --- Bar Calculations ---
    let bar_height =
        (window_height / config::UI_REFERENCE_HEIGHT) * config::UI_BAR_REFERENCE_HEIGHT;
    trace!("SelectMusic Bar height: {:.2}", bar_height);

    // Draw Header Bar
    renderer.draw_quad(
        device,
        cmd_buf,
        DescriptorSetId::SolidColor,
        Vector3::new(center_x, bar_height / 2.0, 0.0),
        (window_width, bar_height),
        Rad(0.0),
        config::UI_BAR_COLOR,
        [0.0, 0.0],
        [1.0, 1.0],
    );

    // Draw Footer Bar
    let footer_y_top_edge = window_height - bar_height;
    renderer.draw_quad(
        device,
        cmd_buf,
        DescriptorSetId::SolidColor,
        Vector3::new(center_x, footer_y_top_edge + bar_height / 2.0, 0.0),
        (window_width, bar_height),
        Rad(0.0),
        config::UI_BAR_COLOR,
        [0.0, 0.0],
        [1.0, 1.0],
    );

    // --- Header and Footer Text Calculations ---
    let target_bar_text_visual_current_px_height =
        TARGET_BAR_TEXT_VISUAL_PX_HEIGHT_AT_REF_RES * (window_height / config::UI_REFERENCE_HEIGHT);

    let font_typographic_height_normalized = (header_footer_font.metrics.ascender
        - header_footer_font.metrics.descender)
        .max(1e-5); 

    let base_scale_for_typographic_height =
        target_bar_text_visual_current_px_height / font_typographic_height_normalized;

    let height_adjustment_factor = if OBSERVED_PX_HEIGHT_AT_REF_FOR_30PX_TARGET_OLD_METHOD > 1e-5 {
        TARGET_BAR_TEXT_VISUAL_PX_HEIGHT_AT_REF_RES / OBSERVED_PX_HEIGHT_AT_REF_FOR_30PX_TARGET_OLD_METHOD
    } else {
        1.0 
    };
    
    let hf_effective_scale = base_scale_for_typographic_height * height_adjustment_factor;

    trace!(
        "SelectMusic H/F Text: target_visual_px_h={:.2}, final_scale={:.2}",
        target_bar_text_visual_current_px_height,
        hf_effective_scale
    );
    
    // --- Vertical Centering ---
    let scaled_ascender_metric = header_footer_font.metrics.ascender * hf_effective_scale;
    // Apply empirical adjustment for positioning capital letters
    let scaled_ascender_for_positioning = scaled_ascender_metric * ASCENDER_POSITIONING_ADJUSTMENT_FACTOR;
    
    let empty_vertical_space = (bar_height - target_bar_text_visual_current_px_height).max(0.0);
    let padding_from_bar_top_to_text_visual_top = empty_vertical_space / 2.0;
    
    let header_baseline_y = padding_from_bar_top_to_text_visual_top + scaled_ascender_for_positioning;
    let footer_baseline_y = footer_y_top_edge + padding_from_bar_top_to_text_visual_top + scaled_ascender_for_positioning;

    trace!(
        "SelectMusic H/F Text Vertical: bar_h={:.1}, text_visual_h={:.1}, empty_space={:.1}, pad_visual_top={:.1}, scaled_asc_metric={:.1}, scaled_asc_pos={:.1}",
        bar_height, target_bar_text_visual_current_px_height, empty_vertical_space, padding_from_bar_top_to_text_visual_top, scaled_ascender_metric, scaled_ascender_for_positioning
    );
    trace!(
        "Header baseline_y={:.1}, Footer baseline_y={:.1}",
        header_baseline_y, footer_baseline_y
    );


    // --- Draw Header Text ---
    let header_text_left_padding_px = 14.0 * (window_width / config::WINDOW_WIDTH as f32);
    let header_text_str = "SELECT MUSIC";
    renderer.draw_text(
        device,
        cmd_buf,
        header_footer_font,
        header_text_str,
        header_text_left_padding_px,
        header_baseline_y,
        config::UI_BAR_TEXT_COLOR,
        hf_effective_scale,
        Some(HEADER_FOOTER_LETTER_SPACING_FACTOR), // Pass the factor
    );

    // --- Draw Footer Text ---
    let footer_text_str = "EVENT MODE";
    // Recalculate width with spacing factor if you want perfect centering of the *newly spaced* text.
    // However, for a small change, centering based on original width might be acceptable.
    // For true centering of adjusted text, you'd need a `measure_text_with_spacing` method.
    // Let's assume for now centering based on original width is fine.
    let footer_text_pixel_width =
        header_footer_font.measure_text_normalized(footer_text_str) * hf_effective_scale; 
        // To be more accurate, this width should also account for the new spacing factor.
        // A simple approximation: footer_text_pixel_width * HEADER_FOOTER_LETTER_SPACING_FACTOR
        // but it's not perfectly linear as the last char doesn't have "advance" after it.

    renderer.draw_text(
        device,
        cmd_buf,
        header_footer_font,
        footer_text_str,
        center_x - (footer_text_pixel_width * HEADER_FOOTER_LETTER_SPACING_FACTOR) / 2.0, // Adjusted centering slightly
        footer_baseline_y,
        config::UI_BAR_TEXT_COLOR,
        hf_effective_scale,
        Some(HEADER_FOOTER_LETTER_SPACING_FACTOR), // Pass the factor
    );

    // --- Song List ---
    // (Song list drawing remains the same, unless you want to apply letter spacing there too)
    // ...
    let list_area_top = bar_height;
    let list_area_bottom = window_height - bar_height; 
    let list_area_height = list_area_bottom - list_area_top;
    let target_list_item_pixel_height: f32 = 30.0 * (window_height / config::UI_REFERENCE_HEIGHT);
    let list_effective_scale = target_list_item_pixel_height / list_font.metrics.em_size.max(1.0);
    trace!(
        "SelectMusic Song list text effective_scale (Miso): {}",
        list_effective_scale
    );
    let list_item_count = state.songs.len().max(1) as f32; 
    let list_font_line_height_scaled = list_font.metrics.line_height * list_effective_scale;
    let list_font_ascender_scaled = list_font.metrics.ascender * list_effective_scale;
    let list_item_baseline_spacing = list_font_line_height_scaled * 1.3;
    let total_list_content_height = (list_item_count - 1.0).max(0.0)
        * list_item_baseline_spacing 
        + list_font_line_height_scaled; 
    let first_item_baseline_y = list_area_top 
        + (list_area_height - total_list_content_height) / 2.0 
        + list_font_ascender_scaled; 

    if state.songs.is_empty() {
        let empty_text = "No songs found in songs directory!";
        let empty_text_pixel_width =
            list_font.measure_text_normalized(empty_text) * list_effective_scale;
        let empty_text_baseline_y = list_area_top + (list_area_height / 2.0) 
            - (list_font_line_height_scaled / 2.0) 
            + list_font_ascender_scaled; 

        renderer.draw_text(
            device,
            cmd_buf,
            list_font,
            empty_text,
            center_x - empty_text_pixel_width / 2.0,
            empty_text_baseline_y,
            config::MENU_NORMAL_COLOR,
            list_effective_scale,
            None, // No custom spacing for this
        );
    } else {
        for (index, song_name) in state.songs.iter().enumerate() {
            let baseline_y = first_item_baseline_y + index as f32 * list_item_baseline_spacing;

            if baseline_y < list_area_top - list_font_line_height_scaled 
                || baseline_y - list_font_ascender_scaled > list_area_bottom 
            {
                continue;
            }

            let color = if index == state.selected_index {
                config::MENU_SELECTED_COLOR
            } else {
                config::MENU_NORMAL_COLOR
            };
            let text_pixel_width =
                list_font.measure_text_normalized(song_name) * list_effective_scale;
            let x_pos = center_x - text_pixel_width / 2.0;

            renderer.draw_text(
                device,
                cmd_buf,
                list_font, 
                song_name,
                x_pos,
                baseline_y,
                color,
                list_effective_scale,
                None, // No custom spacing for song list items
            );
        }
    }
}