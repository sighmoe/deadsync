// FILE: src/screens/gameplay.rs
use crate::core::input::InputState;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::act;
use crate::core::space::globals::*;
use cgmath::Vector2;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use crate::core::noteskin::{self, Noteskin, Style, Quantization, NUM_QUANTIZATIONS};
use std::path::Path;

/* ---------------------------- transitions ---------------------------- */
const TRANSITION_IN_DURATION: f32 = 0.4;
const TRANSITION_OUT_DURATION: f32 = 0.4;

const PLAYER_SPEED: f32 = 250.0;

pub struct State {
    pub player_position: Vector2<f32>,
    pub player_color: [f32; 4],
    pub noteskin: Option<Noteskin>,
}

pub fn init() -> State {
    let style = Style { num_cols: 4, num_players: 1 };
    let mut noteskin = noteskin::load(Path::new("assets/noteskins/bar/dance-single.txt"), &style).ok()
        .or_else(|| noteskin::load(Path::new("assets/noteskins/bar/all-styles.txt"), &style).ok());

    if let Some(ns) = &mut noteskin {
        let base_path = Path::new("assets");
        ns.tex_notes_dims = image::image_dimensions(base_path.join(&ns.tex_notes_path)).unwrap_or((256, 256));
        ns.tex_receptors_dims = image::image_dimensions(base_path.join(&ns.tex_receptors_path)).unwrap_or((128, 64));
        ns.tex_glow_dims = image::image_dimensions(base_path.join(&ns.tex_glow_path)).unwrap_or((96, 96));
    }

    State {
        player_position: Vector2::new(0.0, 0.0),
        player_color: [0.0, 0.0, 1.0, 1.0],
        noteskin,
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
    let dx = (input.right as u8 as f32) - (input.left as u8 as f32);
    let dy = (input.down  as u8 as f32) - (input.up   as u8 as f32);
    if dx == 0.0 && dy == 0.0 { return; }
    const INV_SQRT2: f32 = 0.70710678118;
    let norm = if dx != 0.0 && dy != 0.0 { INV_SQRT2 } else { 1.0 };
    let step = PLAYER_SPEED * delta_time * norm;
    state.player_position.x += dx * step;
    state.player_position.y += dy * step;
}

pub fn in_transition() -> (Vec<Actor>, f32) {
    let actor = act!(quad:
        align(0.0, 0.0): xy(0.0, 0.0):
        zoomto(screen_width(), screen_height()):
        diffuse(0.0, 0.0, 0.0, 1.0): z(1100):
        linear(TRANSITION_IN_DURATION): alpha(0.0):
        linear(0.0): visible(false)
    );
    (vec![actor], TRANSITION_IN_DURATION)
}

pub fn out_transition() -> (Vec<Actor>, f32) {
    let actor = act!(quad:
        align(0.0, 0.0): xy(0.0, 0.0):
        zoomto(screen_width(), screen_height()):
        diffuse(0.0, 0.0, 0.0, 0.0): z(1200):
        linear(TRANSITION_OUT_DURATION): alpha(1.0)
    );
    (vec![actor], TRANSITION_OUT_DURATION)
}

pub fn get_actors(state: &State) -> Vec<Actor> {
    let mut actors = Vec::new();
    let cx = screen_center_x();
    let cy = screen_center_y();

    if let Some(ns) = &state.noteskin {
        // 1. Draw Receptors
        for (i, col_x_offset) in ns.column_xs.iter().enumerate() {
            let receptor_def = &ns.receptor_off[i];
            let uv = noteskin::get_uv_rect(receptor_def, ns.tex_receptors_dims);
            
            actors.push(act!(sprite(ns.tex_receptors_path.clone()):
                align(0.5, 0.5):
                xy(cx + *col_x_offset as f32, cy):
                zoomto(receptor_def.size[0] as f32, receptor_def.size[1] as f32):
                customtexturerect(uv[0], uv[1], uv[2], uv[3])
            ));
        }

        // 2. Draw a few test notes
        let test_notes = [
            (Quantization::Q4th, -100.0), (Quantization::Q8th, -150.0),
            (Quantization::Q16th, -200.0), (Quantization::Q24th, -250.0),
        ];

        for (quantization, y_offset) in test_notes {
             for (i, col_x_offset) in ns.column_xs.iter().enumerate() {
                let note_idx = i * NUM_QUANTIZATIONS + quantization as usize;
                if let Some(note_def) = ns.notes.get(note_idx) {
                    let uv = noteskin::get_uv_rect(note_def, ns.tex_notes_dims);
                    
                    actors.push(act!(sprite(ns.tex_notes_path.clone()):
                        align(0.5, 0.5):
                        xy(cx + *col_x_offset as f32, cy + y_offset):
                        zoomto(note_def.size[0] as f32, note_def.size[1] as f32):
                        customtexturerect(uv[0], uv[1], uv[2], uv[3])
                    ));
                }
            }
        }
    } else {
        actors.push(act!(quad:
            align(0.5, 0.5):
            xy(cx + state.player_position.x, cy + state.player_position.y):
            zoomto(100.0, 100.0):
            diffuse(state.player_color[0], state.player_color[1], state.player_color[2], state.player_color[3])
        ));
    }
    actors
}
