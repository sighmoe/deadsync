// ===== FILE: src/screens/gameplay.rs =====
use crate::core::input::InputState;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::act;
use crate::core::space::globals::*;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use crate::core::noteskin::{self, Noteskin, Style, Quantization, NUM_QUANTIZATIONS};
use std::path::Path;
use std::time::Instant;
use std::sync::Arc;
use crate::core::timing::TimingData;
use crate::core::song_loading::{get_song_cache, ChartData};

/* ---------------------------- transitions ---------------------------- */
const TRANSITION_IN_DURATION: f32 = 0.4;
const TRANSITION_OUT_DURATION: f32 = 0.4;

const SCROLL_SPEED_SECONDS: f32 = 1.0; // Seconds of chart visible
// --- CHANGED: Receptors are now near the top of the screen ---
const RECEPTOR_Y_FRAC: f32 = 0.15;

#[derive(Clone, Debug)]
pub enum NoteType { Tap, Hold, Roll }

#[derive(Clone, Debug)]
pub struct Note {
    pub beat: f32,
    pub column: usize,
    pub note_type: NoteType,
}

#[derive(Clone, Debug)]
pub struct Arrow {
    pub beat: f32,
    pub column: usize,
    pub note_type: NoteType,
    // No y position; it's calculated dynamically in get_actors
}

pub struct State {
    pub start_time: Instant,
    pub current_beat: f32,
    pub timing: Arc<TimingData>,
    pub chart: Arc<ChartData>,
    pub notes: Vec<Note>,
    pub arrows: [Vec<Arrow>; 4],
    pub noteskin: Option<Noteskin>,
    pub player_color: [f32; 4],
    note_cursor: usize, // To avoid re-scanning the whole chart
}

pub fn init() -> State {
    // TODO: This should take a selected song/chart from the previous screen.
    // For now, we'll just grab the first available chart to get things working.
    let (song, chart) = {
        let cache = get_song_cache();
        let first_song = cache.iter().flat_map(|pack| &pack.songs).next().unwrap();
        let first_chart = first_song.charts.iter().find(|c| !c.notes.is_empty()).unwrap();
        (first_song.clone(), Arc::new(first_chart.clone()))
    };

    let style = Style { num_cols: 4, num_players: 1 };
    let mut noteskin = noteskin::load(Path::new("assets/noteskins/bar/dance-single.txt"), &style).ok()
        .or_else(|| noteskin::load(Path::new("assets/noteskins/bar/all-styles.txt"), &style).ok());

    if let Some(ns) = &mut noteskin {
        let base_path = Path::new("assets");
        ns.tex_notes_dims = image::image_dimensions(base_path.join(&ns.tex_notes_path)).unwrap_or((256, 256));
        ns.tex_receptors_dims = image::image_dimensions(base_path.join(&ns.tex_receptors_path)).unwrap_or((128, 64));
        ns.tex_glow_dims = image::image_dimensions(base_path.join(&ns.tex_glow_path)).unwrap_or((96, 96));
    }
    
    // The old file had chart-specific timing tags, but rssp doesn't seem to parse them.
    // We'll pass None for them and rely on the global tags.
    let timing = Arc::new(TimingData::from_chart_data(
        -song.offset,
        None, // No chart-specific BPMs from rssp yet
        &song.normalized_bpms,
        None, // No chart-specific stops from rssp yet
        "", // rssp doesn't parse global stops yet
    ));

    // --- NEW NOTE PARSING LOGIC ---
    // rssp provides `notes` as a flat Vec<u8> of bitmasks. It discards the measure
    // structure. To reconstruct the beats, we assume a fixed quantization for the
    // entire chart. 48 rows per measure (192nd notes) is a safe, high-resolution guess.
    let mut notes = Vec::new();
    const ROWS_PER_MEASURE: f32 = 48.0; 
    const BEATS_PER_MEASURE: f32 = 4.0;
    
    for (row_index, &mask) in chart.notes.iter().enumerate() {
        if mask == 0 { continue; } // Skip empty rows

        let beat = (row_index as f32 / ROWS_PER_MEASURE) * BEATS_PER_MEASURE;
        for col in 0..4 {
            if (mask >> col) & 1 == 1 {
                notes.push(Note {
                    beat,
                    column: col,
                    note_type: NoteType::Tap, // For now, all notes are taps
                });
            }
        }
    }

    State {
        start_time: Instant::now(),
        current_beat: 0.0,
        timing,
        chart,
        notes,
        arrows: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
        noteskin,
        player_color: [0.0, 0.0, 1.0, 1.0],
        note_cursor: 0,
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
pub fn update(state: &mut State, _input: &InputState, _delta_time: f32) {
    let elapsed_sec = state.start_time.elapsed().as_secs_f32();
    state.current_beat = state.timing.get_beat_for_time(elapsed_sec);

    // Spawn new arrows
    let lookahead_time = state.timing.get_time_for_beat(state.current_beat) + SCROLL_SPEED_SECONDS;
    let lookahead_beat = state.timing.get_beat_for_time(lookahead_time);
    
    while state.note_cursor < state.notes.len() && state.notes[state.note_cursor].beat < lookahead_beat {
        let note = &state.notes[state.note_cursor];
        state.arrows[note.column].push(Arrow {
            beat: note.beat,
            column: note.column,
            note_type: note.note_type.clone(),
        });
        state.note_cursor += 1;
    }

    // Despawn old arrows that have been missed
    let miss_line_beat = state.current_beat - 0.5; // Generous miss window for now
    for col in 0..4 {
        state.arrows[col].retain(|arrow| arrow.beat > miss_line_beat);
    }
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
    let receptor_y = screen_height() * RECEPTOR_Y_FRAC;
    // --- CHANGED: Scroll direction is inverted because receptors are at top ---
    let pixels_per_second = - (screen_height() / SCROLL_SPEED_SECONDS);

    if let Some(ns) = &state.noteskin {
        // 1. Draw Receptors
        for (i, col_x_offset) in ns.column_xs.iter().enumerate() {
            let receptor_def = &ns.receptor_off[i];
            let uv = noteskin::get_uv_rect(receptor_def, ns.tex_receptors_dims);
            
            actors.push(act!(sprite(ns.tex_receptors_path.clone()):
                align(0.5, 0.5):
                xy(cx + *col_x_offset as f32, receptor_y):
                zoomto(receptor_def.size[0] as f32, receptor_def.size[1] as f32):
                customtexturerect(uv[0], uv[1], uv[2], uv[3])
            ));
        }

        // 2. Draw active arrows
        let current_time = state.timing.get_time_for_beat(state.current_beat);

        for column_arrows in &state.arrows {
            for arrow in column_arrows {
                let arrow_time = state.timing.get_time_for_beat(arrow.beat);
                let time_diff = arrow_time - current_time;
                let y_pos = receptor_y - (time_diff * pixels_per_second);
                
                // Culling
                if y_pos < -100.0 || y_pos > screen_height() + 100.0 { continue; }

                let col_x_offset = ns.column_xs[arrow.column];
                
                let beat_fraction = arrow.beat.fract();
                let quantization = match (beat_fraction * 192.0).round() as u32 {
                    0 | 192 => Quantization::Q4th,
                    96 => Quantization::Q8th,
                    48 | 144 => Quantization::Q16th,
                    24 | 72 | 120 | 168 => Quantization::Q32nd,
                    64 | 128 => Quantization::Q12th,
                    32 | 160 => Quantization::Q24th, // or 48th, this is ambiguous
                    _ => Quantization::Q192nd, // fallback for weird stuff
                };

                let note_idx = arrow.column * NUM_QUANTIZATIONS + quantization as usize;
                if let Some(note_def) = ns.notes.get(note_idx) {
                    let uv = noteskin::get_uv_rect(note_def, ns.tex_notes_dims);
                    
                    actors.push(act!(sprite(ns.tex_notes_path.clone()):
                        align(0.5, 0.5):
                        xy(cx + col_x_offset as f32, y_pos):
                        zoomto(note_def.size[0] as f32, note_def.size[1] as f32):
                        customtexturerect(uv[0], uv[1], uv[2], uv[3])
                    ));
                }
            }
        }
    }
    actors
}
