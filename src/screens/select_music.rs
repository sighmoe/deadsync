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
    // Example: CJK font could be used for song titles if they contain CJK characters.
    // For now, Miso is used for all song titles. If you need CJK, you'd select FontId::Cjk.
    // let song_title_font = assets.get_font(FontId::Cjk).expect("CJK font for song titles not loaded");

    let (window_width, window_height) = renderer.window_size();
    let center_x = window_width / 2.0;

    let bar_height =
        (window_height / config::UI_REFERENCE_HEIGHT) * config::UI_BAR_REFERENCE_HEIGHT;
    trace!("SelectMusic Bar height: {}", bar_height);

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
    let footer_y_top = window_height - bar_height;
    renderer.draw_quad(
        device,
        cmd_buf,
        DescriptorSetId::SolidColor,
        Vector3::new(center_x, footer_y_top + bar_height / 2.0, 0.0),
        (window_width, bar_height),
        Rad(0.0),
        config::UI_BAR_COLOR,
        [0.0, 0.0],
        [1.0, 1.0],
    );

    // --- Header and Footer Text ---
    let target_text_height_ratio_hf = 0.80;
    let target_hf_text_pixel_height = bar_height * target_text_height_ratio_hf;
    let hf_effective_scale =
        target_hf_text_pixel_height / header_footer_font.metrics.em_size.max(1.0);
    trace!(
        "SelectMusic Header/Footer text effective_scale (Wendy): {}",
        hf_effective_scale
    );

    let scaled_hf_font_ascender = header_footer_font.metrics.ascender * hf_effective_scale;
    let scaled_hf_font_line_height = header_footer_font.metrics.line_height * hf_effective_scale;
    let padding_y_from_bar_edge_hf = (bar_height - scaled_hf_font_line_height) / 2.0;

    let header_baseline_y = padding_y_from_bar_edge_hf + scaled_hf_font_ascender;
    let footer_baseline_y = footer_y_top + padding_y_from_bar_edge_hf + scaled_hf_font_ascender;

    let header_text = "SELECT MUSIC";
    renderer.draw_text(
        device,
        cmd_buf,
        header_footer_font,
        header_text,
        20.0 * (window_width / config::WINDOW_WIDTH as f32), // Scale padding
        header_baseline_y,
        config::UI_BAR_TEXT_COLOR,
        hf_effective_scale,
    );

    let footer_text = "ENTER: Start Song | ESC: Back"; // Updated footer
    let footer_text_pixel_width =
        header_footer_font.measure_text_normalized(footer_text) * hf_effective_scale;
    renderer.draw_text(
        device,
        cmd_buf,
        header_footer_font,
        footer_text,
        center_x - footer_text_pixel_width / 2.0,
        footer_baseline_y,
        config::UI_BAR_TEXT_COLOR,
        hf_effective_scale,
    );

    // --- Song List ---
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

    let list_item_baseline_spacing = list_font_line_height_scaled * 1.3; // Spacing between baselines

    let total_list_height_for_layout = (list_item_count - 1.0).max(0.0)
        * list_item_baseline_spacing
        + list_font_line_height_scaled;

    let first_item_baseline_y = list_area_top
        + (list_area_height - total_list_height_for_layout) / 2.0
        + list_font_ascender_scaled;

    if state.songs.is_empty() {
        let empty_text = "No songs found in songs directory!";
        let empty_text_pixel_width =
            list_font.measure_text_normalized(empty_text) * list_effective_scale;
        // Center the single line of text
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
        );
    } else {
        for (index, song_name) in state.songs.iter().enumerate() {
            let baseline_y = first_item_baseline_y + index as f32 * list_item_baseline_spacing;

            // Basic culling for items way off screen
            if baseline_y < list_area_top - list_font_line_height_scaled
                || baseline_y > list_area_bottom + list_font_ascender_scaled
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

            // Here you could choose a different font if the song_name contains CJK
            // e.g., if is_cjk(song_name) { use song_title_font } else { use list_font }
            // For now, we use `list_font` (Miso) for all.
            renderer.draw_text(
                device,
                cmd_buf,
                list_font,
                song_name,
                x_pos,
                baseline_y,
                color,
                list_effective_scale,
            );
        }
    }
}
