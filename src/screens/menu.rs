// ### FILE: /mnt/c/Users/PerfectTaste/Documents/Code/deadsync/src/screens/menu.rs ###
use crate::assets::{AssetManager, FontId, SoundId, TextureId};
use crate::audio::AudioManager;
use crate::config;
use crate::graphics::renderer::{DescriptorSetId, Renderer};
use crate::state::{AppState, MenuState, VirtualKeyCode};
use ash::vk;
use log::{debug, trace}; // Added trace
use winit::event::{ElementState, KeyEvent};

// --- Input Handling ---
pub fn handle_input(
    key_event: &KeyEvent,
    menu_state: &mut MenuState,
    audio_manager: &AudioManager,
) -> Option<AppState> {
    if key_event.state == ElementState::Pressed && !key_event.repeat {
        if let Some(virtual_keycode) =
            crate::state::key_to_virtual_keycode(key_event.logical_key.clone())
        {
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
                    menu_state.selected_index =
                        (menu_state.selected_index + 1) % config::MENU_OPTIONS.len();
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
    let logo_texture = assets
        .get_texture(TextureId::Logo)
        .expect("Logo texture not loaded");
    let dancer_texture = assets
        .get_texture(TextureId::Dancer)
        .expect("Dancer texture not loaded");
    let font = assets
        .get_font(FontId::Wendy)
        .expect("Main font not loaded"); // Using Wendy for menu

    let (window_width, window_height) = renderer.window_size();
    let center_x = window_width / 2.0;
    let center_y = window_height / 2.0;

    // --- Logo ---
    let initial_logo_display_height = window_height * config::LOGO_HEIGHT_RATIO_TO_WINDOW_HEIGHT;
    let aspect_ratio_logo = logo_texture.width as f32 / logo_texture.height.max(1) as f32;
    let initial_logo_display_width = initial_logo_display_height * aspect_ratio_logo;

    let final_logo_display_width;
    let final_logo_display_height;

    if initial_logo_display_width > window_width * 0.9 {
        final_logo_display_width = window_width * 0.9;
        if aspect_ratio_logo > 1e-6 {
            final_logo_display_height = final_logo_display_width / aspect_ratio_logo;
        } else {
            final_logo_display_height = initial_logo_display_height;
        }
    } else {
        final_logo_display_width = initial_logo_display_width;
        final_logo_display_height = initial_logo_display_height;
    }

    let logo_center_x = center_x;
    let logo_center_y = center_y * 0.75; // Adjusted to move logo slightly higher for more menu space
    let logo_bottom_edge_y = logo_center_y + (final_logo_display_height / 2.0);

    renderer.draw_quad(
        device,
        cmd_buf,
        DescriptorSetId::Logo,
        cgmath::Vector3::new(logo_center_x, logo_center_y, 0.0),
        (final_logo_display_width, final_logo_display_height),
        cgmath::Rad(0.0),
        [1.0; 4],
        [0.0; 2],
        [1.0; 2],
    );

    // --- Dancer ---
    let dancer_display_width = final_logo_display_width * 0.8;
    let aspect_ratio_dancer = dancer_texture.width as f32 / dancer_texture.height.max(1) as f32;
    let dancer_display_height = if aspect_ratio_dancer > 1e-6 {
        dancer_display_width / aspect_ratio_dancer
    } else {
        0.0
    };

    if dancer_display_height > 0.0 {
        renderer.draw_quad(
            device,
            cmd_buf,
            DescriptorSetId::Dancer,
            cgmath::Vector3::new(logo_center_x, logo_center_y, 0.0),
            (dancer_display_width, dancer_display_height),
            cgmath::Rad(0.0),
            [1.0; 4],
            [0.0; 2],
            [1.0; 2],
        );
    }

    // --- Menu Options ---
    let default_item_pixel_size: f32 = 42.0 * (window_height / config::UI_REFERENCE_HEIGHT);
    let selected_item_pixel_size: f32 = 50.0 * (window_height / config::UI_REFERENCE_HEIGHT);

    let unscaled_font_ascender = font.metrics.ascender;
    let unscaled_font_line_height = font.metrics.line_height;

    let default_effective_scale = default_item_pixel_size / font.metrics.em_size.max(1.0);
    let selected_effective_scale = selected_item_pixel_size / font.metrics.em_size.max(1.0);

    let menu_item_baseline_spacing = unscaled_font_line_height * default_effective_scale * 1.5;

    let num_options = config::MENU_OPTIONS.len() as f32;
    // Calculate total height of the menu block as if all items were default size for consistent layout
    let total_text_block_height_default_items = (num_options - 1.0) * menu_item_baseline_spacing
        + (unscaled_font_line_height * default_effective_scale);

    let space_for_menu_top_y =
        logo_bottom_edge_y + 40.0 * (window_height / config::UI_REFERENCE_HEIGHT); // Increased padding
    let space_for_menu_bottom_y =
        window_height - 20.0 * (window_height / config::UI_REFERENCE_HEIGHT);
    let menu_available_vertical_space = (space_for_menu_bottom_y - space_for_menu_top_y).max(0.0);

    let target_menu_block_center_y = space_for_menu_top_y + (menu_available_vertical_space / 2.0);

    // Baseline Y for the *first* menu item IF IT WERE DEFAULT SIZE, such that the block of default-sized items is centered.
    let first_item_default_baseline_y = target_menu_block_center_y
        - (total_text_block_height_default_items / 2.0)
        + (unscaled_font_ascender * default_effective_scale);

    for (index, option_text_str) in config::MENU_OPTIONS.iter().enumerate() {
        let current_effective_scale;
        let current_color;
        let item_is_selected = index == menu_state.selected_index;

        if item_is_selected {
            current_effective_scale = selected_effective_scale;
            current_color = config::MENU_SELECTED_COLOR;
        } else {
            current_effective_scale = default_effective_scale;
            current_color = config::MENU_NORMAL_COLOR;
        }

        // Calculate the "neutral" baseline Y for this item (where its baseline would be if it were default size)
        let neutral_baseline_y =
            first_item_default_baseline_y + (index as f32 * menu_item_baseline_spacing);
        let mut actual_baseline_y_for_draw_text = neutral_baseline_y;

        if item_is_selected {
            // The item is larger. We want its visual center to align with the
            // visual center of where it would have been if it were default size.
            // The text rendering is relative to the baseline.
            // We use unscaled_font_line_height as a proxy for the full "box height" of the text line.

            let default_scaled_line_height = unscaled_font_line_height * default_effective_scale;
            let selected_scaled_line_height = unscaled_font_line_height * current_effective_scale; // current_effective_scale is selected_effective_scale here

            // The difference in total line height
            let height_increase = selected_scaled_line_height - default_scaled_line_height;

            // To make it expand from the center, the baseline (which is near the "bottom" of the text)
            // needs to be shifted downwards relative to its neutral position by half the height increase.
            // (Y is down, so + moves it down on screen).
            actual_baseline_y_for_draw_text = neutral_baseline_y + (height_increase / 2.0);
        }

        let text_width_pixels =
            font.measure_text_normalized(option_text_str) * current_effective_scale;
        let x_pos = center_x - text_width_pixels / 2.0;

        trace!("Menu item '{}': neutral_baseline_y={:.1}, actual_baseline_y={:.1} x_pos={:.1}, scale={:.2}, selected={}", 
            option_text_str, neutral_baseline_y, actual_baseline_y_for_draw_text, x_pos, current_effective_scale, item_is_selected);

        renderer.draw_text(
            device,
            cmd_buf,
            font,
            option_text_str,
            x_pos,
            actual_baseline_y_for_draw_text,
            current_color,
            current_effective_scale,
        );
    }
}
