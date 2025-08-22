// src/screens/menu.rs
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::color;
use crate::ui::components::logo::{self, LogoParams};
use crate::ui::components::menu_list::{self, MenuParams};
use crate::act;
use rand::prelude::*;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use std::time::Instant;
use rand::rng;

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
    t0: Instant,
    pub active_color_index: i32,
    pub rainbow_mode: bool,
    heart_cells: [[u32; 2]; 10],
}

pub fn init() -> State {
    let mut rng = rng();
    let mut heart_cells = [[0u32; 2]; 10];
    for cell in heart_cells.iter_mut() {
        *cell = [rng.random_range(0..4), rng.random_range(0..4)];
    }

    State {
        selected_index: 0,
        t0: Instant::now(),
        active_color_index: 0,
        rainbow_mode: false,
        heart_cells,
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
    let mut actors: Vec<Actor> = Vec::with_capacity(64);

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

    const COLOR_ADD: [i32; 10]     = [-1, 0, 0, -1, -1, -1, 0, 0, 0, 0];
    const DIFFUSE_ALPHA: [f32; 10] = [0.05, 0.2, 0.1, 0.1, 0.1, 0.1, 0.1, 0.05, 0.1, 0.1];
    const VEL: [[f32; 2]; 10]      = [
        [0.03, 0.01], [0.03, 0.02], [0.03, 0.01], [0.02, 0.02],
        [0.03, 0.03], [0.02, 0.02], [0.03, 0.01], [-0.03, 0.01],
        [0.05, 0.03], [0.03, 0.04],
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

    // --- FIX FOR WIDESCREEN COVERAGE ---
    // To ensure the background covers the entire screen while keeping the hearts square,
    // we determine the largest logical dimension (width or height) and use that
    // as the base for our square sprite's size. This is a "cover" scaling mode.
    let cover_dimension = w.max(h);
    let heart_size = cover_dimension * 1.3;
    let heart_w = heart_size;
    let heart_h = heart_size;
    // --- END FIX ---

    let cx = screen_center_x();
    let cy = screen_center_y();

    for i in 0..10 {
        let rgba = {
            let mut c = theme_color_rgba(state.active_color_index + COLOR_ADD[i]);
            c[3] = DIFFUSE_ALPHA[i];
            c
        };

        let cell_coords = state.heart_cells[i];

        actors.push(act!(sprite("hearts_4x4.png"):
            align(0.5, 0.5):
            xy(cx, cy):
            zoomto(heart_w, heart_h):
            cell(cell_coords[0], cell_coords[1]):
            texcoordvelocity(VEL[i][0], VEL[i][1]):
            blend(add):
            diffuse(rgba[0], rgba[1], rgba[2], rgba[3]):
            z(-100)
        ));
    }

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