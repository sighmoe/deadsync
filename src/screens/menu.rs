// src/screens/menu.rs
use crate::core::space::Metrics;
use crate::screens::{Screen, ScreenAction};
use crate::ui::components::logo::build_logo_default;
use crate::ui::msdf;
use crate::ui::primitives::UIElement;
use crate::text;
use std::collections::HashMap;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

const OPTION_COUNT: usize = 3;
const MENU_OPTIONS: [&str; OPTION_COUNT] = ["GAMEPLAY", "OPTIONS", "EXIT"];

const MENU_SELECTED_COLOR: [f32; 4] = [1.0, 1.0, 0.5, 1.0];
const MENU_NORMAL_COLOR: [f32; 4] = [0.8, 0.8, 0.8, 1.0];

const MENU_SELECTED_PX: f32 = 50.0;
const MENU_NORMAL_PX: f32 = 42.0;
const MENU_BELOW_LOGO: f32 = 28.0;
const MENU_ROW_SPACING: f32 = 36.0;

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
            state.selected_index = if state.selected_index == 0 {
                OPTION_COUNT - 1
            } else {
                state.selected_index - 1
            };
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

pub fn get_ui_elements(
    state: &State,
    m: &Metrics,
    fonts: &HashMap<&'static str, msdf::Font>,
) -> Vec<UIElement> {
    let logo = build_logo_default(m, fonts);

    let top_to_logo_bottom = m.top - logo.logo_bottom_y;
    let mut actors = Vec::with_capacity(OPTION_COUNT);

    for i in 0..OPTION_COUNT {
        let is_selected = i == state.selected_index;
        let px = if is_selected { MENU_SELECTED_PX } else { MENU_NORMAL_PX };
        let color = if is_selected { MENU_SELECTED_COLOR } else { MENU_NORMAL_COLOR };
        let label = MENU_OPTIONS[i];

        // y in SM top-left pixels
        let y_tl = top_to_logo_bottom + MENU_BELOW_LOGO + (i as f32) * MENU_ROW_SPACING;

        actors.push(text!(
            anchor: TopCenter,
            offset: [0, y_tl], // Position baseline; alignment is separate
            align: Center,     // <-- This does the real centering
            px: px,
            color: color,
            text: label
        ));
    }

    let mut elements = logo.ui;
    elements.extend(crate::ui::actors::build_actors(&actors, m, fonts));
    elements
}
