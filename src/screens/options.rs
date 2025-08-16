// src/screens/options.rs
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::{Actor, Anchor, SizeSpec, TextAlign};
use crate::ui::components;
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
    actors.push(Actor::Quad { anchor: Anchor::TopLeft,     offset: [ 12.0,  12.0], size: [SizeSpec::Px(10.0), SizeSpec::Px(10.0)], color: [1.0,0.9,0.2,1.0]});
    actors.push(Actor::Quad { anchor: Anchor::TopRight,    offset: [-12.0,  12.0], size: [SizeSpec::Px(10.0), SizeSpec::Px(10.0)], color: [0.2,1.0,0.6,1.0]});
    actors.push(Actor::Quad { anchor: Anchor::BottomLeft,  offset: [ 12.0, -12.0], size: [SizeSpec::Px(10.0), SizeSpec::Px(10.0)], color: [0.6,0.6,1.0,1.0]});
    actors.push(Actor::Quad { anchor: Anchor::BottomRight, offset: [-12.0, -12.0], size: [SizeSpec::Px(10.0), SizeSpec::Px(10.0)], color: [1.0,0.6,0.2,1.0]});

    // New text message using the "miso" font
    actors.push(Actor::Text {
        anchor: Anchor::BottomCenter,
        offset: [0.0, -100.0],
        align: TextAlign::Center,
        px: 60.0,
        font: "miso",
        color: [0.8, 0.9, 0.7, 1.0],
        content: "This is miso font!".to_string(),
    });

    actors
}
