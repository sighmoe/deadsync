use crate::ui::components::options_swatches;
use crate::ui::primitives::{Sprite, UIElement};
use crate::screens::{Screen, ScreenAction};
use crate::core::space::Metrics;
use crate::ui::build::{from_right, from_top, sm_rect_to_center_size};
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

pub fn get_ui_elements(_state: &State, m: &Metrics) -> Vec<UIElement> {
    let mut elements = Vec::new();

    // Fallback banner: 20px from the right (and 20px from the top), SM top-left coords
    let bw = 256.0;
    let bh = 64.0;
    let x_tl = from_right(bw + 20.0, m);
    let y_tl = from_top(20.0, m);
    let (center, size) = sm_rect_to_center_size(x_tl, y_tl, bw, bh, m);

    elements.push(UIElement::Sprite(Sprite {
        center: Vector2::new(center[0], center[1]),
        size:   Vector2::new(size[0],   size[1]),
        texture_id: "fallback_banner.png",
    }));

    // The three colored swatches
    elements.extend(options_swatches());

    elements
}