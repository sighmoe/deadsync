// src/screens/score.rs
use crate::assets::{AssetManager, FontId};
use crate::config;
use crate::graphics::renderer::Renderer;
use crate::state::{AppState, ScoreScreenState, VirtualKeyCode};
use ash::vk;
use log::debug;
use winit::event::{ElementState, KeyEvent};

pub fn handle_input(key_event: &KeyEvent, _state: &mut ScoreScreenState) -> Option<AppState> {
    if key_event.state == ElementState::Pressed && !key_event.repeat {
        if let Some(virtual_keycode) =
            crate::state::key_to_virtual_keycode(key_event.logical_key.clone())
        {
            match virtual_keycode {
                VirtualKeyCode::Escape | VirtualKeyCode::Enter => {
                    debug!("Score Screen: Escape or Enter pressed, returning to Select Music.");
                    return Some(AppState::SelectMusic);
                }
                _ => {}
            }
        }
    }
    None
}

pub fn update(_state: &mut ScoreScreenState, _dt: f32) {
    // No-op for now
}

pub fn draw(
    renderer: &Renderer,
    _state: &ScoreScreenState,
    assets: &AssetManager,
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
) {
    let font = assets
        .get_font(FontId::Wendy)
        .expect("Wendy font missing for score screen");
    let (window_width, window_height) = renderer.window_size();
    let center_x = window_width / 2.0;
    let center_y = window_height / 2.0;

    let text = "Score Screen (Temporary)";
    let target_pixel_size: f32 = 48.0 * (window_height / config::UI_REFERENCE_HEIGHT);

    // Calculate scale based on em_size and desired pixel size
    let effective_scale = target_pixel_size / font.metrics.em_size.max(1.0);
    let text_width_pixels = font.measure_text_normalized(text) * effective_scale;

    // Center text vertically based on its visual center using ascender and descender
    // The baseline is where draw_text places the bottom of characters like 'g', 'p', 'y'.
    // Visual center of text box = baseline_y - (ascender + descender)/2 * scale
    // So, baseline_y = visual_center_y + (ascender + descender)/2 * scale
    let scaled_ascender = font.metrics.ascender * effective_scale;
    let scaled_descender = font.metrics.descender * effective_scale; // descender is usually negative
    let baseline_y = center_y - (scaled_ascender + scaled_descender) / 2.0;

    renderer.draw_text(
        device,
        cmd_buf,
        font,
        text,
        center_x - text_width_pixels / 2.0,
        baseline_y,
        config::MENU_NORMAL_COLOR,
        effective_scale,
        None,
    );
}
