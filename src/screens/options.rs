use crate::api::{Quad, Sprite, UIElement};
use crate::screens::{Screen, ScreenAction};
use cgmath::Vector2;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

pub struct State;

pub fn init() -> State {
    State
}

pub fn handle_key_press(_state: &mut State, event: &KeyEvent) -> ScreenAction {
    if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
        if event.state == ElementState::Pressed {
            return ScreenAction::Navigate(Screen::Menu);
        }
    }
    ScreenAction::None
}

pub fn get_ui_elements(_state: &State) -> Vec<UIElement> {
    vec![
        // Draw the sprite above the colored boxes
        UIElement::Sprite(Sprite {
            center: Vector2::new(0.0, 150.0),
            size: Vector2::new(400.0, 128.0),
            texture_id: "dance.png",
        }),
        // The original colored quads
        UIElement::Quad(Quad {
            center: Vector2::new(-150.0, 0.0),
            size: Vector2::new(100.0, 100.0),
            color: [1.0, 0.0, 0.0, 1.0], // Red
        }),
        UIElement::Quad(Quad {
            center: Vector2::new(0.0, 0.0),
            size: Vector2::new(100.0, 100.0),
            color: [0.0, 1.0, 0.0, 1.0], // Green
        }),
        UIElement::Quad(Quad {
            center: Vector2::new(150.0, 0.0),
            size: Vector2::new(100.0, 100.0),
            color: [0.0, 0.0, 1.0, 1.0], // Blue
        }),
    ]
}