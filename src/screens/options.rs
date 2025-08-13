// src/screens/options.rs
use crate::ui::actors::{self};          // build_actors(...)
use crate::{quad, frame, text};          // macros you actually use (sprite not needed)
use crate::ui::primitives::UIElement;
use crate::core::space::Metrics;
use crate::screens::{Screen, ScreenAction};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

pub struct State;

pub fn init() -> State { State }

pub fn handle_key_press(_state: &mut State, event: &KeyEvent) -> ScreenAction {
    if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
        if event.state == ElementState::Pressed {
            return ScreenAction::Navigate(Screen::Menu);
        }
    }
    ScreenAction::None
}

pub fn get_ui_elements(_state: &State, m: &Metrics) -> Vec<UIElement> {
    let scene = vec![
        frame!(anchor: BottomRight, offset: [12,12], size: [420,80],
            children: [
                quad!(fill: true, color: [0.15,0.15,0.18,1.0]),
                text!(anchor: Center, px: 28, color: [0.9,0.9,1.0,1.0], text: "OPTIONS"),
            ]
        ),
        quad!(anchor: TopLeft,     offset: [12,12], square: 10, color: [1.0,0.9,0.2,1.0]),
        quad!(anchor: TopRight,    offset: [12,12], square: 10, color: [0.2,1.0,0.6,1.0]),
        quad!(anchor: BottomLeft,  offset: [12,12], square: 10, color: [0.6,0.6,1.0,1.0]),
        quad!(anchor: BottomRight, offset: [12,12], square: 10, color: [1.0,0.6,0.2,1.0]),
    ];
    actors::build_actors(&scene, m)
}
