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
    if let PhysicalKey::Code(code) = event.physical_key {
        let is_pressed = event.state == ElementState::Pressed;
        let target = match code {
            KeyCode::ArrowUp    | KeyCode::KeyW => Some(&mut state.up),
            KeyCode::ArrowDown  | KeyCode::KeyS => Some(&mut state.down),
            KeyCode::ArrowLeft  | KeyCode::KeyA => Some(&mut state.left),
            KeyCode::ArrowRight | KeyCode::KeyD => Some(&mut state.right),
            _ => None,
        };
        if let Some(slot) = target {
            *slot = is_pressed;
        }
    }
}