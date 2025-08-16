// src/screens/options.rs
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::components;
use crate::{quad, text};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

pub struct State;
pub fn init() -> State {
    State
}

pub fn handle_key_press(_: &mut State, e: &KeyEvent) -> ScreenAction {
    if e.state == ElementState::Pressed {
        if let PhysicalKey::Code(KeyCode::Escape) = e.physical_key {
            return ScreenAction::Navigate(Screen::Menu);
        }
    }
    ScreenAction::None
}

pub fn get_actors(_state: &State) -> Vec<Actor> {
    let mut actors = Vec::new();

    // Use the reusable top_bar component.
    actors.push(components::top_bar::build("OPTIONS"));

    // (Optional) corner markers
    actors.push(quad!(anchor: TopLeft,     offset: [ 12,  12], size: [10, 10], color: [1.0,0.9,0.2,1.0]));
    actors.push(quad!(anchor: TopRight,    offset: [-12,  12], size: [10, 10], color: [0.2,1.0,0.6,1.0]));
    actors.push(quad!(anchor: BottomLeft,  offset: [ 12, -12], size: [10, 10], color: [0.6,0.6,1.0,1.0]));
    actors.push(quad!(anchor: BottomRight, offset: [-12, -12], size: [10, 10], color: [1.0,0.6,0.2,1.0]));

    // New text message using the "miso" font
    actors.push(text!(
        anchor: BottomCenter,
        offset: [0, -100],
        align: Center,
        px: 60.0,
        font: "miso",
        color: [0.8, 0.9, 0.7, 1.0],
        text: "This is miso font!"
    ));

    actors
}