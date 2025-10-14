use crate::act;
use crate::core::space::*;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::{Actor, SizeSpec};
use crate::ui::color;
use crate::ui::components::{heart_bg, pad_display, screen_bar};
use crate::ui::components::screen_bar::{ScreenBarParams, ScreenBarPosition, ScreenBarTitlePlacement};
use crate::core::space::widescale;

use crate::screens::gameplay::{self, JudgeGrade};
use crate::gameplay::song::SongData;
use crate::gameplay::chart::ChartData;
use crate::gameplay::scores;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use crate::assets::AssetManager;
use crate::ui::font;

use crate::gameplay::profile;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/* ---------------------------- transitions ---------------------------- */
const TRANSITION_IN_DURATION: f32 = 0.4;
const TRANSITION_OUT_DURATION: f32 = 0.4;

// A struct to hold a snapshot of the final score data from the gameplay screen.
#[derive(Clone)]
pub struct ScoreInfo {
    pub song: Arc<SongData>,
    pub chart: Arc<ChartData>,
    pub judgment_counts: HashMap<JudgeGrade, u32>,
    pub score_percent: f64,
    pub grade: scores::Grade,
}

pub struct State {
    pub active_color_index: i32,
    bg: heart_bg::State,
    pub session_elapsed: f32, // To display the timer
    pub score_info: Option<ScoreInfo>,
}

pub fn init(gameplay_results: Option<gameplay::State>) -> State {
    let score_info = gameplay_results.map(|gs| {
        let max_dp = (gs.notes.len() as f64) * 2.0;
        let mut earned_dp = 0.0;
        earned_dp += *gs.judgment_counts.get(&JudgeGrade::Fantastic).unwrap_or(&0) as f64 * 2.0;
        earned_dp += *gs.judgment_counts.get(&JudgeGrade::Excellent).unwrap_or(&0) as f64 * 2.0;
        earned_dp += *gs.judgment_counts.get(&JudgeGrade::Great).unwrap_or(&0) as f64 * 1.0;
        earned_dp += *gs.judgment_counts.get(&JudgeGrade::WayOff).unwrap_or(&0) as f64 * -4.0;
        earned_dp += *gs.judgment_counts.get(&JudgeGrade::Miss).unwrap_or(&0) as f64 * -8.0;

        let score_percent = if max_dp > 0.0 { (earned_dp / max_dp).max(0.0) } else { 0.0 };

        // Use the public function from scores to determine the grade from the percentage.
        // let grade = scores::score_to_grade(score_percent * 10000.0);
        // TEMPORARY: Always show the Failed grade until pass/fail logic is implemented.
        let grade = scores::Grade::Failed;

        ScoreInfo {
            song: gs.song.clone(),
            chart: gs.chart.clone(),
            judgment_counts: gs.judgment_counts.clone(),
            score_percent,
            grade,
        }
    });

    State {
        active_color_index: color::DEFAULT_COLOR_INDEX, // This will be overwritten by app.rs
        bg: heart_bg::State::new(),
        session_elapsed: 0.0,
        score_info,
    }
}

pub fn handle_key_press(_state: &mut State, event: &KeyEvent) -> ScreenAction {
    if event.state == ElementState::Pressed {
        if let PhysicalKey::Code(KeyCode::Escape) | PhysicalKey::Code(KeyCode::Enter) = event.physical_key {
            return ScreenAction::Navigate(Screen::SelectMusic);
        }
    }
    ScreenAction::None
}

// This screen doesn't have any dynamic state updates yet, but we keep the function for consistency.
pub fn update(_state: &mut State, _dt: f32) {
    //
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
        diffuse(0.0, 0.0, 0.0, 0.0):
        z(1200):
        linear(TRANSITION_OUT_DURATION): alpha(1.0)
    );
    (vec![actor], TRANSITION_OUT_DURATION)
}

fn format_session_time(seconds_total: f32) -> String {
    if seconds_total < 0.0 {
        return "0:00".to_string();
    }
    let seconds_total = seconds_total as u64;

    let hours = seconds_total / 3600;
    let minutes = (seconds_total % 3600) / 60;
    let seconds = seconds_total % 60;

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{}:{:02}", minutes, seconds)
    }
}

pub fn get_actors(state: &State, asset_manager: &AssetManager) -> Vec<Actor> {
    let mut actors = Vec::with_capacity(20);
    let profile = profile::get();

    // 1. Background
    actors.extend(state.bg.build(heart_bg::Params {
        active_color_index: state.active_color_index,
        backdrop_rgba: [0.0, 0.0, 0.0, 1.0],
        alpha_mul: 1.0,
    }));

    // 2. Top Bar
    actors.push(screen_bar::build(ScreenBarParams {
        title: "EVALUATION",
        title_placement: ScreenBarTitlePlacement::Left,
        position: ScreenBarPosition::Top,
        transparent: false,
        fg_color: [1.0; 4],
        left_text: None, center_text: None, right_text: None,
    }));

    // Session Timer
    let timer_text = format_session_time(state.session_elapsed);
    actors.push(act!(text:
        font("wendy_monospace_numbers"):
        settext(timer_text):
        align(0.5, 0.5):
        xy(screen_center_x(), 10.0):
        zoom(widescale(0.3, 0.36)):
        z(121):
        diffuse(1.0, 1.0, 1.0, 1.0):
        horizalign(center)
    ));
    
    let Some(score_info) = &state.score_info else {
        actors.push(act!(text:
            font("wendy"):
            settext("NO SCORE DATA AVAILABLE"):
            align(0.5, 0.5): xy(screen_center_x(), screen_center_y()):
            zoom(0.8): horizalign(center):
            z(100)
        ));
        return actors;
    };

    // --- Corrected coordinates: 0,0 is top-left, 480 is bottom ---
    let cy = screen_center_y(); // This is the logical center (240)

    // --- Title, Banner, and Song Features (Center Column) ---
    {
        // --- TitleAndBanner Group ---
        let banner_key = score_info.song.banner_path.as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| {
                let banner_num = state.active_color_index.rem_euclid(12) + 1;
                format!("banner{}.png", banner_num)
            });

        let full_title = if score_info.song.subtitle.trim().is_empty() {
            score_info.song.title.clone()
        } else {
            format!("{} {}", score_info.song.title, score_info.song.subtitle)
        };

        let title_and_banner_frame = Actor::Frame {
            align: [0.5, 0.5],
            offset: [screen_center_x(), 46.0],
            size: [SizeSpec::Px(0.0), SizeSpec::Px(0.0)],
            children: vec![
                // Banner (drawn first, behind title)
                act!(sprite(banner_key):
                    align(0.5, 0.5):
                    xy(0.0, 66.0):
                    setsize(418.0, 164.0):
                    zoom(0.7):
                    z(0)
                ),
                // Quad behind title
                act!(quad:
                    align(0.5, 0.5):
                    xy(0.0, 0.0):
                    setsize(418.0, 25.0):
                    zoom(0.7):
                    diffuse(0.117, 0.157, 0.184, 1.0): // #1E282F
                    z(1)
                ),
                // Title text
                act!(text:
                    font("miso"): settext(full_title):
                    align(0.5, 0.5): xy(0.0, 0.0):
                    maxwidth(418.0 * 0.7):
                    z(2)
                ),
            ],
            background: None,
            z: 50,
        };
        actors.push(title_and_banner_frame);

        // --- SongFeatures Group ---
        let bpm_text = {
            let min = score_info.song.min_bpm.round() as i32;
            let max = score_info.song.max_bpm.round() as i32;
            if (score_info.song.min_bpm - score_info.song.max_bpm).abs() < 1e-6 {
                format!("{} bpm", min)
            } else { format!("{} - {} bpm", min, max) }
        };

        let length_text = {
            let seconds = score_info.song.total_length_seconds;
            if seconds < 0 { "".to_string() }
            else if seconds >= 3600 { format!("{}:{:02}:{:02}", seconds / 3600, (seconds % 3600) / 60, seconds % 60) }
            else { format!("{}:{:02}", seconds / 60, seconds % 60) }
        };

        let song_features_frame = Actor::Frame {
            align: [0.5, 0.5],
            offset: [screen_center_x(), 175.0],
            size: [SizeSpec::Px(0.0), SizeSpec::Px(0.0)],
            children: vec![
                act!(quad: align(0.5, 0.5): xy(0.0, 0.0): setsize(418.0, 16.0): zoom(0.7): diffuse(0.117, 0.157, 0.184, 1.0): z(0) ),
                act!(text: font("miso"): settext(score_info.song.artist.clone()): align(0.0, 0.5): xy(-145.0, 0.0): zoom(0.6): maxwidth(418.0 / 2.3): z(1) ),
                act!(text: font("miso"): settext(bpm_text): align(0.5, 0.5): xy(0.0, 0.0): zoom(0.6): maxwidth(418.0 / 0.875): z(1) ),
                act!(text: font("miso"): settext(length_text): align(1.0, 0.5): xy(145.0, 0.0): zoom(0.6): z(1) ),
            ],
            background: None,
            z: 50,
        };
        actors.push(song_features_frame);
    }

    // --- Player 1 Upper Content Frame ---
    let p1_frame_x = screen_center_x() - 155.0;

    // Letter Grade
    actors.push(act!(sprite("grades/grades 1x19.png"):
        align(0.5, 0.5): xy(p1_frame_x - 70.0, cy - 134.0):
        zoom(0.4): z(101):
        setstate(score_info.grade.to_sprite_state())
    ));

    // Difficulty Text and Meter Block
    {
        let difficulty_color = color::difficulty_rgba(&score_info.chart.difficulty, state.active_color_index);
        let difficulty_text = format!("single / {}", score_info.chart.difficulty);
        
        actors.push(act!(text:
            font("miso"): settext(difficulty_text):
            align(0.0, 0.5): xy(p1_frame_x - 115.0, cy - 64.0):
            zoom(0.7): z(101): diffuse(1.0, 1.0, 1.0, 1.0)
        ));
        actors.push(act!(quad:
            align(0.5, 0.5): xy(p1_frame_x - 134.5, cy - 71.0):
            zoomto(30.0, 30.0): z(101):
            diffuse(difficulty_color[0], difficulty_color[1], difficulty_color[2], 1.0)
        ));
        actors.push(act!(text:
            font("wendy"): settext(score_info.chart.meter.to_string()):
            align(0.5, 0.5): xy(p1_frame_x - 134.5, cy - 71.0):
            zoom(0.4): z(102): diffuse(0.0, 0.0, 0.0, 1.0)
        ));
    }

    // Step Artist
    actors.push(act!(text:
        font("miso"): settext(score_info.chart.step_artist.clone()):
        align(0.0, 0.5): xy(p1_frame_x - 115.0, cy - 80.0):
        zoom(0.7): z(101): diffuse(1.0, 1.0, 1.0, 1.0)
    ));

    // --- Player 1 Score Percentage Display ---
    {
        let score_frame_y = screen_center_y() - 26.0;
        let percent_text = format!("{:.2}", score_info.score_percent * 100.0);
        let score_bg_color = color::rgba_hex("#101519");

        let score_display_frame = Actor::Frame {
            // This frame's center is positioned relative to the P1 side.
            align: [0.5, 0.5],
            offset: [p1_frame_x, score_frame_y],
            size: [SizeSpec::Px(0.0), SizeSpec::Px(0.0)],
            background: None,
            z: 101, // Keep it on the same layer as other P1 UI
            children: vec![
                // Background Quad, positioned relative to the frame's center.
                act!(quad:
                    align(0.0, 0.5): // left-aligned
                    xy(-150.0, 0.0):
                    setsize(158.5, 60.0):
                    diffuse(score_bg_color[0], score_bg_color[1], score_bg_color[2], 1.0)
                ),
                // Percentage Text, positioned relative to the frame's center.
                act!(text: font("wendy_white"): settext(percent_text): align(1.0, 0.5): xy(1.5, 0.0): zoom(0.585): horizalign(right)),
            ],
        };
        actors.push(score_display_frame);
    }

    // --- Player 1 Lower Stats Pane Background ---
    // This replicates the large quad for a single player from the Lua.
    {
        let pane_width = (300.0 * 2.0) + 10.0; // Two small panes plus spacing
        let pane_x_left = screen_center_x() - 305.0; // Centered group's left edge

        // The top of this pane should align with the top of the score percentage box's background.
        // Score pane bg is centered at (cy - 26) with height 60, so its top is at (cy - 26) - 30 = cy - 56.
        let pane_y_top = screen_center_y() - 56.0;

        // The bottom edge should align with the visually correct result from the first attempt,
        // which placed the top of a 180px pane at `cy + 34`.
        let pane_y_bottom = (screen_center_y() + 34.0) + 180.0;

        // The correct height is the distance between these two visual boundaries.
        let pane_height = pane_y_bottom - pane_y_top;
        let pane_bg_color = color::rgba_hex("#1E282F");

        actors.push(act!(quad:
            align(0.0, 0.0): // top-left alignment
            xy(pane_x_left, pane_y_top):
            zoomto(pane_width, pane_height):
            diffuse(pane_bg_color[0], pane_bg_color[1], pane_bg_color[2], 1.0):
            z(100) // Below stats text, above main background
        ));
    }

    // --- "ITG" text and Pads (top right) ---
    {
        let itg_text_x = screen_width() - widescale(55.0, 62.0);
        actors.push(act!(text:
            font("wendy"): settext("ITG"):
            align(1.0, 0.5): xy(itg_text_x, 15.0):
            zoom(widescale(0.5, 0.6)): z(121):
            diffuse(1.0, 1.0, 1.0, 1.0):
        ));

        let final_pad_zoom = 0.24 * widescale(0.435, 0.525);

        actors.push(pad_display::build(pad_display::PadDisplayParams {
            center_x: screen_width() - widescale(35.0, 41.0),
            center_y: widescale(22.0, 23.5),
            zoom: final_pad_zoom,
            z: 121,
            is_active: true,
        }));
        actors.push(pad_display::build(pad_display::PadDisplayParams {
            center_x: screen_width() - widescale(15.0, 17.0),
            center_y: widescale(22.0, 23.5),
            zoom: final_pad_zoom,
            z: 121,
            is_active: false,
        }));
    }

    // 3. Bottom Bar
    actors.push(screen_bar::build(ScreenBarParams {
        title: "",
        title_placement: screen_bar::ScreenBarTitlePlacement::Center,
        position: screen_bar::ScreenBarPosition::Bottom,
        transparent: true,
        fg_color: [1.0; 4],
        left_text: Some(&profile.display_name), center_text: None, right_text: None,
    }));

    actors
}
