use crate::input::InputState;
use cgmath::Vector3;

const PLAYER_SPEED: f32 = 250.0; // Pixels per second

pub struct GameState {
    pub square_position: Vector3<f32>,
}

pub fn init_state() -> GameState {
    GameState {
        square_position: Vector3::new(0.0, 0.0, 0.0),
    }
}

pub fn update_state(game_state: &mut GameState, input: &InputState, delta_time: f32) {
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

    // Normalize the move vector to prevent faster diagonal movement
    if move_vector.x != 0.0 || move_vector.y != 0.0 {
        let normalized_move = cgmath::InnerSpace::normalize(move_vector);
        game_state.square_position += normalized_move * distance;
    }
}