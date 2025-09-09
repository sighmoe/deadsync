// FILE: src/screens/sandbox.rs
use crate::act;
use crate::core::space::globals::*;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::anim;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/* ---------------------------- transitions ---------------------------- */
const TRANSITION_IN_DURATION: f32 = 0.4;
const TRANSITION_OUT_DURATION: f32 = 0.4;

pub struct State {
    pub elapsed: f32,
}

pub fn init() -> State {
    State { elapsed: 0.0 }
}

pub fn handle_key_press(_state: &mut State, event: &KeyEvent) -> ScreenAction {
    if event.state == ElementState::Pressed {
        if let PhysicalKey::Code(KeyCode::Escape) | PhysicalKey::Code(KeyCode::F4) = event.physical_key {
            return ScreenAction::Navigate(Screen::Menu);
        }
    }
    ScreenAction::None
}

pub fn update(state: &mut State, dt: f32) {
    state.elapsed += dt;
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

pub fn get_actors(state: &State) -> Vec<Actor> {
    let mut actors = Vec::with_capacity(10);
    let cx = screen_center_x();
    let cy = screen_center_y();

    actors.push(act!(text:
        align(0.5, 0.0): xy(cx, 20.0):
        zoomtoheight(30.0): font("wendy"): settext("Actor System Sandbox"): horizalign(center)
    ));
    actors.push(act!(text:
        align(0.5, 0.0): xy(cx, 60.0):
        zoomtoheight(15.0): font("miso"): settext("Press ESC or F4 to return to Menu"): horizalign(center)
    ));
    actors.push(act!(text:
        align(1.0, 1.0): xy(screen_width() - 10.0, screen_height() - 10.0):
        zoomtoheight(15.0): font("miso"): settext(format!("Elapsed: {:.2}", state.elapsed)): horizalign(right)
    ));

    // Test 1
    //actors.push(act!(quad:
    //    diffuse(0,1,0,1)
    //));

    // Test 2
    //actors.push(act!(quad:
    //    zoomto(100,100):
    //));

    // Test 3
    //actors.push(act!(quad:
    //    zoomto(100,100):diffuse(1,0,0,1)
    //));

    // Test S1
    //actors.push(act!(sprite("logo.png"):
    //    zoom(2.0)
    //));

    actors.push(act!(sprite("logo.png"):
        align(0.5, 0.5):
        xy(screen_center_x(), screen_center_y()):
        zoom(0.10)
    ));

    actors
}