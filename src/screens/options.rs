// ### FILE: /mnt/c/Users/PerfectTaste/Documents/Code/deadsync/src/screens/options.rs ###
use crate::assets::{AssetManager, FontId};
use crate::config;
use crate::graphics::renderer::Renderer;
use crate::state::{AppState, OptionsState, VirtualKeyCode};
use ash::vk;
use log::{debug, trace}; // Added trace
use winit::event::{ElementState, KeyEvent};

// --- Input Handling ---
pub fn handle_input(key_event: &KeyEvent, _state: &mut OptionsState) -> Option<AppState> {
    if key_event.state == ElementState::Pressed && !key_event.repeat {
        if let Some(VirtualKeyCode::Escape) =
            crate::state::key_to_virtual_keycode(key_event.logical_key.clone())
        {
            debug!("Options Escape: Returning to Main Menu");
            return Some(AppState::Menu);
        }
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
    let font = assets
        .get_font(FontId::Wendy)
        .expect("Main font not loaded"); // Use Wendy for Options screen
    let (window_width, window_height) = renderer.window_size();
    let center_x = window_width / 2.0;

    // Define desired pixel sizes for text
    let title_pixel_size: f32 = 48.0 * (window_height / config::UI_REFERENCE_HEIGHT);
    let regular_text_pixel_size: f32 = 32.0 * (window_height / config::UI_REFERENCE_HEIGHT);

    // --- Draw Title ---
    let title = "Options";
    let title_effective_scale = title_pixel_size / font.metrics.em_size.max(1.0);
    let title_width_pixels = font.measure_text_normalized(title) * title_effective_scale;
    let title_baseline_y = 80.0 * (window_height / config::UI_REFERENCE_HEIGHT)
        + (font.metrics.ascender * title_effective_scale);

    trace!(
        "Options title: baseline_y={:.1}, scale={:.2}",
        title_baseline_y,
        title_effective_scale
    );
    renderer.draw_text(
        device,
        cmd_buf,
        font,
        title,
        center_x - title_width_pixels / 2.0,
        title_baseline_y,
        config::MENU_NORMAL_COLOR,
        title_effective_scale,
        None, // No custom spacing for this
    );

    // --- Draw Placeholder Text ---
    let placeholder_text = "Options will go here! (e.g., Volume, Keybinds)";
    let placeholder_effective_scale = regular_text_pixel_size / font.metrics.em_size.max(1.0);
    let placeholder_width_pixels =
        font.measure_text_normalized(placeholder_text) * placeholder_effective_scale;
    let placeholder_baseline_y =
        window_height / 2.0 + (font.metrics.ascender * placeholder_effective_scale);

    trace!(
        "Options placeholder: baseline_y={:.1}, scale={:.2}",
        placeholder_baseline_y,
        placeholder_effective_scale
    );
    renderer.draw_text(
        device,
        cmd_buf,
        font,
        placeholder_text,
        center_x - placeholder_width_pixels / 2.0,
        placeholder_baseline_y,
        config::MENU_NORMAL_COLOR,
        placeholder_effective_scale,
        None, // No custom spacing for this
    );

    // --- Draw Help Text ---
    let help_text = "Esc: Back to Menu";
    let help_effective_scale = regular_text_pixel_size / font.metrics.em_size.max(1.0);
    let help_width_pixels = font.measure_text_normalized(help_text) * help_effective_scale;
    let help_baseline_y = window_height - (60.0 * (window_height / config::UI_REFERENCE_HEIGHT))
        + (font.metrics.ascender * help_effective_scale);

    trace!(
        "Options help: baseline_y={:.1}, scale={:.2}",
        help_baseline_y,
        help_effective_scale
    );
    renderer.draw_text(
        device,
        cmd_buf,
        font,
        help_text,
        center_x - help_width_pixels / 2.0,
        help_baseline_y,
        config::MENU_NORMAL_COLOR,
        help_effective_scale,
        None, // No custom spacing for this
    );
}
