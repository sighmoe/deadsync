use crate::ui::primitives::{Quad, UIElement};
use crate::core::space::Metrics;
use crate::core::input::InputState;
use crate::screens::{Screen, ScreenAction};
use cgmath::{Vector2};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

const PLAYER_SPEED: f32 = 250.0;

pub struct State {
    pub player_position: Vector2<f32>,
}

pub fn init() -> State {
    State {
        player_position: Vector2::new(0.0, 0.0),
    }
}

pub fn handle_key_press(_state: &mut State, event: &KeyEvent) -> ScreenAction {
    if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
        if event.state == ElementState::Pressed {
            return ScreenAction::Navigate(Screen::Menu);
        }
    }
    ScreenAction::None
}

pub fn update(state: &mut State, input: &InputState, delta_time: f32) {
    // Compute axis as {-1,0,1} without branches
    let dx = (input.right as i32 - input.left as i32) as f32;
    let dy = (input.up as i32 - input.down as i32) as f32;

    // Early-out if idle
    if dx == 0.0 && dy == 0.0 {
        return;
    }

    // Normalize and scale by speed * dt
    let len_sq = dx * dx + dy * dy;
    // `len_sq` can only be 1 or 2 here, but keep it general
    let inv_len = 1.0 / len_sq.sqrt();
    let step = PLAYER_SPEED * delta_time;

    state.player_position.x += dx * inv_len * step;
    state.player_position.y += dy * inv_len * step;
}


pub fn get_ui_elements(state: &State, _m: &Metrics) -> Vec<UIElement> {
    vec![UIElement::Quad(Quad {
        center: state.player_position,
        size: Vector2::new(100.0, 100.0),
        color: [0.0, 0.0, 1.0, 1.0], // Blue
    })]
}