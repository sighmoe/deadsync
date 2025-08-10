use crate::ui::primitives::{Quad, Sprite, UIElement};
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
    // logo + OPTION_COUNT boxes + arrow
    let mut elements = Vec::with_capacity(1 + OPTION_COUNT + 1);

    // 1) Logo sprite
    elements.push(UIElement::Sprite(Sprite {
        center: Vector2::new(0.0, 250.0),
        size: Vector2::new(600.0, 200.0),
        texture_id: "logo.png",
    }));

    // 2) Menu option quads
    let size = Vector2::new(200.0, 60.0);
    for i in 0..OPTION_COUNT {
        let y_pos = 100.0 - (i as f32 * 80.0);
        let selected = i == state.selected_index;
        let color = if selected {
            [0.2, 0.6, 0.2, 1.0]
        } else {
            [0.2, 0.2, 0.2, 1.0]
        };

        elements.push(UIElement::Quad(Quad {
            center: Vector2::new(0.0, y_pos),
            size,
            color,
        }));
    }

    // 3) Arrow sprite next to selected option
    let selected_y_pos = 100.0 - (state.selected_index as f32 * 80.0);
    elements.push(UIElement::Sprite(Sprite {
        center: Vector2::new(-150.0, selected_y_pos),
        size: Vector2::new(64.0, 64.0),
        texture_id: "meter_arrow.png",
    }));

    elements
}
