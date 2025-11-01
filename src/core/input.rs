use std::time::Instant;

use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Lane {
    Left = 0,
    Down = 1,
    Up = 2,
    Right = 3,
}

impl Lane {
    #[inline(always)]
    pub const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputSource {
    Keyboard,
    Gamepad,
}

#[derive(Clone, Copy, Debug)]
pub struct InputEdge {
    pub lane: Lane,
    pub pressed: bool,
    pub source: InputSource,
    pub timestamp: Instant,
}

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
            KeyCode::ArrowUp | KeyCode::KeyW => Some(&mut state.up),
            KeyCode::ArrowDown | KeyCode::KeyS => Some(&mut state.down),
            KeyCode::ArrowLeft | KeyCode::KeyA => Some(&mut state.left),
            KeyCode::ArrowRight | KeyCode::KeyD => Some(&mut state.right),
            _ => None,
        };
        if let Some(slot) = target {
            *slot = is_pressed;
        }
    }
}

#[inline(always)]
pub fn lane_from_keycode(code: KeyCode) -> Option<Lane> {
    match code {
        KeyCode::ArrowLeft | KeyCode::KeyD => Some(Lane::Left),
        KeyCode::ArrowDown | KeyCode::KeyF => Some(Lane::Down),
        KeyCode::ArrowUp | KeyCode::KeyJ => Some(Lane::Up),
        KeyCode::ArrowRight | KeyCode::KeyK => Some(Lane::Right),
        _ => None,
    }
}
