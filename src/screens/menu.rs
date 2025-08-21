use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::color;
use crate::ui::components::logo::{self, LogoParams};
use crate::ui::components::menu_list::{self, MenuParams};
use crate::act;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

// new: import the SCREEN_*() getters
use crate::core::space::globals::*;

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

// keep the Metrics arg in the signature (unused), so call sites don't need to change yet
pub fn get_actors(state: &State, _: &crate::core::space::Metrics) -> Vec<Actor> {
    let lp = LogoParams::default();
    let mut actors = logo::build_logo_default();
    actors.reserve(OPTION_COUNT + 2);

    let info2_y_tl = lp.top_margin - INFO_MARGIN_ABOVE - INFO_PX;
    let info1_y_tl = info2_y_tl - INFO_PX - INFO_GAP;
    actors.push(act!(text:
        align(0.5, 0.0):
        xy(screen_center_x(), info1_y_tl):
        px(INFO_PX):
        font("miso"):
        text("DeadSync 0.2.0"):
        talign(center)
    ));
    actors.push(act!(text:
        align(0.5, 0.0):
        xy(screen_center_x(), info2_y_tl):
        px(INFO_PX):
        font("miso"):
        text("X songs in Y groups"):
        talign(center)
    ));

    let selected = color::rgba_hex(SELECTED_COLOR_HEX);
    let normal   = color::rgba_hex(NORMAL_COLOR_HEX);
    let base_y   = lp.top_margin + lp.target_h + MENU_BELOW_LOGO;

    let params = MenuParams {
        options: &MENU_OPTIONS,
        selected_index: state.selected_index,
        start_center_y: base_y + 0.5 * MENU_NORMAL_PX,
        row_spacing:    MENU_ROW_SPACING,
        selected_px:    MENU_SELECTED_PX,
        normal_px:      MENU_NORMAL_PX,
        selected_color: selected,
        normal_color:   normal,
        font:           "wendy",
    };
    actors.extend(menu_list::build_vertical_menu(params));

    actors
}
