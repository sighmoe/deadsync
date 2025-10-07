use crate::act;
use crate::core::space::*;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::color;
use crate::ui::components::{heart_bg, pad_display, screen_bar};
use crate::ui::components::screen_bar::{ScreenBarParams, ScreenBarPosition, ScreenBarTitlePlacement};
use crate::core::space::widescale;

use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/* ---------------------------- transitions ---------------------------- */
const TRANSITION_IN_DURATION: f32 = 0.4;
const TRANSITION_OUT_DURATION: f32 = 0.4;


pub struct State {
    pub active_color_index: i32,
    bg: heart_bg::State,
    pub session_elapsed: f32, // To display the timer
}

pub fn init() -> State {
    State {
        active_color_index: color::DEFAULT_COLOR_INDEX,
        bg: heart_bg::State::new(),
        session_elapsed: 0.0,
    }
}

pub fn handle_key_press(_state: &mut State, event: &KeyEvent) -> ScreenAction {
    if event.state == ElementState::Pressed {
        if let PhysicalKey::Code(KeyCode::Escape) | PhysicalKey::Code(KeyCode::Enter) = event.physical_key {
            return ScreenAction::Navigate(Screen::SelectMusic);
        }
    }
    ScreenAction::None
}

// This screen doesn't have any dynamic state updates yet, but we keep the function for consistency.
pub fn update(_state: &mut State, _dt: f32) {
    //
}

pub fn in_transition() -> (Vec<Actor>, f32) {
    let actor = act!(quad:
        align(0.0, 0.0): xy(0.0, 0.0):
        zoomto(screen_width(), screen_height()):
        diffuse(0.0, 0.0, 0.0, 1.0): z(1100):
        linear(TRANSITION_IN_DURATION): alpha(0.0):
        linear(0.0): visible(false)
    );
    (vec![actor], TRANSITION_IN_DURATION)
}

pub fn out_transition() -> (Vec<Actor>, f32) {
    let actor = act!(quad:
        align(0.0, 0.0): xy(0.0, 0.0):
        zoomto(screen_width(), screen_height()):
        diffuse(0.0, 0.0, 0.0, 0.0):
        z(1200):
        linear(TRANSITION_OUT_DURATION): alpha(1.0)
    );
    (vec![actor], TRANSITION_OUT_DURATION)
}

fn format_session_time(seconds_total: f32) -> String {
    if seconds_total < 0.0 {
        return "0:00".to_string();
    }
    let seconds_total = seconds_total as u64;

    let hours = seconds_total / 3600;
    let minutes = (seconds_total % 3600) / 60;
    let seconds = seconds_total % 60;

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{}:{:02}", minutes, seconds)
    }
}

pub fn get_actors(state: &State) -> Vec<Actor> {
    let mut actors = Vec::with_capacity(20);

    // 1. Background
    actors.extend(state.bg.build(heart_bg::Params {
        active_color_index: state.active_color_index,
        backdrop_rgba: [0.0, 0.0, 0.0, 1.0],
        alpha_mul: 1.0,
    }));

    // 2. Top Bar (like select_music)
    actors.push(screen_bar::build(ScreenBarParams {
        title: "EVALUATION",
        title_placement: ScreenBarTitlePlacement::Left,
        position: ScreenBarPosition::Top,
        transparent: false,
        fg_color: [1.0; 4],
        left_text: None, center_text: None, right_text: None,
    }));

    // Session Timer, centered in the top bar.
    let timer_text = format_session_time(state.session_elapsed);
    actors.push(act!(text:
        font("wendy_monospace_numbers"):
        settext(timer_text):
        align(0.5, 0.5):
        xy(screen_center_x(), 10.0):
        zoom(widescale(0.3, 0.36)):
        z(121):
        diffuse(1.0, 1.0, 1.0, 1.0):
        horizalign(center)
    ));
    
    // "ITG" text and Pads (top right), matching Simply Love layout
    {
        let itg_text_x = screen_width() - widescale(55.0, 62.0);
        actors.push(act!(text:
            font("wendy"):
            settext("ITG"):
            align(1.0, 0.5):
            xy(itg_text_x, 15.0):
            zoom(widescale(0.5, 0.6)):
            z(121):
            diffuse(1.0, 1.0, 1.0, 1.0)
        ));

        let final_pad_zoom = 0.24 * widescale(0.435, 0.525);

        actors.push(pad_display::build(pad_display::PadDisplayParams {
            center_x: screen_width() - widescale(35.0, 41.0),
            center_y: widescale(22.0, 23.5),
            zoom: final_pad_zoom,
            z: 121,
            is_active: true, // Assuming P1 is always active here
        }));
        actors.push(pad_display::build(pad_display::PadDisplayParams {
            center_x: screen_width() - widescale(15.0, 17.0),
            center_y: widescale(22.0, 23.5),
            zoom: final_pad_zoom,
            z: 121,
            is_active: false, // P2 inactive
        }));
    }

    // 3. Bottom Bar (like gameplay)
    actors.push(screen_bar::build(ScreenBarParams {
        title: "",
        title_placement: screen_bar::ScreenBarTitlePlacement::Center,
        position: screen_bar::ScreenBarPosition::Bottom,
        transparent: true,
        fg_color: [1.0; 4],
        left_text: Some("PerfectTaste"), center_text: None, right_text: None,
    }));

    // 4. Placeholder content
    actors.push(act!(text:
        font("wendy"): settext("SCORE SCREEN SHOULD BE HERE"):
        align(0.5, 0.5): xy(screen_center_x(), screen_center_y()):
        zoom(0.8): horizalign(center):
        z(100)
    ));

    actors
}