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

const MENU_SELECTED_PX: f32 = 28.0;
const MENU_NORMAL_PX: f32 = 23.0;
const MENU_BELOW_LOGO: f32 = 28.0;
const MENU_ROW_SPACING: f32 = 23.0;

const INFO_PX: f32 = 15.0;
const INFO_GAP: f32 = 5.0;
const INFO_MARGIN_ABOVE: f32 = 20.0;

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
    let lp = LogoParams::default();
    let mut actors = logo::build_logo_default(screen_width);
    actors.reserve(OPTION_COUNT + 2);

    let info2_y_tl = lp.top_margin - INFO_MARGIN_ABOVE - INFO_PX;
    let info1_y_tl = info2_y_tl - INFO_PX - INFO_GAP;
    let white = [1.0, 1.0, 1.0, 1.0];

    actors.push(Actor::Text {
        anchor:  Anchor::TopCenter,
        offset:  [0.0, info1_y_tl],
        align:   TextAlign::Center,
        px:      INFO_PX,
        color:   white,
        font:    "miso",
        content: "DeadSync 0.2.0".to_string(),
    });
    actors.push(Actor::Text {
        anchor:  Anchor::TopCenter,
        offset:  [0.0, info2_y_tl],
        align:   TextAlign::Center,
        px:      INFO_PX,
        color:   white,
        font:    "miso",
        content: "X songs in Y groups".to_string(),
    });

    let selected = color::rgba_hex(SELECTED_COLOR_HEX);
    let normal   = color::rgba_hex(NORMAL_COLOR_HEX);
    let base_y   = lp.top_margin + lp.target_h + MENU_BELOW_LOGO;

    for (i, label) in MENU_OPTIONS.iter().enumerate() {
        let sel   = i == state.selected_index;
        let px    = if sel { MENU_SELECTED_PX } else { MENU_NORMAL_PX };
        let color = if sel { selected } else { normal };
        let y_tl  = base_y + (i as f32) * MENU_ROW_SPACING;

        actors.push(Actor::Text {
            anchor:  Anchor::TopCenter,
            offset:  [0.0, y_tl],
            align:   TextAlign::Center,
            px,
            color,
            font:    "wendy",
            content: (*label).to_string(),
        });
    }
    actors
}
