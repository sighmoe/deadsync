// src/screens/select_music.rs
use crate::assets::{AssetManager, FontId, SoundId};
use crate::audio::AudioManager;
use crate::config;
use crate::graphics::renderer::Renderer;
use crate::state::{AppState, SelectMusicState, VirtualKeyCode};
use log::{debug};
use ash::vk;
use winit::event::{ElementState, KeyEvent};

// --- Input Handling ---
pub fn handle_input(
    key_event: &KeyEvent,
    state: &mut SelectMusicState,
    audio_manager: &AudioManager,
) -> Option<AppState> {
    if key_event.state == ElementState::Pressed && !key_event.repeat {
         if let Some(virtual_keycode) = crate::state::key_to_virtual_keycode(key_event.logical_key.clone()) {
            match virtual_keycode {
                VirtualKeyCode::Up => {
                    let old_index = state.selected_index;
                    state.selected_index = if state.selected_index == 0 {
                        state.songs.len().saturating_sub(1) // Avoid underflow on empty list
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
                        // TODO: Pass selected song info to gameplay state initialization
                        // For now, just transition, assuming the hardcoded song is the only one
                        return Some(AppState::Gameplay);
                    }
                }
                VirtualKeyCode::Escape => {
                    debug!("SelectMusic Escape: Returning to Main Menu");
                    return Some(AppState::Menu); // Go back to main menu
                }
                _ => {}
            }
        }
    }
    None
}

// --- Update Logic ---
pub fn update(_state: &mut SelectMusicState, _dt: f32) {
    // No update needed for a static list for now
}

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

    // --- Draw Title ---
    let title = "Select Music";
    let title_width = font.measure_text(title);
    renderer.draw_text(
        device, cmd_buf, font, title,
        center_x - title_width / 2.0, 100.0, // Position near top-center
        config::MENU_NORMAL_COLOR,
    );

    // --- Draw Song List ---
    let start_y = window_height / 2.0 - 100.0; // Adjust starting position
    let spacing_y = font.line_height * 2.5; // Adjust spacing

    if state.songs.is_empty() {
        let empty_text = "No songs found!";
        let text_width = font.measure_text(empty_text);
         renderer.draw_text(
             device, cmd_buf, font, empty_text,
             center_x - text_width / 2.0, start_y,
             config::MENU_NORMAL_COLOR,
         );
    } else {
        for (index, song_name) in state.songs.iter().enumerate() {
            let y_pos = start_y + index as f32 * spacing_y;
            let color = if index == state.selected_index {
                config::MENU_SELECTED_COLOR
            } else {
                config::MENU_NORMAL_COLOR
            };

            let text_width = font.measure_text(song_name);
            let x_pos = center_x - text_width / 2.0;

            renderer.draw_text(
                device, cmd_buf, font, song_name,
                x_pos, y_pos,
                color,
            );
        }
    }

    // --- Draw Help Text ---
    let help_text = "Select Music";
    let help_width = font.measure_text(help_text);
     renderer.draw_text(
         device, cmd_buf, font, help_text,
         center_x - help_width / 2.0, window_height - 50.0, // Position near bottom-center
         config::MENU_NORMAL_COLOR,
     );
}