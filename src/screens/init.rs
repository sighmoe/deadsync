// src/screens/init.rs
use crate::act;
use crate::core::space::globals::*;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

// timings mirror the non-"Thonk" SM5 snippet
const ARROW_COUNT: usize = 7;
const ARROW_SPACING: f32 = 50.0;
const ARROW_BASE_DELAY: f32 = 0.2;
const ARROW_STEP_DELAY: f32 = 0.1;
const ARROW_FADE_IN: f32 = 0.75;
const ARROW_FADE_OUT: f32 = 0.75;

// background bar timing
const BAR_ACCEL: f32 = 0.3;
const BAR_TARGET_H: f32 = 128.0;
const BAR_ALPHA: f32 = 0.9;
const BAR_SLEEP: f32 = 2.1;

// when to auto-move to Menu (last arrow completes around 0.9 + 1.5 = 2.4s)
const AUTO_TO_MENU_AT: f32 = 2.5;

pub struct State {
    elapsed: f32,
    active_color_index: i32,
}

pub fn init() -> State {
    State { elapsed: 0.0, active_color_index: 0 }
}

pub fn handle_key_press(_: &mut State, e: &KeyEvent) -> ScreenAction {
    if e.state == ElementState::Pressed {
        match e.physical_key {
            PhysicalKey::Code(KeyCode::Enter)
            | PhysicalKey::Code(KeyCode::Space)
            | PhysicalKey::Code(KeyCode::Escape) => {
                return ScreenAction::Navigate(Screen::Menu);
            }
            _ => {}
        }
    }
    ScreenAction::None
}

pub fn update(state: &mut State, delta: f32) -> ScreenAction {
    state.elapsed += delta;
    if state.elapsed >= AUTO_TO_MENU_AT {
        ScreenAction::Navigate(Screen::Menu)
    } else {
        ScreenAction::None
    }
}

pub fn get_actors(state: &State, _m: &crate::core::space::Metrics) -> Vec<Actor> {
    let mut actors: Vec<Actor> = Vec::with_capacity(16 + ARROW_COUNT);

    // --- background bar (semi-transparent black quad growing to 128 px) ---
    let w = screen_width();
    let cy = screen_center_y();

    actors.push(act!(quad:
        align(0.0, 0.0):
        xy(0.0, cy - BAR_TARGET_H * 0.5):     // top-left position so it grows centered on Y
        zoomto(w, 0.0):
        diffuse(0.0, 0.0, 0.0, 0.0):
        // OnCommand: accelerate(0.3):zoomtoheight(128):diffusealpha(0.9):sleep(2.1)
        accelerate(BAR_ACCEL): zoomtoheight(BAR_TARGET_H): alpha(BAR_ALPHA):
        sleep(BAR_SLEEP)
    ));

    // --- 7 “SM5 logo” arrows (we just reuse logo.png, tinted like GetHexColor(..., true)) ---
    let cx = screen_center_x();
    for i in 1..=ARROW_COUNT {
        let x = (i as f32 - 4.0) * ARROW_SPACING;
        let delay = ARROW_BASE_DELAY + ARROW_STEP_DELAY * (i as f32);

        let tint = palette_color(state.active_color_index - i as i32 - 4, true);

        actors.push(act!(sprite("init_arrow.png"):
            align(0.5, 0.5):
            xy(cx + x, cy):
            z(10):                         // sit above the black bar for sure
            zoomto(51.0, 51.0):            // <-- set BOTH w & h; square is fine
            diffuse(tint[0], tint[1], tint[2], 0.0):
            sleep(delay):
            linear(ARROW_FADE_IN): alpha(1.0):
            linear(ARROW_FADE_OUT): alpha(0.0):
            linear(0.0): visible(false)
        ));
    }

    actors
}

// simple palette helper for desaturated color like GetHexColor(..., true)
fn palette_color(idx: i32, desat: bool) -> [f32; 4] {
    const HEX: [&str; 7] = [
        "#FFFF00","#AEFA44","#5CE087", "#00ADC0", "#0073FF", "#413AD0", "#8200A1"
    ];
    let c = crate::ui::color::rgba_hex(HEX[idx.rem_euclid(HEX.len() as i32) as usize]);
    c
}
