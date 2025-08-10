use crate::ui::primitives::{Quad, UIElement};

use crate::core::input::InputState;
use crate::screens::{Screen, ScreenAction};
use cgmath::{InnerSpace, Vector2, Vector3};
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
    let distance = PLAYER_SPEED * delta_time;
    let mut move_vector = Vector3::new(0.0, 0.0, 0.0);

    if input.up {
        move_vector.y += 1.0;
    }
    if input.down {
        move_vector.y -= 1.0;
    }
    if input.left {
        move_vector.x -= 1.0;
    }
    if input.right {
        move_vector.x += 1.0;
    }

    if move_vector.x != 0.0 || move_vector.y != 0.0 {
        let normalized_move = move_vector.normalize();
        state.player_position.x += normalized_move.x * distance;
        state.player_position.y += normalized_move.y * distance;
    }
}

pub fn get_ui_elements(state: &State) -> Vec<UIElement> {
    vec![UIElement::Quad(Quad {
        center: state.player_position,
        size: Vector2::new(100.0, 100.0),
        color: [0.0, 0.0, 1.0, 1.0], // Blue
    })]
}