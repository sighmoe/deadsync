use crate::ui::actors::{self};           // build_actors(...)
use crate::{frame, quad, text};          // DSL macros
use crate::core::space::Metrics;
use crate::ui::primitives::UIElement;

use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use crate::screens::{Screen, ScreenAction};

pub struct State;
pub fn init() -> State { State }

pub fn handle_key_press(_: &mut State, e: &KeyEvent) -> ScreenAction {
    if e.state == ElementState::Pressed {
        if let PhysicalKey::Code(KeyCode::Escape) = e.physical_key {
            return ScreenAction::Navigate(Screen::Menu);
        }
    }
    ScreenAction::None
}

pub fn get_ui_elements(_state: &State, m: &Metrics) -> Vec<UIElement> {
    let mut actors = Vec::new();

    // ——— Top bar config ———
    const BAR_H: f32 = 50.0;          // similar height as before
    const TITLE_PX: f32 = 40.0;
    const TEXT_WIDTH_K: f32 = 0.45;   // crude width estimate per glyph
    const BG: [f32; 4] = [0.15, 0.15, 0.18, 1.0];
    const FG: [f32; 4] = [0.90, 0.90, 1.00, 1.0];

    let title = "OPTIONS";
    let est_w = title.len() as f32 * TITLE_PX * TEXT_WIDTH_K;

    // Full-width top bar with centered title
    actors.push(frame!(
        anchor: TopLeft,
        offset: [0, 0],
        size:   [m.right - m.left, BAR_H],   // span the whole screen width
        children: [
            quad!(fill: true, color: BG),    // bar background
            text!(
                anchor: TopCenter,
                size:   [est_w, TITLE_PX],   // lets anchor center it horizontally
                // baseline roughly in the vertical middle of the bar
                offset: [0, BAR_H * 0.5 + TITLE_PX * 0.35],
                px:     TITLE_PX,
                color:  FG,
                text:   title
            )
        ]
    ));

    // (Optional) corner markers you had before
    actors.push(quad!(anchor: TopLeft,     offset: [12,12], square: 10, color: [1.0,0.9,0.2,1.0]));
    actors.push(quad!(anchor: TopRight,    offset: [12,12], square: 10, color: [0.2,1.0,0.6,1.0]));
    actors.push(quad!(anchor: BottomLeft,  offset: [12,12], square: 10, color: [0.6,0.6,1.0,1.0]));
    actors.push(quad!(anchor: BottomRight, offset: [12,12], square: 10, color: [1.0,0.6,0.2,1.0]));

    actors::build_actors(&actors, m)
}
