// src/screens/menu.rs
use crate::ui::primitives::UIElement;
use crate::screens::{Screen, ScreenAction};
use crate::core::space::Metrics;
use crate::ui::components::logo::build_logo_default;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use crate::ui::actors::{self}; // build_actors(...)
use crate::{quad, sprite, text, frame}; // macros (only `text!` used here, but keeping for symmetry)
use crate::ui::msdf;
use std::sync::OnceLock;

const OPTION_COUNT: usize = 3;
const MENU_OPTIONS: [&str; OPTION_COUNT] = ["GAMEPLAY", "OPTIONS", "EXIT"];

const MENU_SELECTED_COLOR: [f32; 4] = [1.0, 1.0, 0.5, 1.0];
const MENU_NORMAL_COLOR:   [f32; 4] = [0.8, 0.8, 0.8, 1.0];

// slightly smaller sizes + tighter spacing
const MENU_SELECTED_PX:   f32 = 44.0;
const MENU_NORMAL_PX:     f32 = 36.0;
const MENU_ROW_SPACING:   f32 = 40.0;
// distance up from the bottom edge (in SM px)
const MENU_BOTTOM_MARGIN: f32 = 20.0;

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
            state.selected_index =
                if state.selected_index == 0 { OPTION_COUNT - 1 } else { state.selected_index - 1 };
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
        PhysicalKey::Code(KeyCode::Escape) => {
            return ScreenAction::Exit;
        }
        _ => {}
    }
    ScreenAction::None
}

// ---- precise text width (so horizontal centering is exact) ----
fn measure_text_px(content: &str, px: f32) -> f32 {
    static WENDY: OnceLock<msdf::Font> = OnceLock::new();
    let font = WENDY.get_or_init(|| {
        // Load once for measuring; the rendering path uses the font loaded in App.
        let json = std::fs::read("assets/fonts/wendy.json")
            .expect("assets/fonts/wendy.json");
        msdf::load_font(&json, "wendy.png", 4.0)
    });

    if px <= 0.0 || font.line_h == 0.0 || content.is_empty() {
        return 0.0;
    }
    let scale = px / font.line_h;
    content.chars().map(|ch| {
        if let Some(g) = font.glyphs.get(&ch) { g.xadv } else { font.space_advance }
    }).sum::<f32>() * scale
}

pub fn get_ui_elements(state: &State, m: &Metrics) -> Vec<UIElement> {
    // Keep the logo at the top as before.
    let logo = build_logo_default(m);

    // Bottom-centered column: first item highest, last item closest to bottom.
    // For Bottom* anchors, offset.y is measured UP from the bottom edge.
    let mut actors = Vec::with_capacity(OPTION_COUNT);
    let start = MENU_BOTTOM_MARGIN + (OPTION_COUNT as f32 - 1.0) * MENU_ROW_SPACING;

    for i in 0..OPTION_COUNT {
        let is_selected = i == state.selected_index;
        let px    = if is_selected { MENU_SELECTED_PX } else { MENU_NORMAL_PX };
        let color = if is_selected { MENU_SELECTED_COLOR } else { MENU_NORMAL_COLOR };
        let label = MENU_OPTIONS[i];

        // exact width in screen pixels based on MSDF metrics
        let exact_w = measure_text_px(label, px);

        // Stack upward from the bottom margin.
        let y_off = start - (i as f32) * MENU_ROW_SPACING;

        actors.push(text!(
            anchor: BottomCenter,   // anchor list to the bottom center
            size:   [exact_w, px],  // accurate width => perfect centering
            offset: [0, y_off],     // y is "distance up from bottom"
            px:     px,
            color:  color,
            text:   label
        ));
    }

    let mut elements = logo.ui;
    elements.extend(crate::ui::actors::build_actors(&actors, m));
    elements
}
