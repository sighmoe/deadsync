use crate::assets::{AssetManager, FontId, SoundId, TextureId};
use crate::audio::AudioManager;
use crate::config;
use crate::graphics::renderer::{DescriptorSetId, Renderer};
use crate::state::{AppState, MenuState, VirtualKeyCode};
use log::{debug};
use ash::vk;
use winit::event::{ElementState, KeyEvent};

// --- Input Handling ---
pub fn handle_input(
    key_event: &KeyEvent,
    menu_state: &mut MenuState,
    audio_manager: &AudioManager, // Borrow audio manager to play SFX
) -> Option<AppState> {
    // Process only key presses, ignore repeats for menu navigation
    if key_event.state == ElementState::Pressed && !key_event.repeat {
        if let Some(virtual_keycode) = crate::state::key_to_virtual_keycode(key_event.logical_key.clone()) {
            match virtual_keycode {
                VirtualKeyCode::Up => {
                    let old_index = menu_state.selected_index;
                    menu_state.selected_index = if menu_state.selected_index == 0 {
                        menu_state.options.len() - 1
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
                    menu_state.selected_index = (menu_state.selected_index + 1) % menu_state.options.len();
                     if menu_state.selected_index != old_index {
                         audio_manager.play_sfx(SoundId::MenuChange);
                     }
                    debug!("Menu Down: Selected index {}", menu_state.selected_index);
                }
                VirtualKeyCode::Enter => {
                    debug!("Menu Enter: Selected index {}", menu_state.selected_index);
                    audio_manager.play_sfx(SoundId::MenuStart);
                    // Add a small delay to let the sound play slightly before transition
                    // std::thread::sleep(Duration::from_millis(50)); // Consider if needed
                    match menu_state.selected_index {
                        0 => return Some(AppState::Gameplay), // Request transition to Gameplay
                        1 => return Some(AppState::Exiting),  // Request application exit
                        _ => {} // Should not happen with current options
                    }
                }
                VirtualKeyCode::Escape => {
                    debug!("Menu Escape: Exiting");
                    return Some(AppState::Exiting); // Request application exit
                }
                _ => {} // Ignore other keys like Left/Right in menu
            }
        }
    }
    None // No state change requested
}

// --- Update Logic ---
// Menu typically doesn't need per-frame updates unless animating something.
pub fn update(_menu_state: &mut MenuState, _dt: f32) {
    // No-op for now
}

// --- Drawing Logic ---
pub fn draw(
    renderer: &Renderer, // Use the renderer for drawing commands
    menu_state: &MenuState,
    assets: &AssetManager, // Access loaded assets
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
) {
    // Assumes renderer.begin_frame() was called before this.

    // Get necessary assets
    let logo_texture = assets.get_texture(TextureId::Logo).expect("Logo texture not loaded");
    let dancer_texture = assets.get_texture(TextureId::Dancer).expect("Dancer texture not loaded");
    let font = assets.get_font(FontId::Main).expect("Main font not loaded");

    let (window_width, window_height) = renderer.window_size(); // Use getter

    // --- Draw Logo ---
    let aspect_ratio_logo = logo_texture.width as f32 / logo_texture.height.max(1) as f32;
    let logo_height = config::LOGO_DISPLAY_WIDTH / aspect_ratio_logo;
    let logo_x = (window_width - config::LOGO_DISPLAY_WIDTH) / 2.0;
    let logo_y = config::LOGO_Y_POS;
    let logo_center_x = logo_x + config::LOGO_DISPLAY_WIDTH / 2.0;
    let logo_center_y = logo_y + logo_height / 2.0;

    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::Logo,
        cgmath::Vector3::new(logo_center_x, logo_center_y, 0.0),
        (config::LOGO_DISPLAY_WIDTH, logo_height),
        cgmath::Rad(0.0), // No rotation
        [1.0, 1.0, 1.0, 1.0], // White tint
        [0.0, 0.0], // Default UV offset
        [1.0, 1.0], // Default UV scale
    );

    // --- Draw Dancer (overlayed on Logo) ---
    let aspect_ratio_dancer = dancer_texture.width as f32 / dancer_texture.height.max(1) as f32;
    let dancer_height = config::LOGO_DISPLAY_WIDTH / aspect_ratio_dancer; // Scale based on logo width
    // Center the dancer horizontally like the logo
    let dancer_x = logo_x;
    // Center the dancer vertically within the logo's vertical space
    let dancer_y = logo_y + (logo_height / 2.0) - (dancer_height / 2.0);
    let dancer_center_x = dancer_x + config::LOGO_DISPLAY_WIDTH / 2.0;
    let dancer_center_y = dancer_y + dancer_height / 2.0;

    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::Dancer,
        cgmath::Vector3::new(dancer_center_x, dancer_center_y, 0.0),
        (config::LOGO_DISPLAY_WIDTH, dancer_height),
        cgmath::Rad(0.0),
        [1.0, 1.0, 1.0, 1.0],
        [0.0, 0.0],
        [1.0, 1.0],
    );

    // --- Draw Menu Options ---
    let center_x = window_width / 2.0;
    // Calculate Y position based on center, offset, and spacing
    let start_y = window_height / 2.0 + config::MENU_START_Y_OFFSET;
    let spacing_y = font.line_height * config::MENU_ITEM_SPACING;

    for (index, option_text) in menu_state.options.iter().enumerate() {
        let y_pos = start_y + index as f32 * spacing_y;
        let color = if index == menu_state.selected_index {
            config::MENU_SELECTED_COLOR
        } else {
            config::MENU_NORMAL_COLOR
        };

        // Center text horizontally
        let text_width = font.measure_text(option_text);
        let x_pos = center_x - text_width / 2.0;

        // Use the renderer's draw_text function
        renderer.draw_text(
            device, cmd_buf, font, option_text,
            x_pos, y_pos, // Position is baseline
            color,
        );
    }

    // Assumes renderer end_frame / submit happens after this.
}
