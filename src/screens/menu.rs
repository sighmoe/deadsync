// src/screens/menu.rs
use crate::act;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::color;
use crate::ui::components::logo::{self, LogoParams};
use crate::ui::components::menu_list::{self, MenuParams};
use crate::ui::components::{heart_bg, screen_bar};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

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

const FADE_OUT_SECONDS: f32 = 1.0;

enum MenuMode {
    Idle,
    FadingOut { target: Screen, elapsed: f32 },
}

pub struct State {
    pub selected_index: usize,
    pub active_color_index: i32,
    pub rainbow_mode: bool,
    bg: heart_bg::State,
    mode: MenuMode,
}

pub fn init() -> State {
    State {
        selected_index: 0,
        active_color_index: 0,
        rainbow_mode: false,
        bg: heart_bg::State::new(),
        mode: MenuMode::Idle,
    }
}

pub fn update(state: &mut State, delta_time: f32) -> ScreenAction {
    if let MenuMode::FadingOut { target, elapsed } = &mut state.mode {
        *elapsed += delta_time;
        if *elapsed >= FADE_OUT_SECONDS {
            let final_target = *target;
            state.mode = MenuMode::Idle;
            return ScreenAction::Navigate(final_target);
        }
    }
    ScreenAction::None
}

pub fn handle_key_press(state: &mut State, event: &KeyEvent) -> ScreenAction {
    if event.state != ElementState::Pressed { return ScreenAction::None; }
    if !matches!(state.mode, MenuMode::Idle) { return ScreenAction::None; }

    match event.physical_key {
        PhysicalKey::Code(KeyCode::Enter) => {
            let target = match state.selected_index {
                0 => Some(Screen::Gameplay),
                1 => Some(Screen::Options),
                _ => None,
            };
            if let Some(screen) = target {
                state.mode = MenuMode::FadingOut { target: screen, elapsed: 0.0 };
                return ScreenAction::None;
            }
            if state.selected_index == 2 {
                return ScreenAction::Exit;
            }
            ScreenAction::None
        },
        PhysicalKey::Code(KeyCode::Escape) => ScreenAction::Exit,
        _ => {
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

pub fn get_actors(state: &State, _: &crate::core::space::Metrics) -> Vec<Actor> {
    let lp = LogoParams::default();
    let mut actors: Vec<Actor> = Vec::with_capacity(96);

    // 1) background component (never fades)
    let backdrop = if state.rainbow_mode { [1.0, 1.0, 1.0, 1.0] } else { [0.0, 0.0, 0.0, 1.0] };
    actors.extend(state.bg.build(heart_bg::Params {
        active_color_index: state.active_color_index,
        backdrop_rgba: backdrop,
    }));

    let alpha_multiplier = if let MenuMode::FadingOut { elapsed, .. } = state.mode {
        (1.0 - (elapsed / FADE_OUT_SECONDS)).clamp(0.0, 1.0)
    } else {
        1.0
    };

    if alpha_multiplier <= 0.0 {
        return actors;
    }

    // 2) logo + info
    let info2_y_tl = lp.top_margin - INFO_MARGIN_ABOVE - INFO_PX;
    let info1_y_tl = info2_y_tl - INFO_PX - INFO_GAP;

    // ---- FIX: Correctly iterate and modify logo actors ----
    let logo_actors = logo::build_logo_default();
    for mut actor in logo_actors {
        if let Actor::Sprite { tint, .. } = &mut actor {
            tint[3] *= alpha_multiplier;
        }
        actors.push(actor);
    }

    let mut info_color = [1.0, 1.0, 1.0, 1.0];
    info_color[3] *= alpha_multiplier;

    actors.push(act!(text:
        align(0.5, 0.0): xy(screen_center_x(), info1_y_tl):
        px(INFO_PX): font("miso"): text("DeadSync 0.2.174"): talign(center):
        diffuse(info_color[0], info_color[1], info_color[2], info_color[3])
    ));
    actors.push(act!(text:
        align(0.5, 0.0): xy(screen_center_x(), info2_y_tl):
        px(INFO_PX): font("miso"): text("2672 songs in 29 groups, 209 courses"): talign(center):
        diffuse(info_color[0], info_color[1], info_color[2], info_color[3])
    ));

    // 3) menu list
    // ---- FIX: Define `base_y` before it is used ----
    let base_y = lp.top_margin + lp.target_h + MENU_BELOW_LOGO;

    let mut selected = color::rgba_hex(SELECTED_COLOR_HEX);
    let mut normal = color::rgba_hex(NORMAL_COLOR_HEX);
    selected[3] *= alpha_multiplier;
    normal[3] *= alpha_multiplier;

    let params = MenuParams {
        options: &MENU_OPTIONS,
        selected_index: state.selected_index,
        start_center_y: base_y + 0.5 * MENU_NORMAL_PX,
        row_spacing: MENU_ROW_SPACING,
        selected_px: MENU_SELECTED_PX,
        normal_px: MENU_NORMAL_PX,
        selected_color: selected,
        normal_color: normal,
        font: "wendy",
    };
    actors.extend(menu_list::build_vertical_menu(params));

    // --- footer bar (demonstration) ---
    let mut footer_fg = [1.0, 1.0, 1.0, 1.0];
    footer_fg[3] *= alpha_multiplier;

    actors.push(screen_bar::build(screen_bar::ScreenBarParams {
        title: "EVENT MODE",
        position: screen_bar::ScreenBarPosition::Bottom,
        transparent: true,
        left_text: Some("PRESS START"),
        right_text: Some("PRESS START"),
        fg_color: footer_fg,
    }));

    actors
}
