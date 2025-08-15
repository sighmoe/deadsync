// src/screens/options.rs
use crate::core::space::Metrics;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::{self};
use crate::ui::msdf;
use crate::ui::primitives::UIElement;
use crate::{frame, quad, text};
use std::collections::HashMap;
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

pub fn get_ui_elements(
    _state: &State,
    m: &Metrics,
    fonts: &HashMap<&'static str, msdf::Font>,
) -> Vec<UIElement> {
    let mut actors = Vec::new();

    // ——— Top bar config ———
    const BAR_H: f32 = 50.0;
    const TITLE_PX: f32 = 40.0;
    const BG: [f32; 4] = [0.15, 0.15, 0.18, 1.0];
    const FG: [f32; 4] = [0.90, 0.90, 1.00, 1.0];
    let title = "OPTIONS";

    // Full-width top bar with centered title
    actors.push(frame!(
        anchor: TopLeft,
        offset: [0, 0],
        size:   [m.right - m.left, BAR_H],
        bg_color: BG,
        children: [
            text!(
                anchor: TopCenter,
                offset: [0, BAR_H * 0.5 + TITLE_PX * 0.35],
                align:  Center,
                px:     TITLE_PX,
                color:  FG,
                text:   title
            )
        ]
    ));

    // (Optional) corner markers you had before
    actors.push(quad!(anchor: TopLeft,     offset: [ 12,  12], square: 10, color: [1.0,0.9,0.2,1.0]));
    actors.push(quad!(anchor: TopRight,    offset: [-12,  12], square: 10, color: [0.2,1.0,0.6,1.0]));
    actors.push(quad!(anchor: BottomLeft,  offset: [ 12, -12], square: 10, color: [0.6,0.6,1.0,1.0]));
    actors.push(quad!(anchor: BottomRight, offset: [-12, -12], square: 10, color: [1.0,0.6,0.2,1.0]));

    // New text message using the "miso" font
    actors.push(text!(
        anchor: BottomCenter,
        offset: [0, -100], // 100px up from the absolute bottom
        align: Center,
        px: 60.0,
        font: "miso",
        color: [0.8, 0.9, 0.7, 1.0],
        text: "This is miso font!"
    ));

    actors::build_actors(&actors, m, fonts)
}
