use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

#[derive(Default)]
pub struct InputState {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

pub fn init_state() -> InputState {
    InputState::default()
}

pub fn handle_keyboard_input(event: &KeyEvent, state: &mut InputState) {
    if let PhysicalKey::Code(key_code) = event.physical_key {
        let is_pressed = event.state == ElementState::Pressed;
        match key_code {
            KeyCode::ArrowUp | KeyCode::KeyW => state.up = is_pressed,
            KeyCode::ArrowDown | KeyCode::KeyS => state.down = is_pressed,
            KeyCode::ArrowLeft | KeyCode::KeyA => state.left = is_pressed,
            KeyCode::ArrowRight | KeyCode::KeyD => state.right = is_pressed,
            _ => {}
        }
    }
}