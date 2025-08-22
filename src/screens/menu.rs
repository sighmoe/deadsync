use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::color;
use crate::ui::components::logo::{self, LogoParams};
use crate::ui::components::menu_list::{self, MenuParams};
use crate::act;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use std::time::Instant;

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
    // For Simply Love background:
    t0: Instant,
    active_color_index: i32,
    rainbow_mode: bool,
}

pub fn init() -> State {
    State {
        selected_index: 0,
        t0: Instant::now(),
        active_color_index: 0, // change this when you swap palettes
        rainbow_mode: false,   // set true to get white backdrop (RainbowMode)
    }
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
    let mut actors: Vec<Actor> = Vec::with_capacity(64);

    // ---------- Simply Love title-screen background ----------
    // Backdrop: solid black (or white if RainbowMode).
    let w = screen_width();
    let h = screen_height();
    let backdrop = if state.rainbow_mode { [1.0, 1.0, 1.0, 1.0] } else { [0.0, 0.0, 0.0, 1.0] };
    actors.push(act!(quad:
        align(0.0, 0.0):
        xy(0.0, 0.0):
        zoomto(w, h):
        diffuse(backdrop[0], backdrop[1], backdrop[2], backdrop[3]):
        z(-200)
    ));

    // Heart sprites: SharedBackground.png
    // Arrays copied from Simply Love:
    const COLOR_ADD: [i32; 10]       = [-1, 0, 0, -1, -1, -1, 0, 0, 0, 0];
    const DIFFUSE_ALPHA: [f32; 10]   = [0.05, 0.20, 0.10, 0.10, 0.10, 0.10, 0.10, 0.05, 0.10, 0.10];
    const XY: [f32; 10]              = [0.0, 40.0, 80.0, 120.0, 200.0, 280.0, 360.0, 400.0, 480.0, 560.0];
    const VEL: [[f32; 2]; 10] = [
        [ 0.03,  0.01], [ 0.03,  0.02], [ 0.03,  0.01], [ 0.02,  0.02], [ 0.03,  0.03],
        [ 0.02,  0.02], [ 0.03,  0.01], [-0.03,  0.01], [ 0.05,  0.03], [ 0.03,  0.04],
    ];

    // Simply Love main palette (12 colors). Used by GetHexColor(index).
    // These match the common SL hues you’re already using elsewhere.
    const THEME_COLORS: [&str; 12] = [
        "#FF5D47", "#FF577E", "#FF47B3", "#DD57FF", "#8885FF", "#3D94FF",
        "#00B8CC", "#5CE087", "#AEFA44", "#FFFF00", "#FFBE00", "#FF7D00",
    ];

    #[inline(always)]
    fn theme_color_rgba(idx: i32) -> [f32; 4] {
        use crate::ui::color::rgba_hex;
        let n = THEME_COLORS.len() as i32;
        let i = idx.rem_euclid(n) as usize;
        rgba_hex(THEME_COLORS[i])
    }

    // Per-spec: zoom 1.3, position (xy[i], xy[i]), faint alpha, texcoordvelocity.
    let t = state.t0.elapsed().as_secs_f32();
    let heart_w = w * 1.3;
    let heart_h = h * 1.3;

    for i in 0..10 {
        // palette index with offset
        let rgba = {
            let mut c = theme_color_rgba(state.active_color_index + COLOR_ADD[i]);
            c[3] = DIFFUSE_ALPHA[i]; // faint alpha from array
            c
        };

        // UV scroll: wrap via fractional part so it keeps moving forever.
        let mut u = (VEL[i][0] * t).fract();
        let mut v = (VEL[i][1] * t).fract();
        if u < 0.0 { u += 1.0; }
        if v < 0.0 { v += 1.0; }

        actors.push(act!(sprite("hearts_4x4.png"):
            align(0.0, 0.0):
            xy(XY[i], XY[i]):
            zoomto(heart_w, heart_h):
            texrect(u, v, u + 1.0, v + 1.0): // StepMania: customtexturerect(0,0,1,1) + texcoordvelocity
            diffuse(rgba[0], rgba[1], rgba[2], rgba[3]):
            z(-100)
        ));
    }
    // ---------- end Simply Love background ----------

    // existing header/info + logo/menu sit above the background:
    actors.extend(logo::build_logo_default());
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
