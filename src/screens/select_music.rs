// src/screens/select_music.rs
use crate::assets::{AssetManager, FontId, SoundId};
use crate::audio::AudioManager;
use crate::config; // Import config to use its constants
use crate::graphics::renderer::{DescriptorSetId, Renderer};
use crate::state::{AppState, SelectMusicState, VirtualKeyCode};
use cgmath::{Rad, Vector3};
use log::{debug, trace}; // Added trace
use ash::vk;
use winit::event::{ElementState, KeyEvent};

// --- Input Handling ---
// (No changes needed here - keep as is)
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
// (No changes needed here - keep as is)
pub fn update(_state: &mut SelectMusicState, _dt: f32) {}

// --- Drawing Logic ---
pub fn draw(
    renderer: &Renderer,
    state: &SelectMusicState,
    assets: &AssetManager,
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
) {
    let font = assets.get_font(FontId::Main).expect("Main font not loaded");
    let (window_width, window_height) = renderer.window_size();
    let center_x = window_width / 2.0;

    // --- Calculate Scaled Heights ---
    let bar_height = (window_height / config::UI_REFERENCE_HEIGHT) * config::UI_BAR_REFERENCE_HEIGHT;
    trace!("Bar height: {}", bar_height);

    // --- 1. Draw Header Background --- (Drawn FIRST)
    let header_center_y = bar_height / 2.0;
    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::SolidColor,
        Vector3::new(center_x, header_center_y, 0.0), (window_width, bar_height),
        Rad(0.0), config::UI_BAR_COLOR, [0.0, 0.0], [1.0, 1.0],
    );

    // --- 2. Draw Footer Background --- (Drawn SECOND)
    let footer_y_top = window_height - bar_height;
    let footer_center_y = footer_y_top + bar_height / 2.0;
    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::SolidColor,
        Vector3::new(center_x, footer_center_y, 0.0), (window_width, bar_height),
        Rad(0.0), config::UI_BAR_COLOR, [0.0, 0.0], [1.0, 1.0],
    );

    // --- Calculate Text Scale & Baselines ---
    let target_text_height_ratio = 0.6;
    let target_text_pixel_height = bar_height * target_text_height_ratio;
    let text_scale = target_text_pixel_height / font.line_height;
    trace!("Text scale: {}", text_scale);

    let scaled_font_ascent = (font.metrics.baseline - font.metrics.top) * text_scale;
    let padding_y = bar_height * 0.15; // Fine-tune this padding if needed

    let header_baseline_y = padding_y + scaled_font_ascent;
    let footer_baseline_y = footer_y_top + padding_y + scaled_font_ascent;
    trace!("Header baseline Y: {}, Footer baseline Y: {}", header_baseline_y, footer_baseline_y);
    trace!("Text color: {:?}", config::UI_BAR_TEXT_COLOR); // Log the color being used

    // --- 3. Draw Header Text --- (Drawn THIRD, should be ON TOP of header bg)
    let header_text = "Select Music";
    renderer.draw_text(
        device, cmd_buf, font, header_text,
        1.0, 1.0,
        config::UI_BAR_TEXT_COLOR, // Use the (now black) color from config
        0.4,
    );

    // --- 4. Draw Footer Text --- (Drawn FOURTH, should be ON TOP of footer bg)
    let footer_text = "EVENT MODE";
    let scaled_footer_text_width = font.measure_text(footer_text) * text_scale;
    renderer.draw_text(
        device, cmd_buf, font, footer_text,
        center_x - scaled_footer_text_width / 2.0, footer_baseline_y,
        config::UI_BAR_TEXT_COLOR, // Use the (now black) color from config
        text_scale,
    );

    // --- 5. Draw Song List --- (Drawn LAST, between bars)
    let list_area_top = bar_height;
    let list_area_bottom = window_height - bar_height;
    let list_area_height = list_area_bottom - list_area_top;

    let list_item_count = state.songs.len().max(1) as f32;
    let list_text_scale = 1.0;
    let list_line_height = font.line_height * list_text_scale;
    let list_spacing_y = list_line_height * 2.5;
    let total_list_height = (list_item_count - 1.0) * list_spacing_y + list_line_height;
    let start_y = list_area_top + (list_area_height - total_list_height) / 2.0 + list_line_height * 0.5;

    if state.songs.is_empty() {
        let empty_text = "No songs found!";
        let text_width = font.measure_text(empty_text) * list_text_scale;
        renderer.draw_text(
            device, cmd_buf, font, empty_text,
            center_x - text_width / 2.0, list_area_top + list_area_height / 2.0,
            config::MENU_NORMAL_COLOR, list_text_scale,
        );
    } else {
        for (index, song_name) in state.songs.iter().enumerate() {
            let y_pos = start_y + index as f32 * list_spacing_y;
            let color = if index == state.selected_index {
                config::MENU_SELECTED_COLOR
            } else {
                config::MENU_NORMAL_COLOR
            };
            let text_width = font.measure_text(song_name) * list_text_scale;
            let x_pos = center_x - text_width / 2.0;
            renderer.draw_text(
                device, cmd_buf, font, song_name,
                x_pos, y_pos, color, list_text_scale,
            );
        }
    }
}