use crate::assets::{AssetManager, FontId, SoundId, TextureId};
use crate::audio::AudioManager;
use crate::config;
use crate::graphics::renderer::{DescriptorSetId, Renderer};
use crate::state::{AppState, MenuState, VirtualKeyCode};
use log::{debug, trace};
use ash::vk;
use winit::event::{ElementState, KeyEvent};

// --- Input Handling ---
// ... (no changes) ...
pub fn handle_input(
    key_event: &KeyEvent,
    menu_state: &mut MenuState,
    audio_manager: &AudioManager,
) -> Option<AppState> {
    if key_event.state == ElementState::Pressed && !key_event.repeat {
        if let Some(virtual_keycode) = crate::state::key_to_virtual_keycode(key_event.logical_key.clone()) {
            match virtual_keycode {
                VirtualKeyCode::Up => {
                    let old_index = menu_state.selected_index;
                    menu_state.selected_index = if menu_state.selected_index == 0 {
                        config::MENU_OPTIONS.len() - 1
                    } else {
                        menu_state.selected_index - 1
                    };
                    if menu_state.selected_index != old_index {
                        audio_manager.play_sfx(SoundId::MenuChange);
                    }
                    debug!("Menu Up: Selected index {}", menu_state.selected_index);
                }
                VirtualKeyCode::Down => {
                    let old_index = menu_state.selected_index;
                    menu_state.selected_index = (menu_state.selected_index + 1) % config::MENU_OPTIONS.len();
                     if menu_state.selected_index != old_index {
                         audio_manager.play_sfx(SoundId::MenuChange);
                     }
                    debug!("Menu Down: Selected index {}", menu_state.selected_index);
                }
                VirtualKeyCode::Enter => {
                    debug!("Menu Enter: Selected index {}", menu_state.selected_index);
                    audio_manager.play_sfx(SoundId::MenuStart);

                    match menu_state.selected_index {
                        0 => return Some(AppState::SelectMusic),
                        1 => return Some(AppState::Options),
                        2 => return Some(AppState::Exiting),
                        _ => {}
                    }
                }
                VirtualKeyCode::Escape => {
                    debug!("Menu Escape: Exiting");
                    return Some(AppState::Exiting);
                }
                _ => {}
            }
        }
    }
    None
}

// --- Update Logic ---
pub fn update(_menu_state: &mut MenuState, _dt: f32) {
    // No-op for now
}

// --- Drawing Logic ---
pub fn draw(
    renderer: &Renderer,
    menu_state: &MenuState,
    assets: &AssetManager,
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
) {
    let logo_texture = assets.get_texture(TextureId::Logo).expect("Logo texture not loaded");
    let dancer_texture = assets.get_texture(TextureId::Dancer).expect("Dancer texture not loaded");
    let font = assets.get_font(FontId::Main).expect("Main font not loaded");

    let (window_width, window_height) = renderer.window_size();
    let center_x = window_width / 2.0;
    let center_y = window_height / 2.0;
    trace!("Menu draw - Window size: {}x{}", window_width, window_height);

    // --- Calculate Dynamic Logo Size (based on HEIGHT) ---
    let logo_display_height = window_height * config::LOGO_HEIGHT_RATIO_TO_WINDOW_HEIGHT;
    let aspect_ratio_logo = logo_texture.width as f32 / logo_texture.height.max(1) as f32;
    let logo_display_width = logo_display_height * aspect_ratio_logo;

    // --- Position Logo Centered ---
    let logo_center_x = center_x;
    let logo_center_y = center_y;

    trace!("Logo dynamic height: {}, width: {}, center_x: {}, center_y: {}",
           logo_display_height, logo_display_width, logo_center_x, logo_center_y);

    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::Logo,
        cgmath::Vector3::new(logo_center_x, logo_center_y, 0.0),
        (logo_display_width, logo_display_height),
        cgmath::Rad(0.0),
        [1.0, 1.0, 1.0, 1.0],
        [0.0, 0.0],
        [1.0, 1.0],
    );

    // --- Draw Dancer (Width matches logo width, Height derived from dancer aspect ratio) ---
    // Set dancer width to match the calculated logo width
    let dancer_display_width = logo_display_width; // <-- KEY CHANGE: Base dancer scale on logo width
    // Calculate dancer height based on ITS width and ITS aspect ratio
    let aspect_ratio_dancer = dancer_texture.width as f32 / dancer_texture.height.max(1) as f32;
    // Avoid division by zero if aspect ratio is invalid
    let dancer_display_height = if aspect_ratio_dancer > 0.0 {
        dancer_display_width / aspect_ratio_dancer
    } else {
        0.0 // Or some fallback height
    };

    // Position centered on logo
    let dancer_center_x = logo_center_x;
    let dancer_center_y = logo_center_y;

    trace!("Dancer dynamic width: {}, height: {}, center_x: {}, center_y: {}",
           dancer_display_width, dancer_display_height, dancer_center_x, dancer_center_y);

    // Only draw if height is valid
    if dancer_display_height > 0.0 {
        renderer.draw_quad(
            device, cmd_buf, DescriptorSetId::Dancer,
            cgmath::Vector3::new(dancer_center_x, dancer_center_y, 0.0),
            (dancer_display_width, dancer_display_height), // Use dynamic width and height
            cgmath::Rad(0.0),
            [1.0, 1.0, 1.0, 1.0],
            [0.0, 0.0],
            [1.0, 1.0],
        );
    }


    // --- Draw Menu Options (Corrected Y position for top-left draw_text) ---
    let font_scale = 0.5;
    let scaled_line_height = font.line_height * font_scale;
    let item_y_spacing = scaled_line_height * config::MENU_ITEM_SPACING * (2.0 / 3.0);
    let num_options = config::MENU_OPTIONS.len() as f32;
    let total_text_block_height = (num_options - 1.0) * item_y_spacing + scaled_line_height;

    let text_block_bottom_y = window_height * (1.0 - config::MENU_TEXT_BOTTOM_MARGIN_RATIO);
    let text_block_desired_top_y = text_block_bottom_y - total_text_block_height;

    trace!("Menu text_block_desired_top_y (from bottom margin): {}", text_block_desired_top_y);

    for (index, option_text_str) in config::MENU_OPTIONS.iter().enumerate() {
        let y_pos_top = text_block_desired_top_y + index as f32 * item_y_spacing;

        let color = if index == menu_state.selected_index {
            config::MENU_SELECTED_COLOR
        } else {
            config::MENU_NORMAL_COLOR
        };

        let text_width = font.measure_text(option_text_str) * font_scale;
        let x_pos = center_x - text_width / 2.0;

        renderer.draw_text(
            device, cmd_buf, font, option_text_str,
            x_pos, y_pos_top, // Pass top-left Y
            color,
            font_scale,
        );
    }
}