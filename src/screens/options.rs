use crate::ui::components::options_swatches;
use crate::ui::primitives::{Sprite, UIElement};
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
    let mut elements = Vec::new();

    // Draw the sprite above the colored boxes
    elements.push(UIElement::Sprite(Sprite {
        center: Vector2::new(0.0, 150.0),
        size: Vector2::new(400.0, 128.0),
        texture_id: "dance.png",
    }));

    // Add the three swatches component
    elements.extend(options_swatches());

    elements
}
