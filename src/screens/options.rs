// src/screens/options.rs
use crate::act;
use crate::core::space::globals::*;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::components::{heart_bg, screen_bar};

use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use std::time::Instant;

// corner test params
const CORNER_ROT_SPEED: f32 = 0.9;   // radians/sec
const CORNER_ROT_AMPL: f32  = 30.0;  // degrees
const CORNER_SIZE: f32      = 10.0;  // px

pub struct State {
    // shared animated background (uses heart.png)
    pub bg: heart_bg::State,
    pub active_color_index: i32,
    pub rainbow_mode: bool,

    // local timer for corner wobble + demo text
    t0: Instant,
}

pub fn init() -> State {
    State {
        bg: heart_bg::State::new(),
        active_color_index: 0,
        rainbow_mode: false,
        t0: Instant::now(),
    }
}

pub fn handle_key_press(_: &mut State, e: &KeyEvent) -> ScreenAction {
    if e.state == ElementState::Pressed {
        if let PhysicalKey::Code(KeyCode::Escape) = e.physical_key {
            return ScreenAction::Navigate(Screen::Menu);
        }
    }
    ScreenAction::None
}

pub fn get_actors(state: &State, _: &crate::core::space::Metrics) -> Vec<Actor> {
    let mut actors: Vec<Actor> = Vec::with_capacity(64);

    // --- animated background (shared with menu) ---
    let backdrop = if state.rainbow_mode {
        [1.0, 1.0, 1.0, 1.0]
    } else {
        [0.0, 0.0, 0.0, 1.0]
    };
    actors.extend(state.bg.build(heart_bg::Params {
        active_color_index: state.active_color_index,
        backdrop_rgba: backdrop,
    }));

    // --- top bar title ---
    actors.push(screen_bar::build(screen_bar::ScreenBarParams {
        title: "OPTIONS",
        position: screen_bar::ScreenBarPosition::Top,
        transparent: false,
    }));

    // --- footer bar (demonstration) ---
    actors.push(screen_bar::build(screen_bar::ScreenBarParams {
        title: "FOOTER",
        position: screen_bar::ScreenBarPosition::Bottom,
        transparent: false,
    }));

    // --- corner spinning quads (testing) ---
    let w = screen_width();
    let h = screen_height();
    let t = state.t0.elapsed().as_secs_f32();

    // (h_align, v_align) in {0,1}, small offsets, and distinct colors
    let corners = [
        ((0.0_f32, 0.0_f32), [ 12.0,  62.0], [1.0, 0.90, 0.20, 1.0]), // top-left
        ((1.0_f32, 0.0_f32), [-12.0,  62.0], [0.20, 1.0, 0.60, 1.0]), // top-right
        ((0.0_f32, 1.0_f32), [ 12.0, -62.0], [0.60, 0.60, 1.0, 1.0]), // bottom-left
        ((1.0_f32, 1.0_f32), [-12.0, -62.0], [1.0, 0.60, 0.20, 1.0]), // bottom-right
    ];

    for (i, ((hx, vy), off, col)) in corners.into_iter().enumerate() {
        let x = hx * w + off[0];
        let y = vy * h + off[1];
        let rot = CORNER_ROT_AMPL * ((t * CORNER_ROT_SPEED) + i as f32 * 0.7).sin();
        actors.push(act!(quad:
            align(hx, vy):
            xy(x, y):
            zoomto(CORNER_SIZE, CORNER_SIZE):
            diffuse(col[0], col[1], col[2], col[3]):
            rotationz(rot):
            z(50) // draw above bg
        ));
    }

    // --- big miso sample text ---
    actors.push(act!(text:
        align(0.5, 1.0):                // bottom-center baseline
        xy(0.5 * w, h - 100.0):
        px(60.0):
        font("miso"):
        diffuse(0.80, 0.90, 0.70, 1.0):
        text("This is miso font!"):
        talign(center)
    ));

    // --- small bottom hint ---
    actors.push(act!(text:
        align(0.5, 1.0):
        xy(0.5 * w, h - 60.0):
        px(20.0):
        font("miso"):
        diffuse(0.85, 0.90, 0.75, 0.9):
        text("Press Esc to return"):
        talign(center)
    ));

    actors
}
