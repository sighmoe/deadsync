use crate::core::input::InputState;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::{Actor, Anchor, SizeSpec};
use crate::quad;
use cgmath::{Vector2, InnerSpace};
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
    // Compute axis as {-1, 0, 1} from booleans.
    let dx = (input.right as u8 as f32) - (input.left as u8 as f32);
    let dy = (input.down as u8 as f32) - (input.up as u8 as f32);

    let move_vec = Vector2::new(dx, dy);

    // Early-out if idle.
    if move_vec.x == 0.0 && move_vec.y == 0.0 {
        return;
    }

    // Normalize to get a direction vector and scale by speed and delta time.
    let displacement = move_vec.normalize() * PLAYER_SPEED * delta_time;
    state.player_position += displacement;
}

pub fn get_actors(state: &State) -> Vec<Actor> {
    // Player as a solid-color quad (now a Sprite with Solid source)
    let player = quad! {
        anchor: Anchor::Center,
        offset: [state.player_position.x, state.player_position.y],
        size:   [SizeSpec::Px(100.0), SizeSpec::Px(100.0)],
        color:  [0.0, 0.0, 1.0, 1.0], // Blue
    };

    vec![player]
}
