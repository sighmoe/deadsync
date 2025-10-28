use crate::act;
use crate::assets::AssetManager;
use crate::core::audio;
use crate::core::input::InputState;
use crate::core::space::*;
use crate::core::space::{is_wide, widescale};
use crate::gameplay::chart::{ChartData, NoteType as ChartNoteType};
use crate::gameplay::parsing::notes as note_parser;
use crate::gameplay::parsing::noteskin::{self, NUM_QUANTIZATIONS, Noteskin, Quantization, Style};
use crate::gameplay::profile::{self, ScrollSpeedSetting};
use crate::gameplay::song::SongData;
use crate::gameplay::timing::TimingData;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::{Actor, SizeSpec};
use crate::ui::color;
use crate::ui::components::screen_bar::{self, ScreenBarParams};
use crate::ui::font;
use log::{info, warn};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

impl ScrollSpeedSetting {
    pub const ARROW_SPACING: f32 = 64.0;

    pub fn effective_bpm(self, current_chart_bpm: f32, reference_bpm: f32) -> f32 {
        match self {
            ScrollSpeedSetting::CMod(bpm) => bpm,
            ScrollSpeedSetting::XMod(multiplier) => current_chart_bpm * multiplier,
            ScrollSpeedSetting::MMod(target_bpm) => {
                if reference_bpm > 0.0 {
                    current_chart_bpm * (target_bpm / reference_bpm)
                } else {
                    current_chart_bpm
                }
            }
        }
    }

    pub fn beat_multiplier(self, reference_bpm: f32) -> f32 {
        match self {
            ScrollSpeedSetting::XMod(multiplier) => multiplier,
            ScrollSpeedSetting::MMod(target_bpm) => {
                if reference_bpm > 0.0 {
                    target_bpm / reference_bpm
                } else {
                    1.0
                }
            }
            _ => 1.0,
        }
    }

    pub fn pixels_per_second(self, current_chart_bpm: f32, reference_bpm: f32) -> f32 {
        let bpm = self.effective_bpm(current_chart_bpm, reference_bpm);
        if !bpm.is_finite() || bpm <= 0.0 {
            0.0
        } else {
            (bpm / 60.0) * Self::ARROW_SPACING
        }
    }

    pub fn travel_time_seconds(
        self,
        draw_distance: f32,
        current_chart_bpm: f32,
        reference_bpm: f32,
    ) -> f32 {
        let speed = self.pixels_per_second(current_chart_bpm, reference_bpm);
        if speed <= 0.0 {
            0.0
        } else {
            draw_distance / speed
        }
    }
}

// --- CONSTANTS ---

// Transitions
const TRANSITION_IN_DURATION: f32 = 0.4;
const TRANSITION_OUT_DURATION: f32 = 0.4;

// Gameplay Layout & Feel
const RECEPTOR_Y_OFFSET_FROM_CENTER: f32 = -125.0; // From Simply Love metrics for standard up-scroll
const TARGET_ARROW_PIXEL_SIZE: f32 = 64.0; // Match Simply Love's on-screen arrow height
const TARGET_EXPLOSION_PIXEL_SIZE: f32 = 125.0; // Simply Love tap explosions top out around 125px tall

//const DANGER_THRESHOLD: f32 = 0.2; // For implementation of red/green flashing light

// Lead-in timing (from StepMania theme defaults)
const MIN_SECONDS_TO_STEP: f32 = 6.0;
const MIN_SECONDS_TO_MUSIC: f32 = 2.0;
const M_MOD_HIGH_CAP: f32 = 600.0;

// Visual Feedback
const RECEPTOR_GLOW_DURATION: f32 = 0.2; // How long the glow sprite is visible
const SHOW_COMBO_AT: u32 = 4; // From Simply Love metrics

// --- JUDGMENT WINDOWS (in seconds) ---
// These are the base values from StepMania's defaults.
// A small constant is added at runtime to match ITG's precise breakpoints,
// as discovered from reverse-engineering Simply Love's timing logic.
const TIMING_WINDOW_ADD: f32 = 0.0015;

pub const BASE_FANTASTIC_WINDOW: f32 = 0.0215; // W1 (0.0230 final)
const BASE_EXCELLENT_WINDOW: f32 = 0.0430; // W2 (0.0445 final)
const BASE_GREAT_WINDOW: f32 = 0.1020; // W3 (0.1035 final)
const BASE_DECENT_WINDOW: f32 = 0.1350; // W4 (0.1365 final)
const BASE_WAY_OFF_WINDOW: f32 = 0.1800; // W5 (0.1815 final)
// Notes outside the final WayOff window are considered a Miss.

// --- DATA STRUCTURES ---

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum JudgeGrade {
    Fantastic, // W1
    Excellent, // W2
    Great,     // W3
    Decent,    // W4
    WayOff,    // W5
    Miss,
}

#[derive(Clone, Debug)]
pub struct Judgment {
    pub time_error_ms: f32,
    pub grade: JudgeGrade, // The grade of this specific note
    pub row: usize,        // The row this judgment belongs to
}

#[derive(Clone, Debug)]
pub enum NoteType {
    Tap,
    Hold,
    Roll,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HoldResult {
    Held,
    LetGo,
}

#[derive(Clone, Debug)]
pub struct HoldData {
    pub end_row_index: usize,
    pub end_beat: f32,
    pub result: Option<HoldResult>,
}

#[derive(Clone, Debug)]
pub struct Note {
    pub beat: f32,
    pub column: usize,
    pub note_type: NoteType,
    // NEW: Add a row index for grouping and a place to store results
    pub row_index: usize,
    pub result: Option<Judgment>,
    pub hold: Option<HoldData>,
}

#[derive(Clone, Debug)]
pub struct Arrow {
    pub beat: f32,
    pub column: usize,
    #[allow(dead_code)]
    pub note_type: NoteType,
    // NEW: Add an index back to the main `notes` Vec to link visual arrows to their data
    pub note_index: usize,
}

#[derive(Clone, Debug)]
pub struct JudgmentRenderInfo {
    pub judgment: Judgment,
    pub judged_at: Instant,
}

#[derive(Clone, Debug)]
struct ActiveTapExplosion {
    window: String,
    elapsed: f32,
    start_beat: f32,
}

#[derive(Clone, Debug)]
struct ActiveHold {
    note_index: usize,
    end_time: f32,
    note_type: NoteType,
    let_go: bool,
    last_input_time: f32,
    is_pressed: bool,
}

impl ActiveHold {
    fn is_engaged(&self, now: f32) -> bool {
        if self.let_go {
            return false;
        }

        self.is_pressed || (now - self.last_input_time) <= HOLD_DROP_TOLERANCE
    }
}

// NEW: Life change constants
struct LifeChange;
impl LifeChange {
    const FANTASTIC: f32 = 0.008;
    const EXCELLENT: f32 = 0.008;
    const GREAT: f32 = 0.004;
    const DECENT: f32 = 0.0;
    const WAY_OFF: f32 = -0.050;
    const MISS: f32 = -0.100;
    #[allow(dead_code)]
    const HIT_MINE: f32 = -0.050;
    const HELD: f32 = 0.008;
    #[allow(dead_code)]
    const LET_GO: f32 = -0.080;
}

fn grade_to_window(grade: JudgeGrade) -> Option<&'static str> {
    match grade {
        JudgeGrade::Fantastic => Some("W1"),
        JudgeGrade::Excellent => Some("W2"),
        JudgeGrade::Great => Some("W3"),
        JudgeGrade::Decent => Some("W4"),
        JudgeGrade::WayOff => Some("W5"),
        JudgeGrade::Miss => None,
    }
}

fn trigger_tap_explosion(state: &mut State, column: usize, grade: JudgeGrade) {
    let Some(window_key) = grade_to_window(grade) else {
        return;
    };

    let spawn_window = state.noteskin.as_ref().and_then(|ns| {
        if ns.tap_explosions.contains_key(window_key) {
            Some(window_key.to_string())
        } else {
            None
        }
    });

    if let Some(window) = spawn_window {
        state.tap_explosions[column] = Some(ActiveTapExplosion {
            window,
            elapsed: 0.0,
            start_beat: state.current_beat,
        });
    }
}

fn handle_hold_let_go(state: &mut State, column: usize, note_index: usize) {
    if let Some(hold) = state.notes[note_index].hold.as_mut() {
        if hold.result == Some(HoldResult::LetGo) {
            return;
        }
        hold.result = Some(HoldResult::LetGo);
    }

    state.change_life(LifeChange::LET_GO);
    state.combo = 0;
    state.miss_combo = state.miss_combo.saturating_add(1);
    state.combo_after_miss = 0;
    if state.full_combo_grade.is_some() {
        state.first_fc_attempt_broken = true;
    }
    state.full_combo_grade = None;
    state.receptor_glow_timers[column] = 0.0;
}

fn handle_hold_success(state: &mut State, note_index: usize) {
    if let Some(hold) = state.notes[note_index].hold.as_mut() {
        if hold.result == Some(HoldResult::Held) {
            return;
        }
        hold.result = Some(HoldResult::Held);
    }

    if matches!(state.notes[note_index].note_type, NoteType::Hold) {
        state.holds_held = state.holds_held.saturating_add(1);
    }
    state.change_life(LifeChange::HELD);
    state.miss_combo = 0;
}

fn update_active_holds(state: &mut State, inputs: &[bool; 4], current_time: f32) {
    for column in 0..state.active_holds.len() {
        let mut handle_let_go = None;
        let mut handle_success = None;

        {
            let active_opt = &mut state.active_holds[column];
            if let Some(active) = active_opt {
                if inputs[column] {
                    active.last_input_time = current_time;
                    active.is_pressed = true;
                } else {
                    active.is_pressed = false;
                    if !active.let_go
                        && current_time < active.end_time
                        && (current_time - active.last_input_time) > HOLD_DROP_TOLERANCE
                    {
                        active.let_go = true;
                        handle_let_go = Some((column, active.note_index));
                    }
                }

                if current_time >= active.end_time {
                    let note_index = active.note_index;
                    let still_engaged = active.is_engaged(current_time);
                    if still_engaged {
                        handle_success = Some(note_index);
                    } else if !active.let_go {
                        active.let_go = true;
                        handle_let_go = Some((column, note_index));
                    }
                    *active_opt = None;
                }
            }
        }

        if let Some((column, note_index)) = handle_let_go {
            handle_hold_let_go(state, column, note_index);
        }

        if let Some(note_index) = handle_success {
            handle_hold_success(state, note_index);
        }
    }
}

const REGEN_COMBO_AFTER_MISS: u32 = 5;
const MAX_REGEN_COMBO_AFTER_MISS: u32 = 10;
const LIFE_REGEN_AMOUNT: f32 = LifeChange::HELD; // In SM, this is tied to LifePercentChangeHeld
// Simply Love sets TimingWindowSecondsHold to 0.32s, so mirror that grace window.
// Reference: itgmania/Themes/Simply Love/Scripts/SL_Init.lua
const HOLD_DROP_TOLERANCE: f32 = 0.32;

pub struct State {
    // Song & Chart data
    pub song: Arc<SongData>,
    pub background_texture_key: String,
    pub chart: Arc<ChartData>,
    pub timing: Arc<TimingData>,
    pub notes: Vec<Note>,

    // Gameplay state
    pub song_start_instant: Instant, // The wall-clock moment music t=0 begins (after the initial delay).
    pub current_beat: f32,
    pub current_music_time: f32, // Time calculated at the start of each update frame.
    pub note_spawn_cursor: usize, // For spawning visual arrows
    pub judged_row_cursor: usize, // For finalizing row judgments
    pub arrows: [Vec<Arrow>; 4], // Active on-screen arrows per column

    // Scoring & Feedback
    pub combo: u32,
    pub miss_combo: u32,
    pub full_combo_grade: Option<JudgeGrade>,
    pub first_fc_attempt_broken: bool,
    pub judgment_counts: HashMap<JudgeGrade, u32>,
    pub last_judgment: Option<JudgmentRenderInfo>,

    // Life Meter
    pub life: f32,             // 0.0 to 1.0
    pub combo_after_miss: u32, // for regeneration logic
    pub is_failing: bool,
    pub fail_time: Option<f32>,

    // Grade/Percent scoring
    pub earned_grade_points: i32,
    pub possible_grade_points: i32,
    pub song_completed_naturally: bool,

    // Visuals
    pub noteskin: Option<Noteskin>,
    pub active_color_index: i32,
    pub player_color: [f32; 4],
    pub scroll_speed: ScrollSpeedSetting,
    pub scroll_reference_bpm: f32,
    pub scroll_pixels_per_second: f32,
    pub scroll_travel_time: f32,
    pub receptor_glow_timers: [f32; 4], // Timers for glow effect on each receptor
    pub receptor_bop_timers: [f32; 4],  // Timers for the "bop" animation on empty press
    pub tap_explosions: [Option<ActiveTapExplosion>; 4],
    pub active_holds: [Option<ActiveHold>; 4],
    pub holds_total: u32,
    pub holds_held: u32,

    // Animation timing for this screen
    pub total_elapsed_in_screen: f32,

    // Hold-to-exit logic
    pub hold_to_exit_key: Option<KeyCode>,
    pub hold_to_exit_start: Option<Instant>,
    prev_inputs: [bool; 4],
    keyboard_inputs: [bool; 4],

    // Debugging
    log_timer: f32,
}

impl State {
    #[inline(always)]
    fn is_dead(&self) -> bool {
        self.is_failing || self.life <= 0.0
    }

    fn change_life(&mut self, delta: f32) {
        // If we've *ever* died, keep pinned.
        if self.is_dead() {
            self.life = 0.0;
            self.is_failing = true;
            return;
        }

        let mut final_delta = delta;

        if final_delta > 0.0 {
            // regen only when alive
            if self.combo_after_miss < REGEN_COMBO_AFTER_MISS {
                self.combo_after_miss += 1;
            } else {
                final_delta += LIFE_REGEN_AMOUNT;
                self.combo_after_miss = (self.combo_after_miss + 1).min(MAX_REGEN_COMBO_AFTER_MISS);
            }
        } else if final_delta < 0.0 {
            self.combo_after_miss = 0;
        }

        self.life = (self.life + final_delta).clamp(0.0, 1.0);

        if self.life <= 0.0 {
            if !self.is_failing {
                // first frame of failure
                self.fail_time = Some(self.current_music_time);
            }
            self.life = 0.0;
            self.is_failing = true; // latch immediately in the same call
            info!("Player has failed!");
        }
    }
}

// --- INITIALIZATION ---

pub fn init(song: Arc<SongData>, chart: Arc<ChartData>, active_color_index: i32) -> State {
    info!("Initializing Gameplay Screen...");
    info!(
        "Loaded song '{}' and chart '{}'",
        song.title, chart.difficulty
    );

    let style = Style {
        num_cols: 4,
        num_players: 1,
    };
    let noteskin = noteskin::load(Path::new("assets/noteskins/cel/dance-single.txt"), &style)
        .ok()
        .or_else(|| noteskin::load(Path::new("assets/noteskins/fallback.txt"), &style).ok());

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

    let parsed_notes = note_parser::parse_chart_notes(&chart.notes);
    let mut notes: Vec<Note> = Vec::with_capacity(parsed_notes.len());
    for parsed in parsed_notes {
        let Some(beat) = timing.get_beat_for_row(parsed.row_index) else {
            continue;
        };

        let note_type = match parsed.note_type {
            ChartNoteType::Tap => NoteType::Tap,
            ChartNoteType::Hold => NoteType::Hold,
            ChartNoteType::Roll => NoteType::Roll,
        };

        let hold = match (&note_type, parsed.tail_row_index) {
            (NoteType::Hold | NoteType::Roll, Some(tail_row)) => {
                timing.get_beat_for_row(tail_row).map(|end_beat| HoldData {
                    end_row_index: tail_row,
                    end_beat,
                    result: None,
                })
            }
            _ => None,
        };

        notes.push(Note {
            beat,
            column: parsed.column,
            note_type,
            row_index: parsed.row_index,
            result: None,
            hold,
        });
    }

    let holds_total = chart.stats.holds as u32;

    let num_taps_and_holds = notes.len() as u64;
    // Possible grade points are based on taps/hold heads only.
    // The max score is W1 (Fantastic), which has a PercentScoreWeight of 5.
    let possible_grade_points = (num_taps_and_holds * 5) as i32;

    info!("Parsed {} notes from chart data.", notes.len());

    // --- StepMania Timing Logic Implementation ---
    let first_note_beat = notes.first().map_or(0.0, |n| n.beat);
    let first_second = timing.get_time_for_beat(first_note_beat);
    let start_delay = (MIN_SECONDS_TO_STEP - first_second).max(MIN_SECONDS_TO_MUSIC);
    let song_start_instant = Instant::now() + Duration::from_secs_f32(start_delay);

    if let Some(music_path) = &song.music_path {
        info!("Starting music with a preroll delay of {:.2}s", start_delay);
        let cut = audio::Cut {
            start_sec: (-start_delay) as f64,
            length_sec: f64::INFINITY,
            ..Default::default()
        };
        audio::play_music(music_path.clone(), cut, false);
    } else {
        warn!("No music path found for song '{}'", song.title);
    }

    let profile = profile::get();
    let scroll_speed = profile.scroll_speed;
    let initial_bpm = timing.get_bpm_for_beat(first_note_beat);
    let mut reference_bpm = timing.get_capped_max_bpm(Some(M_MOD_HIGH_CAP));
    if !reference_bpm.is_finite() || reference_bpm <= 0.0 {
        reference_bpm = initial_bpm.max(120.0);
    }

    let mut pixels_per_second = scroll_speed.pixels_per_second(initial_bpm, reference_bpm);
    if !pixels_per_second.is_finite() || pixels_per_second <= 0.0 {
        warn!(
            "Scroll speed {} produced non-positive velocity; falling back to default.",
            scroll_speed
        );
        pixels_per_second =
            ScrollSpeedSetting::default().pixels_per_second(initial_bpm, reference_bpm);
    }
    let mut travel_time =
        scroll_speed.travel_time_seconds(screen_height(), initial_bpm, reference_bpm);
    if !travel_time.is_finite() || travel_time <= 0.0 {
        travel_time = screen_height() / pixels_per_second;
    }
    info!(
        "Scroll speed set to {} ({:.2} BPM at start), {:.2} px/s",
        scroll_speed,
        scroll_speed.effective_bpm(initial_bpm, reference_bpm),
        pixels_per_second
    );

    State {
        song,
        chart,
        background_texture_key: "__white".to_string(),
        timing,
        notes,
        song_start_instant,
        current_beat: 0.0,
        current_music_time: -start_delay,
        note_spawn_cursor: 0,
        judged_row_cursor: 0,
        arrows: [vec![], vec![], vec![], vec![]],
        judgment_counts: HashMap::from_iter([
            (JudgeGrade::Fantastic, 0),
            (JudgeGrade::Excellent, 0),
            (JudgeGrade::Great, 0),
            (JudgeGrade::Decent, 0),
            (JudgeGrade::WayOff, 0),
            (JudgeGrade::Miss, 0),
        ]),
        combo: 0,
        miss_combo: 0,
        full_combo_grade: None,
        first_fc_attempt_broken: false,
        last_judgment: None,
        life: 0.5,
        combo_after_miss: MAX_REGEN_COMBO_AFTER_MISS, // Start in a state where regen is active
        is_failing: false,
        fail_time: None,
        earned_grade_points: 0,
        possible_grade_points,
        song_completed_naturally: false,
        noteskin,
        active_color_index,
        player_color: color::decorative_rgba(active_color_index),
        scroll_speed,
        scroll_reference_bpm: reference_bpm,
        scroll_pixels_per_second: pixels_per_second,
        scroll_travel_time: travel_time,
        receptor_glow_timers: [0.0; 4],
        receptor_bop_timers: [0.0; 4],
        tap_explosions: Default::default(),
        active_holds: Default::default(),
        holds_total,
        holds_held: 0,
        total_elapsed_in_screen: 0.0,
        hold_to_exit_key: None,
        hold_to_exit_start: None,
        prev_inputs: [false; 4],
        keyboard_inputs: [false; 4],
        log_timer: 0.0,
    }
}

// --- INPUT HANDLING ---

fn judge_a_tap(state: &mut State, column: usize, current_time: f32) -> bool {
    if let Some(arrow_to_judge) = state.arrows[column].first().cloned() {
        let note_index = arrow_to_judge.note_index;
        let (note_beat, note_row_index) = {
            let note = &state.notes[note_index];
            (note.beat, note.row_index)
        };
        let note_time = state.timing.get_time_for_beat(note_beat);
        let time_error = current_time - note_time;
        let abs_time_error = time_error.abs();

        let fantastic_window = BASE_FANTASTIC_WINDOW + TIMING_WINDOW_ADD;
        let excellent_window = BASE_EXCELLENT_WINDOW + TIMING_WINDOW_ADD;
        let great_window = BASE_GREAT_WINDOW + TIMING_WINDOW_ADD;
        let decent_window = BASE_DECENT_WINDOW + TIMING_WINDOW_ADD;
        let way_off_window = BASE_WAY_OFF_WINDOW + TIMING_WINDOW_ADD;

        if abs_time_error <= way_off_window {
            let grade = if abs_time_error <= fantastic_window {
                JudgeGrade::Fantastic
            } else if abs_time_error <= excellent_window {
                JudgeGrade::Excellent
            } else if abs_time_error <= great_window {
                JudgeGrade::Great
            } else if abs_time_error <= decent_window {
                JudgeGrade::Decent
            } else {
                JudgeGrade::WayOff
            };

            let judgment = Judgment {
                time_error_ms: time_error * 1000.0,
                grade,
                row: note_row_index,
            };

            state.notes[note_index].result = Some(judgment);
            let note_type = state.notes[note_index].note_type.clone();
            let hold_end_time = state.notes[note_index]
                .hold
                .as_ref()
                .map(|hold| state.timing.get_time_for_beat(hold.end_beat));
            info!(
                "JUDGED (pending): Row {}, Col {}, Error: {:.2}ms, Grade: {:?}",
                note_row_index,
                column,
                time_error * 1000.0,
                grade
            );

            state.arrows[column].remove(0);
            state.receptor_glow_timers[column] = RECEPTOR_GLOW_DURATION;
            trigger_tap_explosion(state, column, grade);

            if matches!(note_type, NoteType::Hold | NoteType::Roll) {
                if let Some(end_time) = hold_end_time {
                    state.active_holds[column] = Some(ActiveHold {
                        note_index,
                        end_time,
                        note_type,
                        let_go: false,
                        last_input_time: current_time,
                        is_pressed: true,
                    });
                }
            }

            return true;
        }
    }
    false
}

pub fn handle_key_press(state: &mut State, event: &KeyEvent) -> ScreenAction {
    if let PhysicalKey::Code(key_code) = event.physical_key {
        if event.state == ElementState::Pressed && event.repeat {
            return ScreenAction::None;
        }

        let column = match key_code {
            KeyCode::ArrowLeft | KeyCode::KeyD => Some(0),
            KeyCode::ArrowDown | KeyCode::KeyF => Some(1),
            KeyCode::ArrowUp | KeyCode::KeyJ => Some(2),
            KeyCode::ArrowRight | KeyCode::KeyK => Some(3),
            _ => None,
        };

        if let Some(col_idx) = column {
            state.keyboard_inputs[col_idx] = event.state == ElementState::Pressed;
        }

        match event.state {
            ElementState::Pressed => {
                if key_code == KeyCode::Escape || key_code == KeyCode::Enter {
                    state.hold_to_exit_key = Some(key_code);
                    state.hold_to_exit_start = Some(Instant::now());
                    return ScreenAction::None;
                }
            }
            ElementState::Released => {
                if state.hold_to_exit_key == Some(key_code) {
                    state.hold_to_exit_key = None;
                    state.hold_to_exit_start = None;
                }
            }
        }
    }
    ScreenAction::None
}

fn finalize_row_judgment(state: &mut State, judgments_in_row: Vec<Judgment>) {
    if judgments_in_row.is_empty() {
        return;
    }

    // If the player is not dead, update the score points.
    if !state.is_dead() {
        for judgment in &judgments_in_row {
            // Update Grade Points (for percentage display) using PercentScoreWeight values.
            let grade_points = match judgment.grade {
                JudgeGrade::Fantastic => 5,
                JudgeGrade::Excellent => 4,
                JudgeGrade::Great => 2,
                JudgeGrade::Decent => 0,
                JudgeGrade::WayOff => -6,
                JudgeGrade::Miss => -12,
            };
            state.earned_grade_points += grade_points;
        }
    }

    // Select the representative judgment for the row (ITG logic)
    let mut representative_judgment = None;
    let mut has_miss = false;
    let mut latest_offset = f32::NEG_INFINITY;

    for judgment in judgments_in_row {
        if judgment.grade == JudgeGrade::Miss {
            representative_judgment = Some(judgment.clone());
            has_miss = true;
            break;
        }
        if judgment.time_error_ms > latest_offset {
            latest_offset = judgment.time_error_ms;
            representative_judgment = Some(judgment.clone());
        }
    }

    let Some(final_judgment) = representative_judgment else {
        return;
    };
    let final_grade = final_judgment.grade;

    info!(
        "FINALIZED: Row {}, Grade: {:?}, Offset: {:.2}ms",
        final_judgment.row, final_grade, final_judgment.time_error_ms
    );

    // Change life based on judgment
    let life_delta = match final_grade {
        JudgeGrade::Fantastic => LifeChange::FANTASTIC,
        JudgeGrade::Excellent => LifeChange::EXCELLENT,
        JudgeGrade::Great => LifeChange::GREAT,
        JudgeGrade::Decent => LifeChange::DECENT,
        JudgeGrade::WayOff => LifeChange::WAY_OFF,
        JudgeGrade::Miss => LifeChange::MISS,
    };
    state.change_life(life_delta);

    // Update global state based on this single representative judgment
    state.last_judgment = Some(JudgmentRenderInfo {
        judgment: final_judgment,
        judged_at: Instant::now(),
    });
    *state.judgment_counts.entry(final_grade).or_insert(0) += 1;

    state.miss_combo = 0;

    if has_miss || matches!(final_grade, JudgeGrade::WayOff) {
        state.combo = 0;
        if state.full_combo_grade.is_some() {
            state.first_fc_attempt_broken = true;
        }
        state.full_combo_grade = None;
    } else {
        // Don't increase combo if dead
        if !state.is_dead() {
            state.combo += 1;
        }

        if !state.first_fc_attempt_broken {
            let new_grade = if let Some(current_fc_grade) = &state.full_combo_grade {
                final_grade.max(*current_fc_grade)
            } else {
                final_grade
            };
            state.full_combo_grade = Some(new_grade);
        }
    }
}

fn update_judged_rows(state: &mut State) {
    loop {
        let max_row_index = state.notes.iter().map(|n| n.row_index).max().unwrap_or(0);

        if state.judged_row_cursor > max_row_index {
            break;
        }

        let is_row_complete = {
            let notes_on_row: Vec<&Note> = state
                .notes
                .iter()
                .filter(|n| n.row_index == state.judged_row_cursor)
                .collect();
            notes_on_row.is_empty() || notes_on_row.iter().all(|n| n.result.is_some())
        };

        if is_row_complete {
            let judgments_on_row: Vec<Judgment> = state
                .notes
                .iter()
                .filter(|n| n.row_index == state.judged_row_cursor)
                .filter_map(|n| n.result.clone())
                .collect();

            finalize_row_judgment(state, judgments_on_row);
            state.judged_row_cursor += 1;
        } else {
            break;
        }
    }
}

fn get_music_end_time(state: &State) -> f32 {
    let last_note_beat = state.notes.last().map_or(0.0, |n| n.beat);
    let last_step_seconds = state.timing.get_time_for_beat(last_note_beat);
    let last_hittable_second = last_step_seconds + (BASE_WAY_OFF_WINDOW + TIMING_WINDOW_ADD);
    last_hittable_second + TRANSITION_OUT_DURATION
}

// --- UPDATE LOOP ---

#[inline(always)]
pub fn update(state: &mut State, input: &InputState, delta_time: f32) -> ScreenAction {
    if let (Some(key), Some(start_time)) = (state.hold_to_exit_key, state.hold_to_exit_start) {
        if start_time.elapsed() >= std::time::Duration::from_secs(1) {
            state.hold_to_exit_key = None;
            state.hold_to_exit_start = None;
            // IMPORTANT: Quitting via hold-to-exit does NOT set song_completed_naturally to true.
            return match key {
                winit::keyboard::KeyCode::Enter => ScreenAction::Navigate(Screen::Evaluation),
                winit::keyboard::KeyCode::Escape => ScreenAction::Navigate(Screen::SelectMusic),
                _ => ScreenAction::None,
            };
        }
    }

    state.total_elapsed_in_screen += delta_time;

    let now = std::time::Instant::now();
    let music_time_sec = if now < state.song_start_instant {
        -(state
            .song_start_instant
            .saturating_duration_since(now)
            .as_secs_f32())
    } else {
        now.saturating_duration_since(state.song_start_instant)
            .as_secs_f32()
    };
    state.current_music_time = music_time_sec;
    state.current_beat = state.timing.get_beat_for_time(music_time_sec);

    let current_bpm = state.timing.get_bpm_for_beat(state.current_beat);
    let mut dynamic_speed = state
        .scroll_speed
        .pixels_per_second(current_bpm, state.scroll_reference_bpm);
    if !dynamic_speed.is_finite() || dynamic_speed <= 0.0 {
        dynamic_speed = ScrollSpeedSetting::default()
            .pixels_per_second(current_bpm, state.scroll_reference_bpm);
    }
    state.scroll_pixels_per_second = dynamic_speed;

    let mut travel_time = state.scroll_speed.travel_time_seconds(
        screen_height(),
        current_bpm,
        state.scroll_reference_bpm,
    );
    if !travel_time.is_finite() || travel_time <= 0.0 {
        travel_time = screen_height() / dynamic_speed;
    }
    state.scroll_travel_time = travel_time;

    if state.current_music_time >= get_music_end_time(state) {
        info!("Music end time reached. Transitioning to evaluation.");
        state.song_completed_naturally = true;
        return ScreenAction::Navigate(Screen::Evaluation);
    }

    let current_inputs = [
        input.left || state.keyboard_inputs[0],
        input.down || state.keyboard_inputs[1],
        input.up || state.keyboard_inputs[2],
        input.right || state.keyboard_inputs[3],
    ];
    let prev_inputs = state.prev_inputs;

    for (col, (now_down, was_down)) in current_inputs.iter().copied().zip(prev_inputs).enumerate() {
        if now_down && !was_down {
            if !judge_a_tap(state, col, music_time_sec) {
                state.receptor_bop_timers[col] = 0.11;
            }
        }
    }
    state.prev_inputs = current_inputs;

    update_active_holds(state, &current_inputs, music_time_sec);

    for timer in &mut state.receptor_glow_timers {
        *timer = (*timer - delta_time).max(0.0);
    }
    for timer in &mut state.receptor_bop_timers {
        *timer = (*timer - delta_time).max(0.0);
    }
    for explosion in &mut state.tap_explosions {
        if let Some(active) = explosion {
            active.elapsed += delta_time;
            let lifetime = state
                .noteskin
                .as_ref()
                .and_then(|ns| ns.tap_explosions.get(&active.window))
                .map(|explosion| explosion.animation.duration())
                .unwrap_or(0.0);

            if lifetime <= 0.0 || active.elapsed >= lifetime {
                *explosion = None;
            }
        }
    }

    let lookahead_time = music_time_sec + state.scroll_travel_time;
    let lookahead_beat = state.timing.get_beat_for_time(lookahead_time);
    while state.note_spawn_cursor < state.notes.len()
        && state.notes[state.note_spawn_cursor].beat < lookahead_beat
    {
        let note = &state.notes[state.note_spawn_cursor];
        state.arrows[note.column].push(Arrow {
            beat: note.beat,
            column: note.column,
            note_type: note.note_type.clone(),
            note_index: state.note_spawn_cursor,
        });
        state.note_spawn_cursor += 1;
    }

    let way_off_window = BASE_WAY_OFF_WINDOW + TIMING_WINDOW_ADD;
    for col_arrows in &mut state.arrows {
        let mut missed = false;
        if let Some(arrow) = col_arrows.first().cloned() {
            let note_index = arrow.note_index;
            let (note_row_index, note_result_is_none, note_beat) = {
                let note = &state.notes[note_index];
                (note.row_index, note.result.is_none(), note.beat)
            };

            let note_time = state.timing.get_time_for_beat(note_beat);
            if music_time_sec - note_time > way_off_window && note_result_is_none {
                let judgment = Judgment {
                    time_error_ms: ((music_time_sec - note_time) * 1000.0),
                    grade: JudgeGrade::Miss,
                    row: note_row_index,
                };
                state.notes[note_index].result = Some(judgment);
                info!(
                    "MISSED (pending): Row {}, Col {}, Beat {:.2}",
                    note_row_index, arrow.column, arrow.beat
                );
                missed = true;
            }
        }
        if missed {
            col_arrows.remove(0);
        }
    }

    update_judged_rows(state);

    state.log_timer += delta_time;
    if state.log_timer >= 1.0 {
        let active_arrows: usize = state.arrows.iter().map(|v| v.len()).sum();
        log::info!(
            "Beat: {:.2}, Time: {:.2}, Combo: {}, Misses: {}, Active Arrows: {}",
            state.current_beat,
            music_time_sec,
            state.combo,
            state.miss_combo,
            active_arrows
        );
        state.log_timer -= 1.0;
    }

    ScreenAction::None
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

// --- DRAWING ---

fn build_background(state: &State) -> Actor {
    let sw = screen_width();
    let sh = screen_height();
    let screen_aspect = if sh > 0.0 { sw / sh } else { 16.0 / 9.0 };

    let (tex_w, tex_h) =
        if let Some(meta) = crate::assets::texture_dims(&state.background_texture_key) {
            (meta.w as f32, meta.h as f32)
        } else {
            (1.0, 1.0) // fallback, will just fill screen
        };

    let tex_aspect = if tex_h > 0.0 { tex_w / tex_h } else { 1.0 };

    if screen_aspect > tex_aspect {
        // screen is wider, match width to cover
        act!(sprite(state.background_texture_key.clone()):
            align(0.5, 0.5): xy(screen_center_x(), screen_center_y()):
            zoomtowidth(sw):
            z(-100)
        )
    } else {
        // screen is taller/equal, match height to cover
        act!(sprite(state.background_texture_key.clone()):
            align(0.5, 0.5): xy(screen_center_x(), screen_center_y()):
            zoomtoheight(sh):
            z(-100)
        )
    }
}

// --- Statics for Judgment Counter Display ---

static JUDGMENT_ORDER: [JudgeGrade; 6] = [
    JudgeGrade::Fantastic,
    JudgeGrade::Excellent,
    JudgeGrade::Great,
    JudgeGrade::Decent,
    JudgeGrade::WayOff,
    JudgeGrade::Miss,
];

struct JudgmentDisplayInfo {
    label: &'static str,
    color: [f32; 4],
}

static JUDGMENT_INFO: LazyLock<HashMap<JudgeGrade, JudgmentDisplayInfo>> = LazyLock::new(|| {
    HashMap::from([
        (
            JudgeGrade::Fantastic,
            JudgmentDisplayInfo {
                label: "FANTASTIC",
                color: color::rgba_hex(color::JUDGMENT_HEX[0]),
            },
        ),
        (
            JudgeGrade::Excellent,
            JudgmentDisplayInfo {
                label: "EXCELLENT",
                color: color::rgba_hex(color::JUDGMENT_HEX[1]),
            },
        ),
        (
            JudgeGrade::Great,
            JudgmentDisplayInfo {
                label: "GREAT",
                color: color::rgba_hex(color::JUDGMENT_HEX[2]),
            },
        ),
        (
            JudgeGrade::Decent,
            JudgmentDisplayInfo {
                label: "DECENT",
                color: color::rgba_hex(color::JUDGMENT_HEX[3]),
            },
        ),
        (
            JudgeGrade::WayOff,
            JudgmentDisplayInfo {
                label: "WAY OFF",
                color: color::rgba_hex(color::JUDGMENT_HEX[4]),
            },
        ),
        (
            JudgeGrade::Miss,
            JudgmentDisplayInfo {
                label: "MISS",
                color: color::rgba_hex(color::JUDGMENT_HEX[5]),
            },
        ),
    ])
});

fn format_game_time(s: f32, total_seconds: f32) -> String {
    if s < 0.0 {
        return format_game_time(0.0, total_seconds);
    }
    let s_u64 = s as u64;

    let minutes = s_u64 / 60;
    let seconds = s_u64 % 60;

    if total_seconds >= 3600.0 {
        // Over an hour total? use H:MM:SS
        let hours = s_u64 / 3600;
        let minutes = (s_u64 % 3600) / 60;
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else if total_seconds >= 600.0 {
        // Over 10 mins total? use MM:SS
        format!("{:02}:{:02}", minutes, seconds)
    } else {
        // Under 10 mins total? use M:SS
        format!("{}:{:02}", minutes, seconds)
    }
}

pub fn get_actors(state: &State, asset_manager: &AssetManager) -> Vec<Actor> {
    let mut actors = Vec::new();
    let profile = profile::get();

    // --- Background and Filter ---
    actors.push(build_background(state));

    let filter_alpha = match profile.background_filter {
        crate::gameplay::profile::BackgroundFilter::Off => 0.0,
        crate::gameplay::profile::BackgroundFilter::Dark => 0.5,
        crate::gameplay::profile::BackgroundFilter::Darker => 0.75,
        crate::gameplay::profile::BackgroundFilter::Darkest => 0.95,
    };

    if filter_alpha > 0.0 {
        actors.push(act!(quad:
            align(0.5, 0.5): xy(screen_center_x(), screen_center_y()):
            zoomto(screen_width(), screen_height()):
            diffuse(0.0, 0.0, 0.0, filter_alpha):
            z(-99) // Draw just above the background
        ));
    }

    // --- Playfield Positioning (1:1 with Simply Love) ---
    let logical_screen_width = screen_width();
    let clamped_width = logical_screen_width.clamp(640.0, 854.0);
    let playfield_center_x = screen_center_x() - (clamped_width * 0.25);

    let receptor_y = screen_center_y() + RECEPTOR_Y_OFFSET_FROM_CENTER;
    let pixels_per_second = state.scroll_pixels_per_second;

    // --- Banner (1:1 with Simply Love, including parent frame logic) ---
    if let Some(banner_path) = &state.song.banner_path {
        let banner_key = banner_path.to_string_lossy().into_owned();
        let wide = is_wide();

        let sidepane_center_x = screen_width() * 0.75;
        let sidepane_center_y = screen_center_y() + 80.0;
        let note_field_is_centered = (playfield_center_x - screen_center_x()).abs() < 1.0;
        let is_ultrawide = screen_width() / screen_height() > (21.0 / 9.0);
        let banner_data_zoom = if note_field_is_centered && wide && !is_ultrawide {
            let ar = screen_width() / screen_height();
            let t = ((ar - (16.0 / 10.0)) / ((16.0 / 9.0) - (16.0 / 10.0))).clamp(0.0, 1.0);
            0.825 + (0.925 - 0.825) * t
        } else {
            1.0
        };
        let mut local_banner_x = 70.0;
        if note_field_is_centered && wide {
            local_banner_x = 72.0;
        }
        let local_banner_y = -200.0;

        let banner_x = sidepane_center_x + (local_banner_x * banner_data_zoom);
        let banner_y = sidepane_center_y + (local_banner_y * banner_data_zoom);
        let final_zoom = 0.4 * banner_data_zoom;

        actors.push(act!(sprite(banner_key):
            align(0.5, 0.5): xy(banner_x, banner_y):
            setsize(418.0, 164.0): zoom(final_zoom):
            z(-50)
        ));
    }

    if let Some(ns) = &state.noteskin {
        let scale_sprite = |size: [i32; 2]| -> [f32; 2] {
            let width = size[0].max(0) as f32;
            let height = size[1].max(0) as f32;
            if height <= 0.0 || TARGET_ARROW_PIXEL_SIZE <= 0.0 {
                [width, height]
            } else {
                let scale = TARGET_ARROW_PIXEL_SIZE / height;
                [width * scale, TARGET_ARROW_PIXEL_SIZE]
            }
        };
        let scale_explosion = |size: [i32; 2]| -> [f32; 2] {
            let width = size[0].max(0) as f32;
            let height = size[1].max(0) as f32;
            if height <= 0.0 || TARGET_EXPLOSION_PIXEL_SIZE <= 0.0 {
                [width, height]
            } else {
                let scale = TARGET_EXPLOSION_PIXEL_SIZE / height;
                [width * scale, TARGET_EXPLOSION_PIXEL_SIZE]
            }
        };
        let current_time = state.current_music_time;
        let compute_lane_y = |beat: f32| -> f32 {
            match state.scroll_speed {
                ScrollSpeedSetting::XMod(_) | ScrollSpeedSetting::MMod(_) => {
                    let beat_diff = beat - state.current_beat;
                    let multiplier = state
                        .scroll_speed
                        .beat_multiplier(state.scroll_reference_bpm);
                    receptor_y + (beat_diff * ScrollSpeedSetting::ARROW_SPACING * multiplier)
                }
                _ => {
                    let note_time = state.timing.get_time_for_beat(beat);
                    let time_diff = note_time - current_time;
                    receptor_y + (time_diff * pixels_per_second)
                }
            }
        };

        // Receptors + glow
        for i in 0..4 {
            let col_x_offset = ns.column_xs[i];

            let bop_timer = state.receptor_bop_timers[i];
            let bop_zoom = if bop_timer > 0.0 {
                let t = (0.11 - bop_timer) / 0.11;
                0.75 + (1.0 - 0.75) * t
            } else {
                1.0
            };

            let receptor_slot = &ns.receptor_off[i];
            let receptor_frame =
                receptor_slot.frame_index(state.total_elapsed_in_screen, state.current_beat);
            let receptor_uv = receptor_slot.uv_for_frame(receptor_frame);
            let receptor_size = scale_sprite(receptor_slot.size());
            let receptor_color = ns.receptor_pulse.color_for_beat(state.current_beat);
            actors.push(act!(sprite(receptor_slot.texture_key().to_string()):
                align(0.5, 0.5):
                xy(playfield_center_x + col_x_offset as f32, receptor_y):
                zoomto(receptor_size[0] as f32, receptor_size[1] as f32):
                zoom(bop_zoom):
                diffuse(
                    receptor_color[0],
                    receptor_color[1],
                    receptor_color[2],
                    receptor_color[3]
                ):
                rotationz(-receptor_slot.def.rotation_deg as f32):
                customtexturerect(
                    receptor_uv[0],
                    receptor_uv[1],
                    receptor_uv[2],
                    receptor_uv[3]
                ):
                z(100)
            ));

            if let Some(hold_slot) = state.active_holds[i]
                .as_ref()
                .filter(|active| active.is_engaged(current_time))
                .and_then(|active| {
                    let note_type = &state.notes[active.note_index].note_type;
                    let visuals = if matches!(note_type, NoteType::Roll) {
                        &ns.roll
                    } else {
                        &ns.hold
                    };
                    visuals
                        .explosion
                        .as_ref()
                        .or_else(|| ns.hold.explosion.as_ref())
                })
            {
                let hold_uv = hold_slot.uv_for_frame(0);
                let hold_size = scale_explosion(hold_slot.size());
                let receptor_rotation = ns
                    .receptor_off
                    .get(i)
                    .map(|slot| slot.def.rotation_deg as f32)
                    .unwrap_or(0.0);
                let base_rotation = hold_slot.def.rotation_deg as f32;
                let final_rotation = base_rotation + receptor_rotation;
                actors.push(act!(sprite(hold_slot.texture_key().to_string()):
                    align(0.5, 0.5):
                    xy(playfield_center_x + col_x_offset as f32, receptor_y):
                    zoomto(hold_size[0], hold_size[1]):
                    rotationz(-final_rotation):
                    customtexturerect(hold_uv[0], hold_uv[1], hold_uv[2], hold_uv[3]):
                    blend(normal):
                    z(101)
                ));
            }

            let glow_timer = state.receptor_glow_timers[i];
            if glow_timer > 0.0 {
                if let Some(glow_slot) = ns.receptor_glow.get(i).and_then(|slot| slot.as_ref()) {
                    let glow_frame =
                        glow_slot.frame_index(state.total_elapsed_in_screen, state.current_beat);
                    let glow_uv = glow_slot.uv_for_frame(glow_frame);
                    let glow_size = glow_slot.size();
                    let alpha = (glow_timer / RECEPTOR_GLOW_DURATION).powf(0.75);
                    actors.push(act!(sprite(glow_slot.texture_key().to_string()):
                        align(0.5, 0.5):
                        xy(playfield_center_x + col_x_offset as f32, receptor_y):
                        zoomto(glow_size[0] as f32, glow_size[1] as f32):
                        rotationz(-glow_slot.def.rotation_deg as f32):
                        customtexturerect(glow_uv[0], glow_uv[1], glow_uv[2], glow_uv[3]):
                        diffuse(1.0, 1.0, 1.0, alpha):
                        blend(add):
                        z(102)
                    ));
                }
            }
        }

        // Tap explosions
        for i in 0..4 {
            if let Some(active) = state.tap_explosions[i].as_ref() {
                if let Some(explosion) = ns.tap_explosions.get(&active.window) {
                    let col_x_offset = ns.column_xs[i];
                    let anim_time = active.elapsed;
                    let slot = &explosion.slot;
                    let beat_for_anim = if slot.source.is_beat_based() {
                        (state.current_beat - active.start_beat).max(0.0)
                    } else {
                        state.current_beat
                    };
                    let frame = slot.frame_index(anim_time, beat_for_anim);
                    let uv = slot.uv_for_frame(frame);
                    let size = scale_explosion(slot.size());
                    let visual = explosion.animation.state_at(active.elapsed);
                    let rotation_deg = ns
                        .receptor_off
                        .get(i)
                        .map(|slot| slot.def.rotation_deg)
                        .unwrap_or(0);

                    actors.push(act!(sprite(slot.texture_key().to_string()):
                        align(0.5, 0.5):
                        xy(playfield_center_x + col_x_offset as f32, receptor_y):
                        zoomto(size[0], size[1]):
                        zoom(visual.zoom):
                        customtexturerect(uv[0], uv[1], uv[2], uv[3]):
                        diffuse(
                            visual.diffuse[0],
                            visual.diffuse[1],
                            visual.diffuse[2],
                            visual.diffuse[3]
                        ):
                        rotationz(-(rotation_deg as f32)):
                        blend(normal):
                        z(101)
                    ));

                    let glow = visual.glow;
                    let glow_strength =
                        glow[0].abs() + glow[1].abs() + glow[2].abs() + glow[3].abs();
                    if glow_strength > f32::EPSILON {
                        actors.push(act!(sprite(slot.texture_key().to_string()):
                            align(0.5, 0.5):
                            xy(playfield_center_x + col_x_offset as f32, receptor_y):
                            zoomto(size[0], size[1]):
                            zoom(visual.zoom):
                            customtexturerect(uv[0], uv[1], uv[2], uv[3]):
                            diffuse(glow[0], glow[1], glow[2], glow[3]):
                            rotationz(-(rotation_deg as f32)):
                            blend(add):
                            z(101)
                        ));
                    }
                }
            }
        }

        for (note_index, note) in state.notes.iter().enumerate() {
            if !matches!(note.note_type, NoteType::Hold | NoteType::Roll) {
                continue;
            }
            let Some(hold) = &note.hold else {
                continue;
            };

            if matches!(hold.result, Some(HoldResult::Held)) {
                continue;
            }

            let head_y = compute_lane_y(note.beat);
            let tail_y = compute_lane_y(hold.end_beat);
            let head_is_top = head_y <= tail_y;
            let mut top = head_y.min(tail_y);
            let mut bottom = head_y.max(tail_y);
            if bottom < -200.0 || top > screen_height() + 200.0 {
                continue;
            }
            top = top.max(-400.0);
            bottom = bottom.min(screen_height() + 400.0);
            if bottom <= top {
                continue;
            }

            let col_x_offset = ns.column_xs[note.column];
            let active_state = state.active_holds[note.column]
                .as_ref()
                .filter(|h| h.note_index == note_index);
            let engaged = active_state
                .map(|h| h.is_engaged(current_time))
                .unwrap_or(false);
            let use_active = active_state
                .map(|h| h.is_pressed && !h.let_go)
                .unwrap_or(false);

            if engaged {
                if head_is_top {
                    top = top.max(receptor_y);
                } else {
                    bottom = bottom.min(receptor_y);
                }
            }

            if bottom <= top {
                continue;
            }

            let center_y = (top + bottom) * 0.5;
            let body_height = bottom - top;

            let visuals = if matches!(note.note_type, NoteType::Roll) {
                &ns.roll
            } else {
                &ns.hold
            };

            if let Some(body_slot) = if use_active {
                visuals
                    .body_active
                    .as_ref()
                    .or_else(|| visuals.body_inactive.as_ref())
            } else {
                visuals
                    .body_inactive
                    .as_ref()
                    .or_else(|| visuals.body_active.as_ref())
            } {
                let body_width = TARGET_ARROW_PIXEL_SIZE;
                actors.push(act!(sprite(body_slot.texture_key().to_string()):
                    align(0.5, 0.5):
                    xy(playfield_center_x + col_x_offset as f32, center_y):
                    zoomto(body_width, body_height):
                    z(100)
                ));
            }

            let tail_slot = if use_active {
                visuals
                    .bottomcap_active
                    .as_ref()
                    .or_else(|| visuals.bottomcap_inactive.as_ref())
            } else {
                visuals
                    .bottomcap_inactive
                    .as_ref()
                    .or_else(|| visuals.bottomcap_active.as_ref())
            };
            if let Some(cap_slot) = tail_slot {
                let tail_position = tail_y;
                if tail_position > -400.0 && tail_position < screen_height() + 400.0 {
                    let cap_size = scale_sprite(cap_slot.size());
                    actors.push(act!(sprite(cap_slot.texture_key().to_string()):
                        align(0.5, 0.5):
                        xy(playfield_center_x + col_x_offset as f32, tail_position):
                        zoomto(cap_size[0], cap_size[1]):
                        z(101)
                    ));
                }
            }
        }

        // Active arrows
        for column_arrows in &state.arrows {
            for arrow in column_arrows {
                let arrow_time = state.timing.get_time_for_beat(arrow.beat);
                let time_diff = arrow_time - current_time;
                let y_pos = match state.scroll_speed {
                    ScrollSpeedSetting::XMod(_) | ScrollSpeedSetting::MMod(_) => {
                        let beat_diff = arrow.beat - state.current_beat;
                        let multiplier = state
                            .scroll_speed
                            .beat_multiplier(state.scroll_reference_bpm);
                        receptor_y + (beat_diff * ScrollSpeedSetting::ARROW_SPACING * multiplier)
                    }
                    _ => receptor_y + (time_diff * pixels_per_second),
                };

                if y_pos < -100.0 || y_pos > screen_height() + 100.0 {
                    continue;
                }

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

                let note_idx = (arrow.column % 4) * NUM_QUANTIZATIONS + quantization as usize;
                if let Some(note_slot) = ns.notes.get(note_idx) {
                    let note_frame =
                        note_slot.frame_index(state.total_elapsed_in_screen, state.current_beat);
                    let note_uv = note_slot.uv_for_frame(note_frame);
                    let note_size = scale_sprite(note_slot.size());

                    actors.push(act!(sprite(note_slot.texture_key().to_string()):
                        align(0.5, 0.5):
                        xy(playfield_center_x + col_x_offset as f32, y_pos):
                        zoomto(note_size[0] as f32, note_size[1] as f32):
                        rotationz(-note_slot.def.rotation_deg as f32):
                        customtexturerect(note_uv[0], note_uv[1], note_uv[2], note_uv[3]):
                        z(101)
                    ));
                }
            }
        }
    }

    // Combo
    if state.miss_combo >= SHOW_COMBO_AT {
        actors.push(act!(text:
            font("wendy_combo"): settext(state.miss_combo.to_string()):
            align(0.5, 0.5): xy(playfield_center_x, screen_center_y() + 30.0):
            zoom(0.75): horizalign(center):
            diffuse(1.0, 0.0, 0.0, 1.0):
            z(90)
        ));
    } else if state.combo >= SHOW_COMBO_AT {
        let (color1, color2) = if let Some(fc_grade) = &state.full_combo_grade {
            match fc_grade {
                JudgeGrade::Fantastic => (color::rgba_hex("#C8FFFF"), color::rgba_hex("#6BF0FF")),
                JudgeGrade::Excellent => (color::rgba_hex("#FDFFC9"), color::rgba_hex("#FDDB85")),
                JudgeGrade::Great => (color::rgba_hex("#C9FFC9"), color::rgba_hex("#94FEC1")),
                _ => ([1.0, 1.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0]),
            }
        } else {
            ([1.0, 1.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0])
        };

        let effect_period = 0.8;
        let t = (state.total_elapsed_in_screen / effect_period).fract();
        let anim_t = ((t * 2.0 * std::f32::consts::PI).sin() + 1.0) / 2.0;

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
            z(90)
        ));
    }

    // Judgment Sprite (Love)
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
            let mut frame_base = judgment.grade as usize;
            if judgment.grade >= JudgeGrade::Excellent {
                frame_base += 1;
            }
            let frame_offset = if offset_sec < 0.0 { 0 } else { 1 };
            let linear_index = (frame_base * 2 + frame_offset) as u32;

            actors.push(act!(sprite("judgements/Love 2x7 (doubleres).png"):
                align(0.5, 0.5): xy(playfield_center_x, screen_center_y() - 30.0):
                z(200): zoomtoheight(64.0): setstate(linear_index): zoom(zoom)
            ));
        }
    }

    // Difficulty Box
    let x = screen_center_x() - widescale(292.5, 342.5);
    let y = 56.0;

    let difficulty_index = color::FILE_DIFFICULTY_NAMES
        .iter()
        .position(|&name| name.eq_ignore_ascii_case(&state.chart.difficulty))
        .unwrap_or(2);
    let difficulty_color_index = state.active_color_index - (4 - difficulty_index) as i32;
    let difficulty_color = color::simply_love_rgba(difficulty_color_index);
    let meter_text = state.chart.meter.to_string();

    actors.push(Actor::Frame {
        align: [0.5, 0.5],
        offset: [x, y],
        size: [SizeSpec::Px(0.0), SizeSpec::Px(0.0)],
        children: vec![
            act!(quad:
                align(0.5, 0.5): xy(0.0, 0.0): zoomto(30.0, 30.0):
                diffuse(difficulty_color[0], difficulty_color[1], difficulty_color[2], 1.0)
            ),
            act!(text:
                font("wendy"): settext(meter_text): align(0.5, 0.5): xy(0.0, 0.0):
                zoom(0.4): diffuse(0.0, 0.0, 0.0, 1.0)
            ),
        ],
        background: None,
        z: 90,
    });

    // Score Display (P1)
    let clamped_width = screen_width().clamp(640.0, 854.0);
    let score_x = screen_center_x() - clamped_width / 4.3;
    let score_y = 56.0;

    let score_percent = if state.possible_grade_points > 0 {
        (state.earned_grade_points as f32 / state.possible_grade_points as f32).max(0.0) * 100.0
    } else {
        0.0
    };
    let percent_text = format!("{:.2}", score_percent);

    actors.push(act!(text:
        font("wendy_monospace_numbers"): settext(percent_text):
        align(1.0, 1.0): xy(score_x, score_y):
        zoom(0.5): horizalign(right): z(90)
    ));

    // Current BPM Display (1:1 with Simply Love)
    {
        let bpm_value = state.timing.get_bpm_for_beat(state.current_beat);
        let bpm_display = if bpm_value.is_finite() {
            bpm_value.round() as i32
        } else {
            0
        };

        let bpm_text = bpm_display.to_string();

        // Final world-space positions derived from analyzing the SM Lua transforms.
        // The parent frame is bottom-aligned to y=52, and its children are positioned
        // relative to that y-coordinate, with a zoom of 1.33 applied to the whole group.
        let frame_origin_y = 51.0;
        let frame_zoom = 1.33;

        // The BPM text is at y=0 relative to the frame's origin. Its final position is just the origin.
        let bpm_center_y = frame_origin_y;
        // The Rate text is at y=12 relative to the frame's origin. Its offset is scaled by the frame's zoom.
        let rate_center_y = frame_origin_y + (12.0 * frame_zoom);

        let bpm_final_zoom = 1.0 * frame_zoom;
        let rate_final_zoom = 0.5 * frame_zoom;

        let bpm_x = screen_center_x();

        actors.push(act!(text:
            font("miso"): settext(bpm_text):
            align(0.5, 0.5): xy(bpm_x, bpm_center_y):
            zoom(bpm_final_zoom): horizalign(center): z(90)
        ));

        let music_rate = 1.0_f32; // Placeholder until dynamic music rate support exists
        let rate_text = if (music_rate - 1.0).abs() > 0.001 {
            format!("{music_rate:.2}x rate")
        } else {
            String::new()
        };

        actors.push(act!(text:
            font("miso"): settext(rate_text):
            align(0.5, 0.5): xy(bpm_x, rate_center_y):
            zoom(rate_final_zoom): horizalign(center): z(90)
        ));
    }

    // Song Title Box (SongMeter)
    {
        let w = widescale(310.0, 417.0);
        let h = 22.0;
        let box_cx = screen_center_x();
        let box_cy = 20.0;
        let mut frame_children = Vec::new();

        frame_children.push(act!(quad: align(0.5, 0.5): xy(w / 2.0, h / 2.0): zoomto(w, h): diffuse(1.0, 1.0, 1.0, 1.0): z(0) ));
        frame_children.push(act!(quad: align(0.5, 0.5): xy(w / 2.0, h / 2.0): zoomto(w - 4.0, h - 4.0): diffuse(0.0, 0.0, 0.0, 1.0): z(1) ));

        if state.song.total_length_seconds > 0 && state.current_music_time >= 0.0 {
            let progress =
                (state.current_music_time / state.song.total_length_seconds as f32).clamp(0.0, 1.0);
            frame_children.push(act!(quad:
                align(0.0, 0.5): xy(2.0, h / 2.0): zoomto((w - 4.0) * progress, h - 4.0):
                diffuse(state.player_color[0], state.player_color[1], state.player_color[2], 1.0): z(2)
            ));
        }

        let full_title = if state.song.subtitle.trim().is_empty() {
            state.song.title.clone()
        } else {
            format!("{} {}", state.song.title, state.song.subtitle)
        };
        frame_children.push(act!(text:
            font("miso"): settext(full_title): align(0.5, 0.5): xy(w / 2.0, h / 2.0):
            zoom(0.8): maxwidth(screen_width() / 2.5 - 10.0): horizalign(center): z(3)
        ));

        actors.push(Actor::Frame {
            align: [0.5, 0.5],
            offset: [box_cx, box_cy],
            size: [SizeSpec::Px(w), SizeSpec::Px(h)],
            background: None,
            z: 90,
            children: frame_children,
        });
    }

    // --- Life Meter (P1) ---  (drop-in replacement for the current block)
    {
        let w = 136.0;
        let h = 18.0;
        let meter_cx = screen_center_x() - widescale(238.0, 288.0);
        let meter_cy = 20.0;

        // Frames/border
        actors.push(act!(quad: align(0.5, 0.5): xy(meter_cx, meter_cy): zoomto(w + 4.0, h + 4.0): diffuse(1.0, 1.0, 1.0, 1.0): z(90) ));
        actors.push(act!(quad: align(0.5, 0.5): xy(meter_cx, meter_cy): zoomto(w, h): diffuse(0.0, 0.0, 0.0, 1.0): z(91) ));

        // Latch-to-zero for rendering the very frame we die.
        let dead = state.is_failing || state.life <= 0.0;
        let life_for_render = if dead {
            0.0
        } else {
            state.life.clamp(0.0, 1.0)
        };

        let is_hot = !dead && life_for_render >= 1.0;
        let life_color = if is_hot {
            [1.0, 1.0, 1.0, 1.0]
        } else {
            state.player_color
        };

        let filled_width = w * life_for_render;

        // Never draw swoosh if dead OR nothing to fill.
        if filled_width > 0.0 && !dead {
            let bps = state.timing.get_bpm_for_beat(state.current_beat) / 60.0;
            let swoosh_alpha = if is_hot { 1.0 } else { 0.2 };
            actors.push(act!(sprite("swoosh.png"):
                align(0.0, 0.5):
                xy(meter_cx - w / 2.0, meter_cy):
                zoomto(filled_width, h):
                diffusealpha(swoosh_alpha):
                texcoordvelocity(-(bps * 0.5), 0.0):
                z(93)
            ));

            actors.push(act!(quad:
                align(0.0, 0.5):
                xy(meter_cx - w / 2.0, meter_cy):
                zoomto(filled_width, h):
                diffuse(life_color[0], life_color[1], life_color[2], 1.0):
                z(92)
            ));
        }
    }

    actors.push(screen_bar::build(ScreenBarParams {
        title: "",
        title_placement: screen_bar::ScreenBarTitlePlacement::Center,
        position: screen_bar::ScreenBarPosition::Bottom,
        transparent: true,
        fg_color: [1.0; 4],
        left_text: Some(&profile.display_name),
        center_text: None,
        right_text: None,
        left_avatar: None,
    }));

    actors.extend(build_side_pane(state, asset_manager));
    actors.extend(build_holds_mines_rolls_pane(state, asset_manager));

    actors
}

fn build_holds_mines_rolls_pane(state: &State, asset_manager: &AssetManager) -> Vec<Actor> {
    if !is_wide() {
        return vec![];
    }
    let mut actors = Vec::new();

    let sidepane_center_x = screen_width() * 0.75;
    let sidepane_center_y = screen_center_y() + 80.0;
    let logical_screen_width = screen_width();
    let clamped_width = logical_screen_width.clamp(640.0, 854.0);
    let nf_center_x = screen_center_x() - (clamped_width * 0.25);
    let note_field_is_centered = (nf_center_x - screen_center_x()).abs() < 1.0;
    let is_ultrawide = screen_width() / screen_height() > (21.0 / 9.0);
    let banner_data_zoom = if note_field_is_centered && is_wide() && !is_ultrawide {
        let ar = screen_width() / screen_height();
        let t = ((ar - (16.0 / 10.0)) / ((16.0 / 9.0) - (16.0 / 10.0))).clamp(0.0, 1.0);
        0.825 + (0.925 - 0.825) * t
    } else {
        1.0
    };
    let local_x = 155.0;
    let local_y = -112.0;
    let frame_cx = sidepane_center_x + (local_x * banner_data_zoom);
    let frame_cy = sidepane_center_y + (local_y * banner_data_zoom);
    let frame_zoom = banner_data_zoom;

    let categories = [
        ("holds", state.holds_held, state.holds_total),
        ("mines", 0u32, state.chart.stats.mines as u32),
        ("rolls", 0u32, state.chart.stats.rolls as u32),
    ];

    let largest_count = categories
        .iter()
        .map(|(_, achieved, total)| (*achieved).max(*total))
        .max()
        .unwrap_or(0);
    let digits_needed = if largest_count == 0 {
        1
    } else {
        (largest_count as f32).log10().floor() as usize + 1
    };
    let digits_to_fmt = digits_needed.clamp(3, 4);
    let row_height = 28.0 * frame_zoom;
    let mut children = Vec::new();

    asset_manager.with_fonts(|all_fonts| asset_manager.with_font("wendy_screenevaluation", |metrics_font| {
        let value_zoom = 0.4 * frame_zoom;
        let label_zoom = 0.833 * frame_zoom;
        let gray = color::rgba_hex("#5A6166");
        let white = [1.0, 1.0, 1.0, 1.0];
        
        // --- HYBRID LAYOUT LOGIC ---
        // 1. Measure real character widths for number layout.
        let digit_width = font::measure_line_width_logical(metrics_font, "0", all_fonts) as f32 * value_zoom;
        if digit_width <= 0.0 { return; }
        let slash_width = font::measure_line_width_logical(metrics_font, "/", all_fonts) as f32 * value_zoom;

        // 2. Use a hardcoded width for calculating the label's position (for theme parity).
        const LOGICAL_CHAR_WIDTH_FOR_LABEL: f32 = 36.0;
        let fixed_char_width_scaled_for_label = LOGICAL_CHAR_WIDTH_FOR_LABEL * value_zoom;

        for (i, (label_text, achieved, total)) in categories.iter().enumerate() {
            let item_y = (i as f32 - 1.0) * row_height;
            let right_anchor_x = 0.0;
            let mut cursor_x = right_anchor_x;

            let possible_str = format!("{:0width$}", *total as usize, width = digits_to_fmt);
            let achieved_str = format!("{:0width$}", *achieved as usize, width = digits_to_fmt);

            // --- Layout Numbers using MEASURED widths ---
            // 1. Draw "possible" number (right-most part)
            let first_nonzero_possible = possible_str.find(|c: char| c != '0').unwrap_or(possible_str.len());
            for (char_idx, ch) in possible_str.chars().rev().enumerate() {
                let is_dim = if *total == 0 { char_idx > 0 } else {
                    let original_index = digits_to_fmt - 1 - char_idx;
                    original_index < first_nonzero_possible
                };
                let color = if is_dim { gray } else { white };
                let x_pos = cursor_x - (char_idx as f32 * digit_width);
                children.push(act!(text:
                    font("wendy_screenevaluation"): settext(ch.to_string()):
                    align(1.0, 0.5): xy(x_pos, item_y):
                    zoom(value_zoom): diffuse(color[0], color[1], color[2], color[3])
                ));
            }
            cursor_x -= possible_str.len() as f32 * digit_width;

            // 2. Draw slash
            children.push(act!(text: font("wendy_screenevaluation"): settext("/"): align(1.0, 0.5): xy(cursor_x, item_y): zoom(value_zoom): diffuse(gray[0], gray[1], gray[2], gray[3])));
            cursor_x -= slash_width;

            // 3. Draw "achieved" number
            let achieved_block_right_x = cursor_x;
            let first_nonzero_achieved = achieved_str.find(|c: char| c != '0').unwrap_or(achieved_str.len());
            for (char_idx, ch) in achieved_str.chars().rev().enumerate() {
                let is_dim = if *achieved == 0 { char_idx > 0 } else {
                    let original_index = digits_to_fmt - 1 - char_idx;
                    original_index < first_nonzero_achieved
                };
                let color = if is_dim { gray } else { white };
                let x_pos = achieved_block_right_x - (char_idx as f32 * digit_width);
                children.push(act!(text:
                    font("wendy_screenevaluation"): settext(ch.to_string()):
                    align(1.0, 0.5): xy(x_pos, item_y):
                    zoom(value_zoom): diffuse(color[0], color[1], color[2], color[3])
                ));
            }

            // --- Position Label using HARDCODED width assumption ---
            let total_value_width_for_label = (achieved_str.len() + 1 + possible_str.len()) as f32 * fixed_char_width_scaled_for_label;
            let label_x = right_anchor_x - total_value_width_for_label - (10.0 * frame_zoom);
            
            children.push(act!(text:
                font("miso"): settext(*label_text): align(1.0, 0.5): xy(label_x, item_y):
                zoom(label_zoom): diffuse(white[0], white[1], white[2], white[3])
            ));
        }
    }));

    actors.push(Actor::Frame {
        align: [0.5, 0.5],
        offset: [frame_cx, frame_cy],
        size: [SizeSpec::Px(0.0), SizeSpec::Px(0.0)],
        children,
        background: None,
        z: 70,
    });
    actors
}

fn build_side_pane(state: &State, asset_manager: &AssetManager) -> Vec<Actor> {
    if !is_wide() {
        return vec![];
    }
    let mut actors = Vec::new();

    let sidepane_center_x = screen_width() * 0.75;
    let sidepane_center_y = screen_center_y() + 80.0;
    let logical_screen_width = screen_width();
    let clamped_width = logical_screen_width.clamp(640.0, 854.0);
    let nf_center_x = screen_center_x() - (clamped_width * 0.25);
    let note_field_is_centered = (nf_center_x - screen_center_x()).abs() < 1.0;
    let is_ultrawide = screen_width() / screen_height() > (21.0 / 9.0);
    let banner_data_zoom = if note_field_is_centered && is_wide() && !is_ultrawide {
        let ar = screen_width() / screen_height();
        let t = ((ar - (16.0 / 10.0)) / ((16.0 / 9.0) - (16.0 / 10.0))).clamp(0.0, 1.0);
        0.825 + (0.925 - 0.825) * t
    } else {
        1.0
    };

    let judgments_local_x = -widescale(152.0, 204.0);
    let final_judgments_center_x = sidepane_center_x + (judgments_local_x * banner_data_zoom);
    let final_judgments_center_y = sidepane_center_y;
    let parent_local_zoom = 0.8;
    let final_text_base_zoom = banner_data_zoom * parent_local_zoom;

    let total_tapnotes = state.chart.stats.total_steps as f32;
    let digits = if total_tapnotes > 0.0 {
        (total_tapnotes.log10().floor() as usize + 1).max(4)
    } else {
        4
    };
    let extra_digits = digits.saturating_sub(4) as f32;
    let base_label_local_x_offset = 80.0;
    const LABEL_DIGIT_STEP: f32 = 16.0;
    const NUMBER_TO_LABEL_GAP: f32 = 8.0;
    let base_numbers_local_x_offset = base_label_local_x_offset - NUMBER_TO_LABEL_GAP;
    let row_height = 35.0;
    let y_base = -280.0;

    asset_manager.with_fonts(|all_fonts| asset_manager.with_font("wendy_screenevaluation", |f| {
        let numbers_zoom = final_text_base_zoom * 0.5;
        let max_digit_w = (font::measure_line_width_logical(f, "0", all_fonts) as f32) * numbers_zoom;
        if max_digit_w <= 0.0 { return; }

        let digit_local_width = max_digit_w / final_text_base_zoom;
        let label_local_x_offset = base_label_local_x_offset + (extra_digits * LABEL_DIGIT_STEP);
        let label_world_x = final_judgments_center_x + (label_local_x_offset * final_text_base_zoom);
        let numbers_local_x_offset = base_numbers_local_x_offset + (extra_digits * digit_local_width);
        let numbers_cx = final_judgments_center_x + (numbers_local_x_offset * final_text_base_zoom);

        for (index, grade) in JUDGMENT_ORDER.iter().enumerate() {
            let info = JUDGMENT_INFO.get(grade).unwrap();
            let count = *state.judgment_counts.get(grade).unwrap_or(&0);

            let local_y = y_base + (index as f32 * row_height);
            let world_y = final_judgments_center_y + (local_y * final_text_base_zoom);

            let bright = info.color;
            let dim = [bright[0]*0.35, bright[1]*0.35, bright[2]*0.35, bright[3]];
            let full_number_str = format!("{:0width$}", count, width = digits);

            for (i, ch) in full_number_str.chars().enumerate() {
                let is_dim = if count == 0 { i < digits - 1 } else {
                    let first_nonzero = full_number_str.find(|c: char| c != '0').unwrap_or(full_number_str.len());
                    i < first_nonzero
                };
                let color = if is_dim { dim } else { bright };
                let index_from_right = digits - 1 - i;
                let cell_right_x = numbers_cx - (index_from_right as f32 * max_digit_w);

                actors.push(act!(text:
                    font("wendy_screenevaluation"): settext(ch.to_string()):
                    align(1.0, 0.5): xy(cell_right_x, world_y): zoom(numbers_zoom):
                    diffuse(color[0], color[1], color[2], color[3]): z(71)
                ));
            }

            let label_world_y = world_y + (1.0 * final_text_base_zoom);
            let label_zoom = final_text_base_zoom * 0.833;
    
            actors.push(act!(text:
                font("miso"): settext(info.label): align(0.0, 0.5):
                xy(label_world_x, label_world_y): zoom(label_zoom):
                maxwidth(72.0 * final_text_base_zoom): horizalign(left):
                diffuse(bright[0], bright[1], bright[2], bright[3]):
                z(71)
            ));
        }

        // --- Time Display (Remaining / Total) ---
        {
            let local_y = -40.0 * banner_data_zoom;
            
            let total_seconds = state.song.total_length_seconds.max(0) as f32;
            let total_time_str = format_game_time(total_seconds, total_seconds);

            let remaining_seconds = if let Some(fail_time) = state.fail_time {
                (total_seconds - fail_time.max(0.0)).max(0.0)
            } else {
                (total_seconds - state.current_music_time.max(0.0)).max(0.0)
            };
            let remaining_time_str = format_game_time(remaining_seconds, total_seconds);

            let font_name = "miso";
            let text_zoom = banner_data_zoom * 0.833;

            let numbers_block_width = (digits as f32) * max_digit_w;
            let numbers_left_x = numbers_cx - numbers_block_width;

            let red_color = color::rgba_hex("#ff3030");
            let white_color = [1.0, 1.0, 1.0, 1.0];
            let remaining_color = if state.is_failing { red_color } else { white_color };
            
            // --- Total Time Row ---
            let y_pos_total = sidepane_center_y + local_y + 20.0;
            let label_offset = 32.0;
            
            actors.push(act!(text: font(font_name): settext(total_time_str):
                align(0.0, 0.5): horizalign(left):
                xy(numbers_left_x, y_pos_total):
                z(71):
                diffuse(white_color[0], white_color[1], white_color[2], white_color[3])
            ));
            actors.push(act!(text: font(font_name): settext(" song"):
                align(0.0, 0.5): horizalign(left):
                xy(numbers_left_x + label_offset, y_pos_total - 1.0):
                zoom(text_zoom): z(71):
                diffuse(white_color[0], white_color[1], white_color[2], white_color[3])
            ));
            
            // --- Remaining Time Row ---
            let y_pos_remaining = sidepane_center_y + local_y;

            actors.push(act!(text: font(font_name): settext(remaining_time_str):
                align(0.0, 0.5): horizalign(left):
                xy(numbers_left_x, y_pos_remaining):
                z(71):
                diffuse(remaining_color[0], remaining_color[1], remaining_color[2], remaining_color[3])
            ));
            actors.push(act!(text: font(font_name): settext(" remaining"):
                align(0.0, 0.5): horizalign(left):
                xy(numbers_left_x + label_offset, y_pos_remaining - 1.0):
                zoom(text_zoom): z(71):
                diffuse(white_color[0], white_color[1], white_color[2], white_color[3])
            ));
        }
    }));

    actors
}
