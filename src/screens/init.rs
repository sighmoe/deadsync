// src/screens/init.rs
use crate::act;
use crate::core::space::globals::*;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::components::heart_bg;
use winit::event::{KeyEvent};

/* ----------------------- timing & layout ----------------------- */

// arrows (matches the simple SM-like splash)
const ARROW_COUNT: usize   = 7;
const ARROW_SPACING: f32   = 50.0;
const ARROW_BASE_DELAY: f32 = 0.20;
const ARROW_STEP_DELAY: f32 = 0.10;
const ARROW_FADE_IN: f32    = 0.75;
const ARROW_FADE_OUT: f32   = 0.75;

// black bar behind arrows
const BAR_ACCEL: f32     = 0.30;
const BAR_TARGET_H: f32  = 128.0;
const BAR_ALPHA: f32     = 0.90;
const BAR_SLEEP: f32     = 2.10;

// auto-advance (computed from the slowest arrow finishing)
#[inline(always)]
fn auto_to_menu_at() -> f32 {
    let last_delay = ARROW_BASE_DELAY + ARROW_STEP_DELAY * (ARROW_COUNT as f32);
    // tiny pad so the very last tween can snap invisible(false)
    last_delay + ARROW_FADE_IN + ARROW_FADE_OUT + 0.05
}

/* ---------------------------- state ---------------------------- */

pub struct State {
    elapsed: f32,
    /// Fixed palette base chosen once for this screen (no cycling).
    base_color_index: i32,
    bg: heart_bg::State,
}

pub fn init() -> State {
    State {
        elapsed: 0.0,
        base_color_index: 0,          // keep the splash stable; change if you want variety
        bg: heart_bg::State::new(),
    }
}

/* -------------------------- input -> nav ----------------------- */

// Block ALL input during the splash. Let it auto-advance only.
pub fn handle_key_press(_: &mut State, _: &KeyEvent) -> ScreenAction {
    ScreenAction::None
}

/* ---------------------------- update --------------------------- */

pub fn update(state: &mut State, dt: f32) -> ScreenAction {
    state.elapsed += dt;
    if state.elapsed >= auto_to_menu_at() {
        ScreenAction::Navigate(Screen::Menu)
    } else {
        ScreenAction::None
    }
}

/* --------------------------- drawing --------------------------- */

pub fn get_actors_bg_only(state: &State, _m: &crate::core::space::Metrics) -> Vec<Actor> {
    let mut actors: Vec<Actor> = Vec::with_capacity(16);
    actors.extend(state.bg.build(heart_bg::Params {
        active_color_index: state.base_color_index,
        backdrop_rgba: [0.0, 0.0, 0.0, 1.0],
    }));
    actors
}

pub fn get_actors(state: &State, _m: &crate::core::space::Metrics) -> Vec<Actor> {
    let mut actors: Vec<Actor> = Vec::with_capacity(32 + ARROW_COUNT);

    // 1) HEART BACKGROUND — keep a single palette color for the short splash
    actors.extend(state.bg.build(heart_bg::Params {
        active_color_index: state.base_color_index,
        backdrop_rgba: [0.0, 0.0, 0.0, 1.0],
    }));

    // 2) dark bar behind the arrows (grows in)
    let w  = screen_width();
    let cy = screen_center_y();

    actors.push(act!(quad:
        align(0.0, 0.0):
        xy(0.0, cy - BAR_TARGET_H * 0.5):
        zoomto(w, 0.0):
        diffuse(0.0, 0.0, 0.0, 0.0):
        z(105):
        accelerate(BAR_ACCEL): zoomtoheight(BAR_TARGET_H): alpha(BAR_ALPHA):
        sleep(BAR_SLEEP)
    ));

    // 3) the 7 rainbow arrows with staggered delays
    let cx = screen_center_x();
    for i in 1..=ARROW_COUNT {
        let x     = (i as f32 - 4.0) * ARROW_SPACING;
        let delay = ARROW_BASE_DELAY + ARROW_STEP_DELAY * (i as f32);

        // fixed rainbow mapping (no time-based cycling)
        let tint = palette_color(state.base_color_index - i as i32 - 4);

        actors.push(act!(sprite("init_arrow.png"):
            align(0.5, 0.5):
            xy(cx + x, cy):
            z(110):
            zoomto(51.0, 51.0):
            diffuse(tint[0], tint[1], tint[2], 0.0):
            sleep(delay):
            linear(ARROW_FADE_IN): alpha(1.0):
            linear(ARROW_FADE_OUT): alpha(0.0):
            linear(0.0): visible(false)
        ));
    }

    actors
}

/* ------------------------- color helper ------------------------ */

fn palette_color(idx: i32) -> [f32; 4] {
    const HEX: [&str; 7] = [
        "#FFFF00", "#AEFA44", "#5CE087", "#00ADC0", "#0073FF", "#413AD0", "#8200A1"
    ];
    crate::ui::color::rgba_hex(HEX[idx.rem_euclid(HEX.len() as i32) as usize])
}
