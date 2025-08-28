use crate::act;
use crate::core::space::globals::*;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::components::heart_bg;
use crate::ui::color;
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
const BAR_TARGET_H: f32  = 128.0;

const SQUISH_START_DELAY: f32 = 0.50;   // NEW: pause before unsquish begins
const SQUISH_IN_DURATION: f32 = 0.35;

/* ----------------------- auto-advance ----------------------- */
#[inline(always)]
fn auto_to_menu_at() -> f32 {
    let last_delay = ARROW_BASE_DELAY + ARROW_STEP_DELAY * (ARROW_COUNT as f32);
    // wait → unsquish → arrows fade in/out → tiny pad
    SQUISH_START_DELAY + SQUISH_IN_DURATION + last_delay + ARROW_FADE_IN + ARROW_FADE_OUT + 0.05
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
        base_color_index: color::DEFAULT_COLOR_INDEX,
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
        alpha_mul: 1.0,
    }));
    actors
}

// A single black bar that collapses to the center as `progress` goes 0→1.
pub fn build_squish_bar(progress: f32) -> Actor {
    let w  = screen_width();
    let cy = screen_center_y();

    let t = progress.clamp(0.0, 1.0);
    let crop = 0.5 * t;

    act!(quad:
        align(0.5, 0.5):
        xy(0.5 * w, cy):
        zoomto(w, BAR_TARGET_H):
        diffuse(0.0, 0.0, 0.0, 1.0):
        croptop(crop): cropbottom(crop):
        z(105)   // above the hearts
    )
}

pub fn get_actors(state: &State, _m: &crate::core::space::Metrics) -> Vec<Actor> {
    let mut actors: Vec<Actor> = Vec::with_capacity(32 + ARROW_COUNT);

    // 1) HEART BACKGROUND — starts immediately
    actors.extend(state.bg.build(heart_bg::Params {
        active_color_index: state.base_color_index,
        backdrop_rgba: [0.0, 0.0, 0.0, 1.0],
        alpha_mul: 1.0,
    }));

    // 2) SQUISH BAR — wait, then unsquish over SQUISH_IN_DURATION
    let t = state.elapsed;
    let progress = if t < SQUISH_START_DELAY {
        1.0 // fully squished (line)
    } else if t < SQUISH_START_DELAY + SQUISH_IN_DURATION {
        1.0 - ((t - SQUISH_START_DELAY) / SQUISH_IN_DURATION) // 1→0
    } else {
        0.0 // fully open
    };
    actors.push(build_squish_bar(progress));

    // 3) RAINBOW ARROWS — begin after unsquish finishes
    let unsquish_end = SQUISH_START_DELAY + SQUISH_IN_DURATION;
    let cx = screen_center_x();
    let cy = screen_center_y();

    for i in 1..=ARROW_COUNT {
        let x     = (i as f32 - 4.0) * ARROW_SPACING;
        let delay = unsquish_end + ARROW_BASE_DELAY + ARROW_STEP_DELAY * (i as f32);
        let tint = color::decorative_rgba(state.base_color_index - i as i32 - 4);

        actors.push(act!(sprite("init_arrow.png"):
            align(0.5, 0.5):
            xy(cx + x, cy):
            z(110):
            zoomto(51.0, 51.0):
            diffuse(tint[0], tint[1], tint[2], 0.0):
            sleep(delay):
            linear(ARROW_FADE_IN):  alpha(1.0):
            linear(ARROW_FADE_OUT): alpha(0.0):
            linear(0.0): visible(false)
        ));
    }

    actors
}
