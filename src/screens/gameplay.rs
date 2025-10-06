use crate::core::input::InputState;
use crate::core::noteskin::{self, Noteskin, Quantization, Style, NUM_QUANTIZATIONS};
use crate::screens::select_music::DIFFICULTY_NAMES;
use crate::core::parsing;
use crate::core::song_loading::{ChartData, SongData};
use crate::core::space::globals::*;
use crate::core::timing::TimingData;
use crate::core::audio;
use crate::screens::{Screen, ScreenAction};
use crate::core::space::{is_wide, widescale};
use crate::ui::actors::{Actor, SizeSpec};
use crate::act;
use crate::ui::color;
use crate::ui::components::screen_bar;
use crate::screens::gameplay::screen_bar::ScreenBarParams;
use log::{info, warn};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
// FIX: for precise width measurement of zero/tail without overlap
use crate::core::font;


// --- CONSTANTS ---

// Transitions
const TRANSITION_IN_DURATION: f32 = 0.4;
const TRANSITION_OUT_DURATION: f32 = 0.4;

// Gameplay Layout & Feel
const SCROLL_SPEED_SECONDS: f32 = 0.60; // Time for a note to travel screen_height() pixels
const RECEPTOR_Y_OFFSET_FROM_CENTER: f32 = -125.0; // From Simply Love metrics for standard up-scroll

// Lead-in timing (from StepMania theme defaults)
const MIN_SECONDS_TO_STEP: f32 = 6.0;
const MIN_SECONDS_TO_MUSIC: f32 = 2.0;

// Visual Feedback
const RECEPTOR_GLOW_DURATION: f32 = 0.2; // How long the glow sprite is visible
const JUDGMENT_DISPLAY_DURATION: f32 = 0.8; // How long "Perfect" etc. stays on screen
const SHOW_COMBO_AT: u32 = 4; // From Simply Love metrics

// --- JUDGMENT WINDOWS (in seconds) - From ITG ---
pub const MARVELOUS_WINDOW: f32 = 0.0215; // W1
const PERFECT_WINDOW:   f32 = 0.0430; // W2
const GREAT_WINDOW:     f32 = 0.1020; // W3
const GOOD_WINDOW:      f32 = 0.1350; // W4
const BOO_WINDOW:       f32 = 0.1800; // W5
// Notes outside the BOO_WINDOW are considered a Miss.

// --- DATA STRUCTURES ---

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum JudgeGrade {
    Marvelous, // W1
    Perfect,   // W2
    Great,     // W3
    Good,      // W4
    Boo,       // W5
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

#[derive(Clone, Debug)]
pub struct JudgmentRenderInfo {
    pub judgment: Judgment,
    pub judged_at: Instant,
}


pub struct State {
    // Song & Chart data
    pub song: Arc<SongData>,
    pub chart: Arc<ChartData>,
    pub timing: Arc<TimingData>,
    pub notes: Vec<Note>,
    
    // Gameplay state
    pub song_start_instant: Instant, // The wall-clock moment music t=0 begins (after the initial delay).
    pub start_delay: f32, // The calculated initial pause duration.
    pub current_beat: f32,
    pub current_music_time: f32, // Time calculated at the start of each update frame.
    pub music_started: bool,
    pub note_cursor: usize,
    pub arrows: [Vec<Arrow>; 4], // Active on-screen arrows per column
    
    // Scoring & Feedback
    pub judgments: Vec<Judgment>,
    pub combo: u32,
    pub miss_combo: u32,
    pub full_combo_grade: Option<JudgeGrade>,
    pub judgment_counts: HashMap<JudgeGrade, u32>,
    pub last_judgment: Option<JudgmentRenderInfo>,
    
    // Visuals
    pub noteskin: Option<Noteskin>,
    pub active_color_index: i32,
    pub player_color: [f32; 4],
    pub receptor_glow_timers: [f32; 4], // Timers for glow effect on each receptor

    // Animation timing for this screen
    pub total_elapsed_in_screen: f32,

    // Debugging
    log_timer: f32,
}

// --- INITIALIZATION ---

pub fn init(song: Arc<SongData>, chart: Arc<ChartData>, active_color_index: i32) -> State {
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

    let config = crate::config::get();
    let timing = Arc::new(TimingData::from_chart_data(
        -song.offset,
        config.global_offset_seconds,
        None, // chart-specific BPMs not supported by this timing data constructor yet
        &song.normalized_bpms,
        None, // chart-specific stops not supported yet
        "",   // global stops
        &chart.notes,
    ));

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

    // --- StepMania Timing Logic Implementation ---
    // 1. Find the time of the first note relative to the music file's start.
    let first_note_beat = notes.first().map_or(0.0, |n| n.beat);
    let first_second = timing.get_time_for_beat(first_note_beat);

    // 2. Calculate the required preroll delay to meet theme metrics.
    let start_delay = (MIN_SECONDS_TO_STEP - first_second).max(MIN_SECONDS_TO_MUSIC);
    
    // 3. Schedule the visual clock's "time zero" to be `start_delay` seconds in the future.
    let song_start_instant = Instant::now() + Duration::from_secs_f32(start_delay);

    // 4. Immediately tell the audio engine to start playing, but with a negative
    //    start time. The audio engine will fill the beginning with silence.
    let music_started = if let Some(music_path) = &song.music_path {
        info!("Starting music with a preroll delay of {:.2}s", start_delay);
        let cut = audio::Cut { start_sec: (-start_delay) as f64, length_sec: f64::INFINITY };
        audio::play_music(music_path.clone(), cut, false);
        true
    } else {
        warn!("No music path found for song '{}'", song.title);
        true // Set to true to prevent trying again every frame
    };

    State {
        song,
        chart,
        timing,
        notes,
        song_start_instant,
        start_delay,
        current_beat: 0.0,
        current_music_time: -start_delay, // At screen t=0, music time is negative
        music_started,
        note_cursor: 0,
        arrows: [vec![], vec![], vec![], vec![]],
        judgment_counts: HashMap::from([
            (JudgeGrade::Marvelous, 0),
            (JudgeGrade::Perfect, 0),
            (JudgeGrade::Great, 0),
            (JudgeGrade::Good, 0),
            (JudgeGrade::Boo, 0),
            (JudgeGrade::Miss, 0),
        ]),
        judgments: Vec::new(),
        combo: 0,
        miss_combo: 0,
        full_combo_grade: None,
        last_judgment: None,
        noteskin,
        active_color_index,
        player_color: color::decorative_rgba(active_color_index),
        receptor_glow_timers: [0.0; 4],
        total_elapsed_in_screen: 0.0,
        log_timer: 0.0,
    }
}

// --- INPUT HANDLING ---

fn process_hit(state: &mut State, column: usize, current_time: f32) {
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
            let judgment = Judgment { time_error_ms: time_error * 1000.0, grade: grade.clone() };
            state.judgments.push(judgment.clone());
            state.last_judgment = Some(JudgmentRenderInfo { judgment, judged_at: Instant::now() });
            // Increment the counter for this grade
            *state.judgment_counts.entry(grade.clone()).or_insert(0) += 1;

            state.miss_combo = 0; // Any hit breaks a miss combo
            if matches!(grade, JudgeGrade::Boo) {
                state.combo = 0;
                state.full_combo_grade = None;
            } else {
                state.combo += 1;
                
                // Update full combo grade: if a worse grade is hit, the FC color downgrades
                if let Some(current_fc_grade) = &state.full_combo_grade {
                    state.full_combo_grade = Some(grade.clone().max(current_fc_grade.clone()));
                } else {
                    state.full_combo_grade = Some(grade.clone());
                }
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
            let now = Instant::now();
            let hit_time = if now < state.song_start_instant {
                -(state.song_start_instant.saturating_duration_since(now).as_secs_f32())
            } else {
                now.saturating_duration_since(state.song_start_instant).as_secs_f32()
            };
            process_hit(state, col_idx, hit_time);
        }
    }
    ScreenAction::None
}

// --- UPDATE LOOP ---

#[inline(always)]
pub fn update(state: &mut State, _input: &InputState, delta_time: f32) {
    state.total_elapsed_in_screen += delta_time;

    let now = Instant::now();
    let music_time_sec = if now < state.song_start_instant {
        -(state.song_start_instant.saturating_duration_since(now).as_secs_f32())
    } else {
        now.saturating_duration_since(state.song_start_instant).as_secs_f32()
    };
    state.current_music_time = music_time_sec;
    
    // Update current beat
    state.current_beat = state.timing.get_beat_for_time(music_time_sec);

    // Update glow timers
    for timer in &mut state.receptor_glow_timers {
        *timer = (*timer - delta_time).max(0.0);
    }

    // --- Add notes from the main list to the active on-screen arrows ---
    // Look ahead in time to see which notes should be on screen
    let lookahead_time = music_time_sec + SCROLL_SPEED_SECONDS;
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
            if music_time_sec - note_time > BOO_WINDOW {
                info!("MISS! Column {}, Beat {:.2}", arrow.column, arrow.beat);
                let judgment = Judgment {
                    time_error_ms: ((music_time_sec - note_time) * 1000.0),
                    grade: JudgeGrade::Miss,
                };
                state.judgments.push(judgment.clone());
                // Increment the miss counter
                *state.judgment_counts.entry(JudgeGrade::Miss).or_insert(0) += 1;

                state.last_judgment = Some(JudgmentRenderInfo { judgment, judged_at: Instant::now() });

                state.combo = 0;
                state.miss_combo += 1;
                state.full_combo_grade = None;
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
            "Beat: {:.2}, Time: {:.2}, Combo: {}, Misses: {}, Active Arrows: {}",
            state.current_beat,
            music_time_sec,
            state.combo,
            state.miss_combo,
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
        diffuse(0.0, 0.0, 0.0, 1.0):
        z(1100):
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

// --- NEW: Statics for Judgment Counter Display ---

static JUDGMENT_ORDER: [JudgeGrade; 6] = [
    JudgeGrade::Marvelous,
    JudgeGrade::Perfect,
    JudgeGrade::Great,
    JudgeGrade::Good,
    JudgeGrade::Boo,
    JudgeGrade::Miss,
];

struct JudgmentDisplayInfo {
    label: &'static str,
    color: [f32; 4],
}

static JUDGMENT_INFO: LazyLock<HashMap<JudgeGrade, JudgmentDisplayInfo>> = LazyLock::new(|| {
    HashMap::from([
        (JudgeGrade::Marvelous, JudgmentDisplayInfo { label: "FANTASTIC", color: color::rgba_hex(color::JUDGMENT_HEX[0]) }),
        (JudgeGrade::Perfect,   JudgmentDisplayInfo { label: "EXCELLENT", color: color::rgba_hex(color::JUDGMENT_HEX[1]) }),
        (JudgeGrade::Great,     JudgmentDisplayInfo { label: "GREAT",     color: color::rgba_hex(color::JUDGMENT_HEX[2]) }),
        (JudgeGrade::Good,      JudgmentDisplayInfo { label: "DECENT",    color: color::rgba_hex(color::JUDGMENT_HEX[3]) }),
        (JudgeGrade::Boo,       JudgmentDisplayInfo { label: "WAY OFF",   color: color::rgba_hex(color::JUDGMENT_HEX[4]) }),
        (JudgeGrade::Miss,      JudgmentDisplayInfo { label: "MISS",      color: color::rgba_hex(color::JUDGMENT_HEX[5]) }),
    ])
});

// --- DRAWING ---

pub fn get_actors(state: &State) -> Vec<Actor> {
    let mut actors = Vec::new();
    
    // --- Playfield Positioning (1:1 with Simply Love) ---
    let logical_screen_width = screen_width();
    let clamped_width = logical_screen_width.clamp(640.0, 854.0);
    let playfield_center_x = screen_center_x() - (clamped_width * 0.25);

    let receptor_y = screen_center_y() + RECEPTOR_Y_OFFSET_FROM_CENTER;
    let pixels_per_second = screen_height() / SCROLL_SPEED_SECONDS;

    if let Some(ns) = &state.noteskin {
        // 1. Receptors + glow
        for i in 0..4 {
            let col_x_offset = ns.column_xs[i];
            
            let receptor_def = &ns.receptor_off[i];
            let uv = noteskin::get_uv_rect(receptor_def, ns.tex_receptors_dims);
            actors.push(act!(sprite(ns.tex_receptors_path.clone()):
                align(0.5, 0.5):
                xy(playfield_center_x + col_x_offset as f32, receptor_y):
                zoomto(receptor_def.size[0] as f32, receptor_def.size[1] as f32):
                rotationz(-receptor_def.rotation_deg as f32):
                customtexturerect(uv[0], uv[1], uv[2], uv[3])
            ));

            let glow_timer = state.receptor_glow_timers[i];
            if glow_timer > 0.0 {
                let glow_def = &ns.receptor_glow[i];
                let glow_uv = noteskin::get_uv_rect(glow_def, ns.tex_glow_dims);
                let alpha = (glow_timer / RECEPTOR_GLOW_DURATION).powf(0.75);
                actors.push(act!(sprite(ns.tex_glow_path.clone()):
                    align(0.5, 0.5):
                    xy(playfield_center_x + col_x_offset as f32, receptor_y):
                    zoomto(glow_def.size[0] as f32, glow_def.size[1] as f32):
                    rotationz(-glow_def.rotation_deg as f32):
                    customtexturerect(glow_uv[0], glow_uv[1], glow_uv[2], glow_uv[3]):
                    diffuse(1.0, 1.0, 1.0, alpha):
                    blend(add)
                ));
            }
        }

        // 2. Active arrows
        let current_time = state.current_music_time;

        for column_arrows in &state.arrows {
            for arrow in column_arrows {
                let arrow_time = state.timing.get_time_for_beat(arrow.beat);
                let time_diff = arrow_time - current_time;
                let y_pos = receptor_y + (time_diff * pixels_per_second);
                
                if y_pos < -100.0 || y_pos > screen_height() + 100.0 { continue; }

                let col_x_offset = ns.column_xs[arrow.column];
                
                let beat_fraction = arrow.beat.fract();
                let quantization = match (beat_fraction * 192.0).round() as u32 {
                    0 | 192 => Quantization::Q4th,
                    96 => Quantization::Q8th,
                    48 | 144 => Quantization::Q16th,
                    24 | 72 | 120 | 168 => Quantization::Q32nd,
                    64 | 128 => Quantization::Q12th,
                    32 | 160 => Quantization::Q24th,
                    _ => Quantization::Q192nd,
                };

                let note_idx = arrow.column * NUM_QUANTIZATIONS + quantization as usize;
                if let Some(note_def) = ns.notes.get(note_idx) {
                    let uv = noteskin::get_uv_rect(note_def, ns.tex_notes_dims);
                    
                    actors.push(act!(sprite(ns.tex_notes_path.clone()):
                        align(0.5, 0.5):
                        xy(playfield_center_x + col_x_offset as f32, y_pos):
                        zoomto(note_def.size[0] as f32, note_def.size[1] as f32):
                        rotationz(-note_def.rotation_deg as f32):
                        customtexturerect(uv[0], uv[1], uv[2], uv[3])
                    ));
                }
            }
        }
    }
    
    // 3. Combo
    if state.miss_combo >= SHOW_COMBO_AT {
        actors.push(act!(text:
            font("wendy_combo"): settext(state.miss_combo.to_string()):
            align(0.5, 0.5): xy(playfield_center_x, screen_center_y() + 30.0):
            zoom(0.75): horizalign(center):
            diffuse(1.0, 0.0, 0.0, 1.0):
            z(200)
        ));
    } else if state.combo >= SHOW_COMBO_AT {
        let (color1, color2) = if let Some(fc_grade) = &state.full_combo_grade {
            match fc_grade {
                JudgeGrade::Marvelous => (color::rgba_hex("#C8FFFF"), color::rgba_hex("#6BF0FF")),
                JudgeGrade::Perfect   => (color::rgba_hex("#FDFFC9"), color::rgba_hex("#FDDB85")),
                JudgeGrade::Great     => (color::rgba_hex("#C9FFC9"), color::rgba_hex("#94FEC1")),
                _                     => ([1.0, 1.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0]),
            }
        } else {
            ([1.0, 1.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0])
        };

        let effect_period = 0.8;
        let t = (state.total_elapsed_in_screen / effect_period).fract();
        let anim_t = ( (t * 2.0 * std::f32::consts::PI).sin() + 1.0) / 2.0;

        let final_color = [
            color1[0] + (color2[0] - color1[0]) * anim_t,
            color1[1] + (color2[1] - color1[1]) * anim_t,
            color1[2] + (color2[2] - color1[2]) * anim_t,
            1.0,
        ];
        
        actors.push(act!(text:
            font("wendy_combo"): settext(state.combo.to_string()):
            align(0.5, 0.5): xy(playfield_center_x, screen_center_y() + 30.0):
            zoom(0.75): horizalign(center):
            diffuse(final_color[0], final_color[1], final_color[2], final_color[3]):
            z(200)
        ));
    }
    
    // 4. Judgment Sprite (Love)
    if let Some(render_info) = &state.last_judgment {
        let judgment = &render_info.judgment;
        let elapsed = render_info.judged_at.elapsed().as_secs_f32();
        if elapsed < 0.9 {
            let zoom = if elapsed < 0.1 {
                let t = elapsed / 0.1;
                let ease_t = 1.0 - (1.0 - t).powi(2);
                0.8 + (0.75 - 0.8) * ease_t
            } else if elapsed < 0.7 {
                0.75
            } else {
                let t = (elapsed - 0.7) / 0.2;
                let ease_t = t.powi(2);
                0.75 * (1.0 - ease_t)
            };

            let offset_sec = judgment.time_error_ms / 1000.0;
            let tilt_multiplier = 1.0;
            let offset_rot = offset_sec.abs().min(0.050) * 300.0 * tilt_multiplier;
            let direction = if offset_sec < 0.0 { -1.0 } else { 1.0 };
            let rot = if judgment.grade == JudgeGrade::Miss { 0.0 } else { direction * offset_rot };

            // Frame selection (skip white fantastic row)
            let mut frame_base = judgment.grade as usize;
            if judgment.grade >= JudgeGrade::Perfect {
                frame_base += 1;
            }
            let frame_offset = if offset_sec < 0.0 { 0 } else { 1 };
            let linear_index = (frame_base * 2 + frame_offset) as u32;

            actors.push(act!(sprite("judgements/Love 2x7 (doubleres).png"):
                align(0.5, 0.5): xy(playfield_center_x, screen_center_y() - 30.0):
                z(200):
                zoomtoheight(64.0):
                setstate(linear_index):
                zoom(zoom)
            ));
        }
    }

    // 5. Difficulty Box
    let x = screen_center_x() - widescale(292.5, 342.5);
    let y = 56.0;

    let difficulty_index = DIFFICULTY_NAMES
        .iter()
        .position(|&name| name.eq_ignore_ascii_case(&state.chart.difficulty))
        .unwrap_or(2);

    let difficulty_color_index = state.active_color_index - (4 - difficulty_index) as i32;
    let difficulty_color = color::simply_love_rgba(difficulty_color_index);

    let meter_text = state.chart.meter.to_string();

    let difficulty_meter_frame = Actor::Frame {
        align: [0.5, 0.5],
        offset: [x, y],
        size: [SizeSpec::Px(0.0), SizeSpec::Px(0.0)],
        children: vec![
            act!(quad:
                align(0.5, 0.5): xy(0.0, 0.0):
                zoomto(30.0, 30.0):
                diffuse(difficulty_color[0], difficulty_color[1], difficulty_color[2], 1.0)
            ),
            act!(text:
                font("wendy"): settext(meter_text):
                align(0.5, 0.5): xy(0.0, 0.0):
                zoom(0.4): diffuse(0.0, 0.0, 0.0, 1.0)
            )
        ],
        background: None,
        z: 100,
    };
    actors.push(difficulty_meter_frame);

    // 6. Song Title Box (SongMeter)
    {
        let w = widescale(310.0, 417.0);
        let h = 22.0;
        let box_cx = screen_center_x();
        let box_cy = 20.0;

        let mut frame_children = Vec::new();

        frame_children.push(act!(quad:
            align(0.5, 0.5): xy(w / 2.0, h / 2.0):
            zoomto(w, h):
            diffuse(1.0, 1.0, 1.0, 1.0):
            z(0)
        ));
        frame_children.push(act!(quad:
            align(0.5, 0.5): xy(w / 2.0, h / 2.0):
            zoomto(w - 4.0, h - 4.0):
            diffuse(0.0, 0.0, 0.0, 1.0):
            z(1)
        ));

        if state.song.total_length_seconds > 0 && state.current_music_time >= 0.0 {
            let progress = (state.current_music_time / state.song.total_length_seconds as f32).clamp(0.0, 1.0);
            let stream_max_w = w - 4.0;
            let stream_h = h - 4.0;
            let stream_current_w = stream_max_w * progress;

            frame_children.push(act!(quad:
                align(0.0, 0.5):
                xy(2.0, h / 2.0):
                zoomto(stream_current_w, stream_h):
                diffuse(state.player_color[0], state.player_color[1], state.player_color[2], 1.0):
                z(2)
            ));
        }

        let full_title = if state.song.subtitle.trim().is_empty() {
            state.song.title.clone()
        } else {
            format!("{} {}", state.song.title, state.song.subtitle)
        };

        frame_children.push(act!(text:
            font("miso"): settext(full_title):
            align(0.5, 0.5): xy(w / 2.0, h / 2.0):
            zoom(0.8):
            maxwidth(screen_width() / 2.5 - 10.0):
            horizalign(center):
            z(3)
        ));

        actors.push(Actor::Frame {
            align: [0.5, 0.5],
            offset: [box_cx, box_cy],
            size: [SizeSpec::Px(w), SizeSpec::Px(h)],
            background: None,
            z: 150,
            children: frame_children,
        });
    }

    // --- LIFE METER (P1) ---
    {
        let w = 136.0;
        let h = 18.0;
        let meter_cx = screen_center_x() - widescale(238.0, 288.0);
        let meter_cy = 20.0;

        actors.push(act!(quad:
            align(0.5, 0.5):
            xy(meter_cx, meter_cy):
            zoomto(w + 4.0, h + 4.0):
            diffuse(1.0, 1.0, 1.0, 1.0):
            z(150)
        ));
        actors.push(act!(quad:
            align(0.5, 0.5):
            xy(meter_cx, meter_cy):
            zoomto(w, h):
            diffuse(0.0, 0.0, 0.0, 1.0):
            z(151)
        ));

        let fill_color = state.player_color;
        actors.push(act!(quad:
            align(0.0, 0.5):
            xy(meter_cx - w / 2.0, meter_cy):
            zoomto(w, h):
            diffuse(fill_color[0], fill_color[1], fill_color[2], fill_color[3]):
            z(152)
        ));

        let current_bpm = state.timing.get_bpm_for_beat(state.current_beat);
        let bps = current_bpm / 60.0;
        let velocity_x = -(bps * 0.5);

        actors.push(act!(sprite("swoosh.png"):
            align(0.0, 0.5):
            xy(meter_cx - w / 2.0, meter_cy):
            zoomto(w, h):
            diffusealpha(0.2):
            texcoordvelocity(velocity_x, 0.0):
            z(153)
        ));
    }

    // --- Bottom Bar with Profile Name ---
    actors.push(screen_bar::build(ScreenBarParams {
        title: "",
        title_placement: screen_bar::ScreenBarTitlePlacement::Center,
        position: screen_bar::ScreenBarPosition::Bottom,
        transparent: true,
        fg_color: [1.0; 4],
        left_text: Some("PerfectTaste"), center_text: None, right_text: None,
    }));
    
    // --- Step Statistics Side Pane (P1) ---
    actors.extend(build_side_pane(state));

    actors
}

/// Builds the entire right-side statistics pane, including judgment counters.
fn build_side_pane(state: &State) -> Vec<Actor> {
    // Only show this pane in single-player on a wide screen, mirroring the SL theme's behavior.
    if !is_wide() {
        return vec![];
    }

    let mut actors = Vec::new();

    // --- StepStatsPane container parity (SL defaults for single player) ---
    let sidepane_center_x = screen_width() * 0.75;
    let sidepane_center_y = screen_center_y() + 80.0;

    // Determine if notefield is centered (approximation based on our notefield math).
    let logical_screen_width = screen_width();
    let clamped_width = logical_screen_width.clamp(640.0, 854.0);
    let nf_center_x = screen_center_x() - (clamped_width * 0.25);
    let note_field_is_centered = (nf_center_x - screen_center_x()).abs() < 1.0;

    // Parent zoom for BannerAndData (SL only shrinks when Center1Player & wide)
    let is_ultrawide = screen_width() / screen_height() > (21.0 / 9.0);
    // FIX: default 1.0; only shrink in Center1Player-like case (rough parity)
    let banner_data_zoom = if note_field_is_centered && is_wide() && !is_ultrawide {
        let ar = screen_width() / screen_height();
        let t = ((ar - (16.0/10.0)) / ((16.0/9.0) - (16.0/10.0))).clamp(0.0, 1.0);
        0.825 + (0.925 - 0.825) * t
    } else {
        1.0
    };

    // Local offset for TapNoteJudgments inside BannerAndData:
    // P1 → negative (we only draw P1 here)
    let judgments_local_x = -widescale(152.0, 204.0);

    // FIX: child frame has zoom(0.8) but its x is not scaled by its own zoom; only by parent.
    let final_judgments_center_x = sidepane_center_x + (judgments_local_x * banner_data_zoom);
    let final_judgments_center_y = sidepane_center_y;

    // TapNoteJudgments zoom(0.8) like SL; children inherit this
    let parent_local_zoom = 0.8;
    let final_text_base_zoom = banner_data_zoom * parent_local_zoom;

    // Digits (SL: max(4, floor(log10(total))+1))
    let total_tapnotes = state.chart.stats.total_steps as f32;
    let digits = if total_tapnotes > 0.0 {
        (total_tapnotes.log10().floor() as usize + 1).max(4)
    } else {
        4
    };

    // --- NEW: Calculate label horizontal position first to anchor the numbers ---
    // SL positions the label's right edge at x = 80 + (digits-4)*16 relative to the frame center.
    // Our left-aligned labels (which you've confirmed are correctly placed) use this as a left-edge offset.
    let label_local_x_offset = 80.0 + (digits.saturating_sub(4) as f32 * 16.0);
    let label_world_x = final_judgments_center_x + (label_local_x_offset * final_text_base_zoom);

    // The right edge of the number block should be a small gap to the left of the label's left edge.
    // This value is chosen to visually match the theme.
    const NUMBER_TO_LABEL_GAP: f32 = 8.0;
    let numbers_cx = label_world_x - NUMBER_TO_LABEL_GAP;

    let row_height = 35.0;
    let y_base = -280.0; 

    for (index, grade) in JUDGMENT_ORDER.iter().enumerate() {
        let info = JUDGMENT_INFO.get(grade).unwrap();
        let count = *state.judgment_counts.get(grade).unwrap_or(&0);
        
        let local_y = y_base + (index as f32 * row_height);
        let world_y = final_judgments_center_y + (local_y * final_text_base_zoom);

        // Colors
        let bright = info.color;
        let dim = [bright[0]*0.35, bright[1]*0.35, bright[2]*0.35, bright[3]];

        // ---------- Numbers (right-aligned), SL-style leading-zero dim without overlap ----------
        let full_number_str = format!("{:0width$}", count, width = digits);
        let first_nonzero = full_number_str.find(|c: char| c != '0').unwrap_or(full_number_str.len());

        let zeros_part = &full_number_str[..first_nonzero];
        let tail_part  = &full_number_str[first_nonzero..];

        let numbers_zoom = final_text_base_zoom * 0.5;

        // Measure logical widths and scale by zoom; fallback to a safe monospace-ish guess.
        let (zeros_w, tail_w) = font::with_font("wendy_screenevaluation", |f| {
            let zw = (font::measure_line_width_logical(f, zeros_part) as f32) * numbers_zoom;
            let tw = (font::measure_line_width_logical(f, tail_part)  as f32) * numbers_zoom;
            (zw, tw)
        }).unwrap_or_else(|| {
            let gw = 12.0 * numbers_zoom;
            (zeros_part.len() as f32 * gw, tail_part.len() as f32 * gw)
        });

        // 1) Bright tail: right-aligned to numbers_cx
        if !tail_part.is_empty() {
            actors.push(act!(text:
                font("wendy_screenevaluation"):
                settext(tail_part.to_string()):
                align(1.0, 0.5):
                xy(numbers_cx, world_y):
                zoom(numbers_zoom):
                horizalign(right):
                diffuse(bright[0], bright[1], bright[2], bright[3])
            ));
        }

        // 2) Dim zeros: right-aligned to the left edge of the tail (no overlap)
        if !zeros_part.is_empty() {
            actors.push(act!(text:
                font("wendy_screenevaluation"):
                settext(zeros_part.to_string()):
                align(1.0, 0.5):
                xy(numbers_cx - tail_w, world_y):
                zoom(numbers_zoom):
                horizalign(right):
                diffuse(dim[0], dim[1], dim[2], dim[3])
            ));
        }

        // ---------- Label (left-aligned, position is now calculated above) ----------
        let label_world_y = world_y + (1.0 * final_text_base_zoom);
        let label_zoom = final_text_base_zoom * 0.833;

        // SL keeps labels bright (we’re not implementing disabled-window dim in this file)
        actors.push(act!(text:
            font("miso"):
            settext(info.label):
            align(0.0, 0.5):
            xy(label_world_x, label_world_y):
            zoom(label_zoom):
            maxwidth(72.0 * final_text_base_zoom):
            horizalign(left):
            diffuse(bright[0], bright[1], bright[2], bright[3])
        ));
    }

    actors
}
