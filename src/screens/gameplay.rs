use crate::core::input::InputState;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::{Actor};
use crate::act;
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
    // Convert booleans to {-1.0, 0.0, 1.0} without branches.
    let dx = (input.right as u8 as f32) - (input.left as u8 as f32);
    let dy = (input.down  as u8 as f32) - (input.up   as u8 as f32);

    // Early-out if idle.
    if dx == 0.0 && dy == 0.0 {
        return;
    }

    // For axis-aligned/diagonal movement, speed normalization is trivial:
    // length is 1.0 for cardinal, sqrt(2) for diagonal.
    let len_sq = dx * dx + dy * dy;              // ∈ {1.0, 2.0}
    let norm = if len_sq == 2.0 { 0.70710678 } else { 1.0 }; // 1/√2 for diagonals

    let step = PLAYER_SPEED * delta_time * norm;
    state.player_position.x += dx * step;
    state.player_position.y += dy * step;
}

pub fn get_actors(state: &State) -> Vec<Actor> {
    let player = act!(quad:
        align(0.5, 0.5):
        xy(state.player_position.x, state.player_position.y):
        zoomto(100.0, 100.0):
        diffuse(0.0, 0.0, 1.0, 1.0)
    );

    vec![player]
}
