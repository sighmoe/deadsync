use crate::act;
use crate::core::space::globals::*;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::color; // add
use crate::ui::components::{heart_bg, screen_bar};

use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

// corner test params
const CORNER_SIZE: f32      = 10.0;  // px

pub struct State {
    // shared animated background (uses heart.png)
    pub bg: heart_bg::State,
}

pub fn init() -> State {
    State {
        bg: heart_bg::State::new(),
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
    let backdrop = [0.0, 0.0, 0.0, 1.0];
    actors.extend(state.bg.build(heart_bg::Params {
        active_color_index: color::DEFAULT_COLOR_INDEX,
        backdrop_rgba: backdrop,
        alpha_mul: 1.0,
    }));

    const FG_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

    // --- top bar title ---
    actors.push(screen_bar::build(screen_bar::ScreenBarParams {
        title: "OPTIONS",
        title_placement: screen_bar::ScreenBarTitlePlacement::Center,
        position: screen_bar::ScreenBarPosition::Top,
        transparent: false,
        left_text: None,
        center_text: None,
        right_text: None,
        fg_color: FG_COLOR,
    }));

    // --- footer bar (demonstration) ---
    actors.push(screen_bar::build(screen_bar::ScreenBarParams {
        title: "FOOTER",
        title_placement: screen_bar::ScreenBarTitlePlacement::Center,
        position: screen_bar::ScreenBarPosition::Bottom,
        transparent: false,
        left_text: None,
        center_text: None,
        right_text: None,
        fg_color: FG_COLOR,
    }));

    // --- corner spinning quads (testing) ---
    let w = screen_width();
    let h = screen_height();

    // (h_align, v_align) in {0,1}, small offsets, and distinct colors
    let corners = [
        ((0.0_f32, 0.0_f32), [ 12.0,  62.0], [1.0, 0.90, 0.20, 1.0]), // top-left
        ((1.0_f32, 0.0_f32), [-12.0,  62.0], [0.20, 1.0, 0.60, 1.0]), // top-right
        ((0.0_f32, 1.0_f32), [ 12.0, -62.0], [0.60, 0.60, 1.0, 1.0]), // bottom-left
        ((1.0_f32, 1.0_f32), [-12.0, -62.0], [1.0, 0.60, 0.20, 1.0]), // bottom-right
    ];

    for (_i, ((hx, vy), off, col)) in corners.into_iter().enumerate() {
        let x = hx * w + off[0];
        let y = vy * h + off[1];
        actors.push(act!(quad:
            align(hx, vy):
            xy(x, y):
            zoomto(CORNER_SIZE, CORNER_SIZE):
            diffuse(col[0], col[1], col[2], col[3]):
            z(50) // draw above bg
        ));
    }

    // --- small bottom hint ---
    actors.push(act!(text:
        align(0.5, 1.0):
        xy(0.5 * w, h - 60.0):
        zoomtoheight(20.0):
        font("miso"):
        diffuse(0.85, 0.90, 0.75, 0.9):
        settext("Press Esc to return"):
        horizalign(center)
    ));

    actors
}
