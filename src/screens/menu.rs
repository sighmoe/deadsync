// src/screens/menu.rs
use crate::act;
use crate::core::space::globals::*;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::color;
use crate::ui::components::logo::{self, LogoParams};
use crate::ui::components::menu_list::{self, MenuParams};
use image;
use std::time::Instant;
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

// Tint/placement (from theme)
const COLOR_ADD: [i32; 10]     = [-1, 0, 0, -1, -1, -1, 0, 0, 0, 0];
const DIFFUSE_ALPHA: [f32; 10] = [0.0125, 0.05, 0.025, 0.025, 0.025, 0.025, 0.025, 0.0125, 0.025, 0.025];
const XY: [f32; 10]            = [0.0, 40.0, 80.0, 120.0, 200.0, 280.0, 360.0, 400.0, 480.0, 560.0];

// “UV velocities” we reinterpret as screen px/sec scaling
const UV_VEL: [[f32; 2]; 10] = [
    [ 0.03, 0.01], [ 0.03, 0.02], [ 0.03, 0.01], [ 0.02, 0.02],
    [ 0.03, 0.03], [ 0.02, 0.02], [ 0.03, 0.01], [-0.03, 0.01],
    [ 0.05, 0.03], [ 0.03, 0.04],
];

const THEME_COLORS: [&str; 12] = [
    "#C1006F", "#8200A1", "#413AD0", "#0073FF", "#00ADC0", "#5CE087",
    "#AEFA44", "#FFFF00", "#FFBE00", "#FF7D00", "#FF3C23", "#FF003C",
];

#[inline(always)]
fn theme_color_rgba(idx: i32) -> [f32; 4] {
    let n = THEME_COLORS.len() as i32;
    let i = idx.rem_euclid(n) as usize;
    color::rgba_hex(THEME_COLORS[i])
}

pub struct State {
    pub selected_index: usize,
    t0: Instant,
    pub active_color_index: i32,

    // heart.png intrinsic size
    base_w: f32,
    base_h: f32,

    // per-heart size variant: 0=normal, 1=big, 2=small
    variants: [usize; 10],
}

pub fn init() -> State {
    // Read heart.png size
    let (w_px, h_px) = image::image_dimensions("assets/graphics/heart.png")
        .unwrap_or((320, 271)); // assume the “big” one if missing
    let base_w = w_px as f32;
    let base_h = h_px as f32;

    // Deterministic spread of sizes across 10 hearts
    // (normal, big, small, normal, big, normal, small, normal, big, small)
    let variants = [0, 1, 2, 0, 1, 0, 2, 0, 1, 2];

    State {
        selected_index: 0,
        t0: Instant::now(),
        active_color_index: 0,
        base_w,
        base_h,
        variants,
    }
}

pub fn handle_key_press(state: &mut State, event: &KeyEvent) -> ScreenAction {
    if event.state != ElementState::Pressed {
        return ScreenAction::None;
    }
    match event.physical_key {
        PhysicalKey::Code(KeyCode::Enter) => match state.selected_index {
            0 => ScreenAction::Navigate(Screen::Gameplay),
            1 => ScreenAction::Navigate(Screen::Options),
            2 => ScreenAction::Exit,
            _ => ScreenAction::None,
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

    // --- backdrop ---
    let w = screen_width();
    let h = screen_height();
    actors.push(act!(quad:
        align(0.0, 0.0):
        xy(0.0, 0.0):
        zoomto(w, h):
        diffuse(0.0, 0.0, 0.0, 1.0):
        z(-200)
    ));

    // Maintain aspect ratio of the source
    let aspect = state.base_h / state.base_w; // height/width

    // We’ll scale to match your “zoom ~1.3” feel for the big heart,
    // and deduce normal/small from the original pixel widths.
    // Original widths you gave:
    const BW_BIG: f32 = 320.0;
    const BW_NORMAL: f32 = 256.0;
    const BW_SMALL: f32 = 192.0;

    // Make "big" ≈ base_w * 1.3; others follow the same relative ratios.
    let scale_k = (state.base_w * 1.3) / BW_BIG;
    let var_w = [BW_NORMAL * scale_k, BW_BIG * scale_k, BW_SMALL * scale_k]; // [normal, big, small]
    let var_h = [var_w[0] * aspect, var_w[1] * aspect, var_w[2] * aspect];

    // Motion speed scale (same feel you liked), and left/up movement (double speed)
    let speed_scale_px = w.max(h) * 1.3;
    let t = state.t0.elapsed().as_secs_f32();

    const PHI: f32 = 0.618_033_988_75; // spread

    for i in 0..10 {
        let variant = state.variants[i]; // 0=normal,1=big,2=small
        let heart_w = var_w[variant];
        let heart_h = var_h[variant];
        let half_w = heart_w * 0.5;
        let half_h = heart_h * 0.5;

        let mut rgba = theme_color_rgba(state.active_color_index + COLOR_ADD[i]);
        rgba[3] = DIFFUSE_ALPHA[i];

        // left & up; doubled
        let vx_px = -2.0 * UV_VEL[i][0] * speed_scale_px;
        let vy_px = -2.0 * UV_VEL[i][1] * speed_scale_px;

        // seed positions across the screen
        let start_x = (XY[i] + (i as f32) * (w / 10.0)) % w;
        let start_y = (XY[i] * 0.5 + (i as f32) * (h / 10.0) * PHI) % h;

        let x_raw = start_x + vx_px * t;
        let y_raw = start_y + vy_px * t;

        // canonical position in [0,w)×[0,h)
        let x0 = x_raw.rem_euclid(w);
        let y0 = y_raw.rem_euclid(h);

        // wrap-around copies by size so there’s no blinking when crossing edges
        let mut x_offsets = [0.0f32; 3];
        let mut y_offsets = [0.0f32; 3];
        let mut nx = 1usize;
        let mut ny = 1usize;

        if x0 < half_w { x_offsets[nx] =  w; nx += 1; }
        if x0 > w - half_w { x_offsets[nx] = -w; nx += 1; }
        if y0 < half_h { y_offsets[ny] =  h; ny += 1; }
        if y0 > h - half_h { y_offsets[ny] = -h; ny += 1; }

        for xi in 0..nx {
            for yi in 0..ny {
                let x = x0 + x_offsets[xi];
                let y = y0 + y_offsets[yi];

                actors.push(act!(sprite("heart.png"):
                    align(0.5, 0.5):
                    xy(x, y):
                    zoomto(heart_w, heart_h): // preserve aspect
                    diffuse(rgba[0], rgba[1], rgba[2], rgba[3]):
                    z(-100)
                ));
            }
        }
    }

    // --- logo + menu ---
    actors.extend(logo::build_logo_default());
    actors.reserve(OPTION_COUNT + 2);

    let info2_y_tl = lp.top_margin - INFO_MARGIN_ABOVE - INFO_PX;
    let info1_y_tl = info2_y_tl - INFO_PX - INFO_GAP;
    actors.push(act!(text:
        align(0.5, 0.0): xy(screen_center_x(), info1_y_tl):
        px(INFO_PX): font("miso"): text("DeadSync 0.2.0"): talign(center)
    ));
    actors.push(act!(text:
        align(0.5, 0.0): xy(screen_center_x(), info2_y_tl):
        px(INFO_PX): font("miso"): text("X songs in Y groups"): talign(center)
    ));

    let selected = color::rgba_hex(SELECTED_COLOR_HEX);
    let normal = color::rgba_hex(NORMAL_COLOR_HEX);
    let base_y = lp.top_margin + lp.target_h + MENU_BELOW_LOGO;

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

    actors
}
