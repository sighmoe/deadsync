// src/screens/menu.rs
use crate::core::space::Metrics;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::{Actor, Anchor, TextAlign};
use crate::ui::color;
use crate::ui::components::logo::{self, LogoParams};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

const SELECTED_COLOR_HEX: &str = "#ff5d47";
const NORMAL_COLOR_HEX: &str = "#888888";

const OPTION_COUNT: usize = 3;
const MENU_OPTIONS: [&str; OPTION_COUNT] = ["GAMEPLAY", "OPTIONS", "EXIT"];

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
        _ => {
            // Map key to {-1, 0, +1} delta and apply modulo without branches.
            let delta: isize = match event.physical_key {
                PhysicalKey::Code(KeyCode::ArrowUp) | PhysicalKey::Code(KeyCode::KeyW) => -1,
                PhysicalKey::Code(KeyCode::ArrowDown) | PhysicalKey::Code(KeyCode::KeyS) => 1,
                _ => 0,
            };
            if delta != 0 {
                let n = OPTION_COUNT as isize;
                let cur = state.selected_index as isize;
                state.selected_index = ((cur + delta + n) % n) as usize;
            }
            ScreenAction::None
        }
    }
}

pub fn get_actors(state: &State, m: &Metrics) -> Vec<Actor> {
    let screen_width = m.right - m.left;
    let logo_params = LogoParams::default();
    let mut actors = logo::build_logo_default(screen_width);

    // sRGB hex → linear RGBA
    let selected = color::rgba_hex(SELECTED_COLOR_HEX);
    let normal = color::rgba_hex(NORMAL_COLOR_HEX);

    // Calculate menu position relative to the logo's known geometry.
    let logo_bottom_y_tl = logo_params.top_margin + logo_params.target_h;

    for i in 0..OPTION_COUNT {
        let is_selected = i == state.selected_index;
        let px = if is_selected { MENU_SELECTED_PX } else { MENU_NORMAL_PX };
        let color = if is_selected { selected } else { normal };

        let y_tl = logo_bottom_y_tl + MENU_BELOW_LOGO + (i as f32) * MENU_ROW_SPACING;

        actors.push(Actor::Text {
            anchor: Anchor::TopCenter,
            offset: [0.0, y_tl],
            align:  TextAlign::Center,
            px,
            color,
            font:   "wendy",
            content: (MENU_OPTIONS[i]).to_string(),
        });
    }

    actors
}
