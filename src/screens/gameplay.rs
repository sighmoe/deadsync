use crate::core::input::InputState;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::act;
use cgmath::Vector2;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

// new: import the globals getters
use crate::core::space::globals::*;

const PLAYER_SPEED: f32 = 250.0;

pub struct State {
    pub player_position: Vector2<f32>,
    pub player_color: [f32; 4],
}

pub fn init() -> State {
    State {
        player_position: Vector2::new(0.0, 0.0),
        player_color: [0.0, 0.0, 1.0, 1.0], // default blue
    }
}

pub fn handle_key_press(_state: &mut State, event: &KeyEvent) -> ScreenAction {
    if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
        if event.state == ElementState::Pressed {
            return ScreenAction::Navigate(Screen::Menu);
        }
    }
    ScreenAction::None
}

#[inline(always)]
pub fn update(state: &mut State, input: &InputState, delta_time: f32) {
    // booleans → {-1,0,1} as f32
    let dx = (input.right as u8 as f32) - (input.left as u8 as f32);
    let dy = (input.down  as u8 as f32) - (input.up   as u8 as f32);

    if dx == 0.0 && dy == 0.0 {
        return;
    }

    // Diagonal check without squares: if both axes are non-zero, scale by 1/√2.
    const INV_SQRT2: f32 = 0.70710678118;
    let norm = if dx != 0.0 && dy != 0.0 { INV_SQRT2 } else { 1.0 };

    let step = PLAYER_SPEED * delta_time * norm;
    state.player_position.x += dx * step;
    state.player_position.y += dy * step;
}

// keep Metrics in the signature (unused), so call sites don't change
pub fn get_actors(state: &State, _: &crate::core::space::Metrics) -> Vec<Actor> {
    let cx = screen_center_x();
    let cy = screen_center_y();

    let player = act!(quad:
        align(0.5, 0.5):
        xy(cx + state.player_position.x,
           cy + state.player_position.y):
        zoomto(100.0, 100.0):
        diffuse(state.player_color[0], state.player_color[1], state.player_color[2], state.player_color[3])
    );

    vec![player]
}
