// FILE: src/screens/gameplay.rs

// --- IMPORTS ---
use crate::core::input::InputState; // Though we won't use this for note hits
use crate::core::noteskin::{self, Noteskin, Quantization, Style, NUM_QUANTIZATIONS};
use crate::core::parsing;
use crate::core::song_loading::{ChartData, SongData};
use crate::core::space::globals::*;
use crate::core::timing::TimingData;
use crate::core::audio;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::act;
use log::{info, warn};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

// --- CONSTANTS ---

// Transitions
const TRANSITION_IN_DURATION: f32 = 0.4;
const TRANSITION_OUT_DURATION: f32 = 0.4;

// Gameplay Layout & Feel
const SCROLL_SPEED_SECONDS: f32 = 0.75; // Time for a note to travel from top to bottom
const RECEPTOR_Y_FRAC: f32 = 0.15;      // Receptors are 15% from the bottom of the screen

// Visual Feedback
const RECEPTOR_GLOW_DURATION: f32 = 0.2; // How long the glow sprite is visible
const JUDGMENT_DISPLAY_DURATION: f32 = 0.8; // How long "Perfect" etc. stays on screen

// --- JUDGMENT WINDOWS (in seconds) ---
const MARVELOUS_WINDOW: f32 = 0.022;
const PERFECT_WINDOW: f32 = 0.045;
const GREAT_WINDOW: f32 = 0.090;
const GOOD_WINDOW: f32 = 0.135;
const BOO_WINDOW: f32 = 0.180;
// Notes outside the BOO_WINDOW are considered a Miss.

// --- DATA STRUCTURES ---

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JudgeGrade {
    Marvelous,
    Perfect,
    Great,
    Good,
    Boo,
    Miss,
}

#[derive(Clone, Debug)]
pub struct Judgment {
    pub time_error_ms: f32,
    pub grade: JudgeGrade,
}

#[derive(Clone, Debug)]
pub enum NoteType {
    Tap,
    Hold,
    Roll,
}

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
}

pub struct State {
    // Song & Chart data
    pub song: Arc<SongData>,
    pub chart: Arc<ChartData>,
    pub timing: Arc<TimingData>,
    pub notes: Vec<Note>,
    
    // Gameplay state
    pub start_time: Instant,
    pub current_beat: f32,
    pub music_started: bool,
    pub note_cursor: usize,
    pub arrows: [Vec<Arrow>; 4], // Active on-screen arrows per column
    
    // Scoring & Feedback
    pub judgments: Vec<Judgment>,
    pub combo: u32,
    pub last_judgment: Option<(JudgeGrade, Instant)>,
    
    // Visuals
    pub noteskin: Option<Noteskin>,
    pub player_color: [f32; 4],
    pub receptor_glow_timers: [f32; 4], // Timers for glow effect on each receptor

    // Debugging
    log_timer: f32,
}

// --- INITIALIZATION ---

pub fn init(song: Arc<SongData>, chart: Arc<ChartData>) -> State {
    info!("Initializing Gameplay Screen...");
    info!("Loaded song '{}' and chart '{}'", song.title, chart.difficulty);

    let style = Style { num_cols: 4, num_players: 1 };
    let mut noteskin = noteskin::load(Path::new("assets/noteskins/metal/dance-single.txt"), &style).ok()
        .or_else(|| noteskin::load(Path::new("assets/noteskins/metal/all-styles.txt"), &style).ok());

    if let Some(ns) = &mut noteskin {
        let base_path = Path::new("assets");
        ns.tex_notes_dims = image::image_dimensions(base_path.join(&ns.tex_notes_path)).unwrap_or((256, 256));
        ns.tex_receptors_dims = image::image_dimensions(base_path.join(&ns.tex_receptors_path)).unwrap_or((128, 64));
        ns.tex_glow_dims = image::image_dimensions(base_path.join(&ns.tex_glow_path)).unwrap_or((96, 96));
    }

    let timing = Arc::new(TimingData::from_chart_data(
        -song.offset,
        None, // chart-specific BPMs not supported by this timing data constructor yet
        &song.normalized_bpms,
        None, // chart-specific stops not supported yet
        "",   // global stops
        &chart.notes,
    ));

    let time_at_beat_zero = timing.get_time_for_beat(0.0);
    // Start the clock *before* now if the music has an offset, so elapsed time matches music time.
    let start_time = Instant::now() - Duration::from_secs_f32(time_at_beat_zero.max(0.0));

    let parsed_notes = parsing::simfile::parse_chart_notes(&chart.notes);
    let notes: Vec<Note> = parsed_notes.into_iter().filter_map(|(row_index, column, raw_note_type)| {
        timing.get_beat_for_row(row_index).map(|beat| {
            let note_type = match raw_note_type {
                parsing::simfile::NoteType::Tap => NoteType::Tap,
                parsing::simfile::NoteType::Hold => NoteType::Hold,
                parsing::simfile::NoteType::Roll => NoteType::Roll,
            };
            Note { beat, column, note_type }
        })
    }).collect();

    info!("Parsed {} notes from chart data.", notes.len());

    State {
        song,
        chart,
        timing,
        notes,
        start_time,
        current_beat: 0.0,
        music_started: false,
        note_cursor: 0,
        arrows: [vec![], vec![], vec![], vec![]],
        judgments: Vec::new(),
        combo: 0,
        last_judgment: None,
        noteskin,
        player_color: [0.0, 0.0, 1.0, 1.0], // Default color, can be set from menu
        receptor_glow_timers: [0.0; 4],
        log_timer: 0.0,
    }
}

// --- INPUT HANDLING ---

fn process_hit(state: &mut State, column: usize) {
    let current_time = state.start_time.elapsed().as_secs_f32();

    // Find the first (i.e., earliest) note in the target column
    if let Some(arrow) = state.arrows[column].first() {
        let note_time = state.timing.get_time_for_beat(arrow.beat);
        let time_error = current_time - note_time;
        let abs_time_error = time_error.abs();

        // Check if the hit is within the widest possible timing window
        if abs_time_error <= BOO_WINDOW {
            let grade = if abs_time_error <= MARVELOUS_WINDOW {
                JudgeGrade::Marvelous
            } else if abs_time_error <= PERFECT_WINDOW {
                JudgeGrade::Perfect
            } else if abs_time_error <= GREAT_WINDOW {
                JudgeGrade::Great
            } else if abs_time_error <= GOOD_WINDOW {
                JudgeGrade::Good
            } else {
                JudgeGrade::Boo
            };

            // Process judgment
            info!("HIT! Column {}, Error: {:.2}ms, Grade: {:?}", column, time_error * 1000.0, grade);
            state.judgments.push(Judgment { time_error_ms: time_error * 1000.0, grade: grade.clone() });
            state.last_judgment = Some((grade.clone(), Instant::now()));

            if matches!(grade, JudgeGrade::Boo | JudgeGrade::Miss) {
                state.combo = 0;
            } else {
                state.combo += 1;
            }
            
            // Remove the note that was hit
            state.arrows[column].remove(0);

            // Trigger visual/audio feedback
            state.receptor_glow_timers[column] = RECEPTOR_GLOW_DURATION;

        }
    }
}

pub fn handle_key_press(state: &mut State, event: &KeyEvent) -> ScreenAction {
    if event.state != ElementState::Pressed {
        return ScreenAction::None;
    }
    
    if let PhysicalKey::Code(key_code) = event.physical_key {
        let column = match key_code {
            KeyCode::Escape => return ScreenAction::Navigate(Screen::SelectMusic),
            
            // Player 1 controls (add more as needed)
            KeyCode::ArrowLeft  | KeyCode::KeyD => Some(0),
            KeyCode::ArrowDown  | KeyCode::KeyF => Some(1),
            KeyCode::ArrowUp    | KeyCode::KeyJ => Some(2),
            KeyCode::ArrowRight | KeyCode::KeyK => Some(3),
            
            _ => None,
        };
        
        if let Some(col_idx) = column {
            process_hit(state, col_idx);
        }
    }
    ScreenAction::None
}

// --- UPDATE LOOP ---

#[inline(always)]
pub fn update(state: &mut State, _input: &InputState, delta_time: f32) {
    let elapsed_sec = state.start_time.elapsed().as_secs_f32();

    // Start music on the first valid frame
    if !state.music_started && elapsed_sec >= 0.0 {
        if let Some(music_path) = &state.song.music_path {
            info!("Starting music: {:?}", music_path);
            // We play from the start of the file; the offset is handled by start_time.
            audio::play_music(music_path.clone(), audio::Cut::default(), false);
            state.music_started = true;
        } else {
            warn!("No music path found for song '{}'", state.song.title);
            state.music_started = true; // Prevent this from running every frame
        }
    }
    
    // Update current beat
    state.current_beat = state.timing.get_beat_for_time(elapsed_sec);

    // Update glow timers
    for timer in &mut state.receptor_glow_timers {
        *timer = (*timer - delta_time).max(0.0);
    }

    // --- Add notes from the main list to the active on-screen arrows ---
    // Look ahead in time to see which notes should be on screen
    let lookahead_time = elapsed_sec + SCROLL_SPEED_SECONDS;
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

    // --- Handle missed notes ---
    // A note is missed if the current time has passed its time by more than the BOO_WINDOW
    for col_arrows in &mut state.arrows {
        let mut missed = false;
        if let Some(arrow) = col_arrows.first() {
            let note_time = state.timing.get_time_for_beat(arrow.beat);
            if elapsed_sec - note_time > BOO_WINDOW {
                info!("MISS! Column {}, Beat {:.2}", arrow.column, arrow.beat);
                state.judgments.push(Judgment { time_error_ms: ((elapsed_sec - note_time) * 1000.0) as f32, grade: JudgeGrade::Miss });
                state.last_judgment = Some((JudgeGrade::Miss, Instant::now()));
                state.combo = 0;
                missed = true;
            }
        }
        if missed {
            col_arrows.remove(0);
        }
    }

    // --- Debug Logging ---
    state.log_timer += delta_time;
    if state.log_timer >= 1.0 {
        let active_arrows: usize = state.arrows.iter().map(|v| v.len()).sum();
        info!(
            "Beat: {:.2}, Time: {:.2}, Combo: {}, Active Arrows: {}",
            state.current_beat,
            elapsed_sec,
            state.combo,
            active_arrows
        );
        state.log_timer -= 1.0;
    }
}

// --- TRANSITIONS ---
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
        diffuse(0.0, 0.0, 0.0, 0.0):
        z(1200):
        linear(TRANSITION_OUT_DURATION): alpha(1.0)
    );
    (vec![actor], TRANSITION_OUT_DURATION)
}

// --- DRAWING ---

pub fn get_actors(state: &State) -> Vec<Actor> {
    let mut actors = Vec::new();
    let cx = screen_center_x();
    let receptor_y = screen_height() * RECEPTOR_Y_FRAC;
    let pixels_per_second = screen_height() / SCROLL_SPEED_SECONDS;

    if let Some(ns) = &state.noteskin {
        // 1. Draw Receptors and Glows
        for i in 0..4 {
            let col_x_offset = ns.column_xs[i];
            
            // Draw base receptor
            let receptor_def = &ns.receptor_off[i];
            let uv = noteskin::get_uv_rect(receptor_def, ns.tex_receptors_dims);
            actors.push(act!(sprite(ns.tex_receptors_path.clone()):
                align(0.5, 0.5):
                xy(cx + col_x_offset as f32, receptor_y):
                zoomto(receptor_def.size[0] as f32, receptor_def.size[1] as f32):
                rotationz(-receptor_def.rotation_deg as f32):
                customtexturerect(uv[0], uv[1], uv[2], uv[3])
            ));

            // Draw glow if active
            let glow_timer = state.receptor_glow_timers[i];
            if glow_timer > 0.0 {
                let glow_def = &ns.receptor_glow[i];
                let glow_uv = noteskin::get_uv_rect(glow_def, ns.tex_glow_dims);
                let alpha = (glow_timer / RECEPTOR_GLOW_DURATION).powf(0.75); // Fade out
                actors.push(act!(sprite(ns.tex_glow_path.clone()):
                    align(0.5, 0.5):
                    xy(cx + col_x_offset as f32, receptor_y):
                    zoomto(glow_def.size[0] as f32, glow_def.size[1] as f32):
                    rotationz(-glow_def.rotation_deg as f32):
                    customtexturerect(glow_uv[0], glow_uv[1], glow_uv[2], glow_uv[3]):
                    diffuse(1.0, 1.0, 1.0, alpha):
                    blend(add) // Use additive blending for a nice glow effect
                ));
            }
        }

        // 2. Draw active arrows
        let current_time = state.start_time.elapsed().as_secs_f32();

        for column_arrows in &state.arrows {
            for arrow in column_arrows {
                let arrow_time = state.timing.get_time_for_beat(arrow.beat);
                let time_diff = arrow_time - current_time;
                let y_pos = receptor_y + (time_diff * pixels_per_second);
                
                // Culling
                if y_pos < -100.0 || y_pos > screen_height() + 100.0 { continue; }

                let col_x_offset = ns.column_xs[arrow.column];
                
                // Determine which note sprite to use based on quantization
                let beat_fraction = arrow.beat.fract();
                let quantization = match (beat_fraction * 192.0).round() as u32 {
                    0 | 192 => Quantization::Q4th, 96 => Quantization::Q8th,
                    48 | 144 => Quantization::Q16th, 24 | 72 | 120 | 168 => Quantization::Q32nd,
                    64 | 128 => Quantization::Q12th, 32 | 160 => Quantization::Q24th,
                    _ => Quantization::Q192nd,
                };

                let note_idx = arrow.column * NUM_QUANTIZATIONS + quantization as usize;
                if let Some(note_def) = ns.notes.get(note_idx) {
                    let uv = noteskin::get_uv_rect(note_def, ns.tex_notes_dims);
                    
                    actors.push(act!(sprite(ns.tex_notes_path.clone()):
                        align(0.5, 0.5):
                        xy(cx + col_x_offset as f32, y_pos):
                        zoomto(note_def.size[0] as f32, note_def.size[1] as f32):
                        rotationz(-note_def.rotation_deg as f32):
                        customtexturerect(uv[0], uv[1], uv[2], uv[3])
                    ));
                }
            }
        }
    }
    
    // 3. Draw Combo and Judgment text
    if state.combo > 2 {
        actors.push(act!(text:
            font("wendy"): settext(state.combo.to_string()):
            align(0.5, 0.5): xy(cx, screen_center_y() - 50.0):
            zoom(0.8): horizalign(center)
        ));
    }
    
    if let Some((grade, judged_at)) = &state.last_judgment {
        if judged_at.elapsed().as_secs_f32() < JUDGMENT_DISPLAY_DURATION {
            let (text, color) = match grade {
                JudgeGrade::Marvelous => ("MARVELOUS", [1.0, 1.0, 0.0, 1.0]),
                JudgeGrade::Perfect => ("PERFECT", [1.0, 1.0, 0.0, 1.0]),
                JudgeGrade::Great => ("GREAT", [0.0, 1.0, 0.0, 1.0]),
                JudgeGrade::Good => ("GOOD", [0.0, 0.0, 1.0, 1.0]),
                JudgeGrade::Boo => ("BOO", [1.0, 0.0, 1.0, 1.0]),
                JudgeGrade::Miss => ("MISS", [1.0, 0.0, 0.0, 1.0]),
            };
            actors.push(act!(text:
                font("wendy"): settext(text):
                align(0.5, 0.5): xy(cx, screen_center_y()):
                zoom(0.7): horizalign(center):
                diffuse(color[0], color[1], color[2], color[3])
            ));
        }
    }

    actors
}
