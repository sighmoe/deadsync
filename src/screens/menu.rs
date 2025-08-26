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

const MENU_SELECTED_PX: f32 = 23.0;
const MENU_NORMAL_PX: f32 = 19.0;
const MENU_BELOW_LOGO: f32 = 25.0;
const MENU_ROW_SPACING: f32 = 23.0;

const INFO_PX: f32 = 15.0;
const INFO_GAP: f32 = 5.0;
const INFO_MARGIN_ABOVE: f32 = 20.0;

pub struct State {
    pub selected_index: usize,
    pub active_color_index: i32,
    pub rainbow_mode: bool,
    bg: heart_bg::State,
}

pub fn init() -> State {
    State {
        selected_index: 0,
        active_color_index: 0,
        rainbow_mode: false,
        bg: heart_bg::State::new(),
    }
}

pub fn handle_key_press(state: &mut State, event: &KeyEvent) -> ScreenAction {
    if event.state != ElementState::Pressed { return ScreenAction::None; }

    match event.physical_key {
        PhysicalKey::Code(KeyCode::Enter) => match state.selected_index {
            0 => ScreenAction::Navigate(Screen::Gameplay),
            1 => ScreenAction::Navigate(Screen::Options),
            2 => ScreenAction::Exit,
            _ => ScreenAction::None,
        },
        // Escape is now handled globally in app.rs but we can leave this for clarity
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

// Signature changed to accept the alpha_multiplier
pub fn get_actors(state: &State, _: &crate::core::space::Metrics, alpha_multiplier: f32) -> Vec<Actor> {
    let lp = LogoParams::default();
    let mut actors: Vec<Actor> = Vec::with_capacity(96);

    // 1) background component (never fades)
    let backdrop = if state.rainbow_mode { [1.0, 1.0, 1.0, 1.0] } else { [0.0, 0.0, 0.0, 1.0] };
    actors.extend(state.bg.build(heart_bg::Params {
        active_color_index: state.active_color_index,
        backdrop_rgba: backdrop,
    }));

    // If fully faded, don't create the other actors
    if alpha_multiplier <= 0.0 {
        return actors;
    }

    // --- The rest of the function is the same, but uses the passed-in alpha_multiplier ---

    // 2) logo + info
    let info2_y_tl = lp.top_margin - INFO_MARGIN_ABOVE - INFO_PX;
    let info1_y_tl = info2_y_tl - INFO_PX - INFO_GAP;

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
        zoomtoheight(INFO_PX): font("miso"): settext("DeadSync 0.2.207"): horizalign(center):
        diffuse(info_color[0], info_color[1], info_color[2], info_color[3])
    ));
    actors.push(act!(text:
        align(0.5, 0.0): xy(screen_center_x(), info2_y_tl):
        zoomtoheight(INFO_PX): font("miso"): settext("2672 songs in 29 groups, 209 courses"): horizalign(center):
        diffuse(info_color[0], info_color[1], info_color[2], info_color[3])
    ));

    // 3) menu list
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

    // --- footer bar ---
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
