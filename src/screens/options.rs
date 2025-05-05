// src/screens/options.rs
use crate::assets::{AssetManager, FontId};
// use crate::audio::AudioManager; // Not needed yet
use crate::config;
use crate::graphics::renderer::Renderer;
use crate::state::{AppState, OptionsState, VirtualKeyCode};
use log::{debug};
use ash::vk;
use winit::event::{ElementState, KeyEvent};

// --- Input Handling ---
pub fn handle_input(
    key_event: &KeyEvent,
    _state: &mut OptionsState,
    // _audio_manager: &AudioManager, // Not needed yet
) -> Option<AppState> {
    if key_event.state == ElementState::Pressed && !key_event.repeat {
        if let Some(VirtualKeyCode::Escape) = crate::state::key_to_virtual_keycode(key_event.logical_key.clone()) {
            debug!("Options Escape: Returning to Main Menu");
            return Some(AppState::Menu); // Go back to main menu
        }
        // Handle other keys for options later (e.g., left/right to change volume)
    }
    None
}

// --- Update Logic ---
pub fn update(_state: &mut OptionsState, _dt: f32) {
    // No update needed for now
}

// --- Drawing Logic ---
pub fn draw(
    renderer: &Renderer,
    _state: &OptionsState,
    assets: &AssetManager,
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
) {
    let font = assets.get_font(FontId::Main).expect("Main font not loaded");
    let (window_width, window_height) = renderer.window_size();
    let center_x = window_width / 2.0;

    // --- Draw Title ---
    let title = "Options";
    let title_width = font.measure_text(title);
    renderer.draw_text(
        device, cmd_buf, font, title,
        center_x - title_width / 2.0, 100.0,
        config::MENU_NORMAL_COLOR,
        1.0, // ADDED: Scale factor
    );

    // --- Draw Placeholder Text ---
     let placeholder_text = "Options will go here!";
     let placeholder_width = font.measure_text(placeholder_text);
     renderer.draw_text(
         device, cmd_buf, font, placeholder_text,
         center_x - placeholder_width / 2.0, window_height / 2.0,
         config::MENU_NORMAL_COLOR,
         1.0, // ADDED: Scale factor
     );


    // --- Draw Help Text ---
    let help_text = "Esc: Back";
    let help_width = font.measure_text(help_text);
     renderer.draw_text(
         device, cmd_buf, font, help_text,
         center_x - help_width / 2.0, window_height - 50.0,
         config::MENU_NORMAL_COLOR,
         1.0, // ADDED: Scale factor
     );
}