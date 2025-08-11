// src/screens/menu.rs
use crate::ui::primitives::{Text, UIElement};
use crate::screens::{Screen, ScreenAction};
use crate::utils::layout::Metrics;
use crate::ui::components::logo::{build_logo_default, LogoParams, build_logo};
use cgmath::Vector2;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

const OPTION_COUNT: usize = 3;
const MENU_OPTIONS: [&str; OPTION_COUNT] = ["GAMEPLAY", "OPTIONS", "EXIT"];

// colors
const MENU_SELECTED_COLOR: [f32; 4] = [1.0, 1.0, 0.5, 1.0];
const MENU_NORMAL_COLOR:   [f32; 4] = [0.8, 0.8, 0.8, 1.0];

// menu text sizing/spacing
const MENU_SELECTED_PX: f32 = 50.0;
const MENU_NORMAL_PX:   f32 = 42.0;
const MENU_BELOW_LOGO:  f32 = 28.0;   // gap from logo bottom
const MENU_ROW_SPACING: f32 = 70.0;

// crude width estimate factor for centering MSDF text
const TEXT_WIDTH_K: f32 = 0.45;

pub struct State {
    pub selected_index: usize,
}

pub fn init() -> State {
    State { selected_index: 0 }
}

pub fn handle_key_press(state: &mut State, event: &KeyEvent) -> ScreenAction {
    if event.state != ElementState::Pressed {
        return ScreenAction::None;
    }

    match event.physical_key {
        PhysicalKey::Code(KeyCode::ArrowUp) | PhysicalKey::Code(KeyCode::KeyW) => {
            state.selected_index =
                if state.selected_index == 0 { OPTION_COUNT - 1 } else { state.selected_index - 1 };
        }
        PhysicalKey::Code(KeyCode::ArrowDown) | PhysicalKey::Code(KeyCode::KeyS) => {
            state.selected_index = (state.selected_index + 1) % OPTION_COUNT;
        }
        PhysicalKey::Code(KeyCode::Enter) => {
            return match state.selected_index {
                0 => ScreenAction::Navigate(Screen::Gameplay),
                1 => ScreenAction::Navigate(Screen::Options),
                2 => ScreenAction::Exit,
                _ => ScreenAction::None,
            };
        }
        PhysicalKey::Code(KeyCode::Escape) => {
            return ScreenAction::Exit;
        }
        _ => {}
    }
    ScreenAction::None
}

pub fn get_ui_elements(state: &State, m: &Metrics) -> Vec<UIElement> {
    let mut elements = Vec::with_capacity(2 + OPTION_COUNT);

    // ----- logo component -----
    // Use defaults:
    // let logo = build_logo_default(m);
    //
    // Or tweak if you want (example keeps your current "perfect" defaults):
    let logo = build_logo(m, LogoParams {
        target_h: 238.0,
        top_margin: 102.0,
        banner_y_offset_inside: 0.0,
    });

    elements.extend(logo.ui);

    // ----- menu options under the logo -----
    let y_start = logo.logo_bottom_y - MENU_BELOW_LOGO;

    for i in 0..OPTION_COUNT {
        let is_selected = i == state.selected_index;
        let px = if is_selected { MENU_SELECTED_PX } else { MENU_NORMAL_PX };
        let color = if is_selected { MENU_SELECTED_COLOR } else { MENU_NORMAL_COLOR };

        let label = MENU_OPTIONS[i];
        let est_w = label.len() as f32 * px * TEXT_WIDTH_K;

        let baseline_y = y_start - i as f32 * MENU_ROW_SPACING;
        let origin = Vector2::new(-0.5 * est_w, baseline_y); // center by shifting left half the width

        elements.push(UIElement::Text(Text {
            origin,
            pixel_height: px,
            color,
            font_id: "wendy",
            content: label.to_string(),
        }));
    }

    elements
}
