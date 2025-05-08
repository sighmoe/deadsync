// src/screens/menu.rs
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

    // --- Logo ---
    let logo_display_height = window_height * config::LOGO_HEIGHT_RATIO_TO_WINDOW_HEIGHT;
    let aspect_ratio_logo = logo_texture.width as f32 / logo_texture.height.max(1) as f32;
    let logo_display_width = logo_display_height * aspect_ratio_logo;
    let logo_center_x = center_x;
    let logo_center_y = center_y;
    let logo_bottom_edge_y = logo_center_y + (logo_display_height / 2.0);

    renderer.draw_quad(
        device, cmd_buf, DescriptorSetId::Logo,
        cgmath::Vector3::new(logo_center_x, logo_center_y, 0.0),
        (logo_display_width, logo_display_height),
        cgmath::Rad(0.0), [1.0; 4], [0.0; 2], [1.0; 2],
    );

    // --- Dancer ---
    let dancer_display_width = logo_display_width;
    let aspect_ratio_dancer = dancer_texture.width as f32 / dancer_texture.height.max(1) as f32;
    let dancer_display_height = if aspect_ratio_dancer > 0.0 { dancer_display_width / aspect_ratio_dancer } else { 0.0 };
    if dancer_display_height > 0.0 {
        renderer.draw_quad(
            device, cmd_buf, DescriptorSetId::Dancer,
            cgmath::Vector3::new(logo_center_x, logo_center_y, 0.0),
            (dancer_display_width, dancer_display_height),
            cgmath::Rad(0.0), [1.0; 4], [0.0; 2], [1.0; 2],
        );
    }

    // --- Menu Options ---
    let default_font_scale = 0.4;
    let selected_font_scale = 0.5;

    let unscaled_font_ascent = font.metrics.baseline - font.metrics.top;
    // Using font.line_height (which is metrics.line_spacing) as an approximation for the full visual height of a line of text.
    let default_item_scaled_visual_height = font.line_height * default_font_scale;
    let visual_top_spacing = default_item_scaled_visual_height * config::MENU_ITEM_SPACING * (2.0 / 3.0); // Spacing between visual tops of default-sized items

    let num_options = config::MENU_OPTIONS.len() as f32;
    // This is the total height the block *would* occupy if all items were default size.
    // It's used to center the entire block.
    let total_text_block_height_for_layout = (num_options - 1.0) * visual_top_spacing + default_item_scaled_visual_height;

    let space_for_menu_top_y = logo_bottom_edge_y;
    let space_for_menu_bottom_y = window_height;
    let menu_available_vertical_space = space_for_menu_bottom_y - space_for_menu_top_y;
    let target_menu_block_center_y = space_for_menu_top_y + (menu_available_vertical_space / 2.0);
    let text_block_visual_top_start_y_for_layout = target_menu_block_center_y - (total_text_block_height_for_layout / 2.0);


    for (index, option_text_str) in config::MENU_OPTIONS.iter().enumerate() {
        let current_font_scale;
        let current_color;

        if index == menu_state.selected_index {
            current_font_scale = selected_font_scale;
            current_color = config::MENU_SELECTED_COLOR;
        } else {
            current_font_scale = default_font_scale;
            current_color = config::MENU_NORMAL_COLOR;
        }

        // 1. Determine the Y-coordinate for the *visual center* of the grid slot for this item.
        //    This position is fixed, based on default item sizes.
        let grid_slot_visual_top_y = text_block_visual_top_start_y_for_layout + index as f32 * visual_top_spacing;
        let grid_slot_visual_center_y = grid_slot_visual_top_y + (default_item_scaled_visual_height / 2.0);

        // 2. Calculate the actual scaled visual height of the current item (selected or not).
        let current_item_scaled_visual_height = font.line_height * current_font_scale;

        // 3. Calculate the desired *visual top* of the current item so its center aligns with grid_slot_visual_center_y.
        let current_item_desired_visual_top_y = grid_slot_visual_center_y - (current_item_scaled_visual_height / 2.0);

        // 4. Calculate the baseline Y for draw_text, based on this adjusted visual top.
        let item_scaled_ascent = unscaled_font_ascent * current_font_scale;
        let baseline_y_for_draw_text = current_item_desired_visual_top_y + item_scaled_ascent;

        // Center the text horizontally.
        let text_width = font.measure_text(option_text_str) * current_font_scale;
        let x_pos = center_x - text_width / 2.0;

        renderer.draw_text(
            device, cmd_buf, font, option_text_str,
            x_pos, baseline_y_for_draw_text,
            current_color,
            current_font_scale,
        );
    }
}