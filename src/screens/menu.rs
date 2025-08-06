use crate::api::{Quad, UIElement};
use crate::screens::{Screen, ScreenAction};
use cgmath::Vector2;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

const OPTION_COUNT: usize = 3;

pub struct State {
    pub selected_index: usize,
}

pub fn init() -> State {
    State { selected_index: 0 }
}

pub fn handle_key_press(state: &mut State, event: &KeyEvent) -> ScreenAction {
    if event.state != ElementState::Pressed {
        return ScreenAction::None;
    }

    match event.physical_key {
        PhysicalKey::Code(KeyCode::ArrowUp) | PhysicalKey::Code(KeyCode::KeyW) => {
            state.selected_index = if state.selected_index == 0 {
                OPTION_COUNT - 1
            } else {
                state.selected_index - 1
            };
        }
        PhysicalKey::Code(KeyCode::ArrowDown) | PhysicalKey::Code(KeyCode::KeyS) => {
            state.selected_index = (state.selected_index + 1) % OPTION_COUNT;
        }
        PhysicalKey::Code(KeyCode::Enter) => {
            return match state.selected_index {
                0 => ScreenAction::Navigate(Screen::Gameplay),
                1 => ScreenAction::Navigate(Screen::Options),
                2 => ScreenAction::Exit,
                _ => ScreenAction::None,
            };
        }
        _ => {}
    }
    ScreenAction::None
}

pub fn get_ui_elements(state: &State) -> Vec<UIElement> {
    let mut elements = Vec::new();
    let option_texts = ["Play", "Options", "Exit"];
    let size = Vector2::new(200.0, 60.0);

    for (i, &text) in option_texts.iter().enumerate() {
        let y_pos = 100.0 - (i as f32 * 80.0);
        let color = if i == state.selected_index {
            [0.2, 0.6, 0.2, 1.0] // Selected color
        } else {
            [0.2, 0.2, 0.2, 1.0] // Default color
        };

        elements.push(UIElement::Quad(Quad {
            center: Vector2::new(0.0, y_pos),
            size,
            color,
        }));

        // NOTE: Text rendering is not implemented yet.
        // We would add a `UIElement::Text` here in the future.
        let _ = text; // To avoid unused variable warning
    }
    elements
}