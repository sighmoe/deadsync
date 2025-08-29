use crate::act;
use crate::core::space::globals::*;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::color;
use crate::ui::components::heart_bg;
use crate::ui::components::screen_bar::{self, ScreenBarParams, ScreenBarPosition, ScreenBarTitlePlacement};
use std::sync::Arc;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

// --- Optional palette accessors (hex → rgba) if/when you need them later ---
#[allow(dead_code)] fn col_music_wheel_box() -> [f32; 4] { color::rgba_hex("#0a141b") } // #0a141b
#[allow(dead_code)] fn col_pack_header_box() -> [f32; 4] { color::rgba_hex("#4c565d") } // #4c565d
#[allow(dead_code)] fn col_selected_song_box() -> [f32; 4] { color::rgba_hex("#272f35") } // #272f35
#[allow(dead_code)] fn col_selected_pack_header_box() -> [f32; 4] { color::rgba_hex("#5f686e") } // #5f686e
#[allow(dead_code)] fn col_ui_box_dark() -> [f32; 4] { color::rgba_hex("#1e282e") } // #1e282e
#[allow(dead_code)] fn col_pink_box() -> [f32; 4] { color::rgba_hex("#ff47b3") } // #ff47b3

// --- Typography (targets in SM TL units) ---
const MUSIC_WHEEL_TEXT_TARGET_PX: f32 = 23.0;
const DETAIL_HEADER_TEXT_TARGET_PX: f32 = 27.0;
const DETAIL_VALUE_TEXT_TARGET_PX: f32 = 27.0;

// --- Difficulty Meter ---
const DIFFICULTY_DISPLAY_INNER_BOX_COLOR: [f32; 4] = [0.059, 0.059, 0.059, 1.0]; // #0f0f0f
const DIFFICULTY_NAMES: [&str; 5] = ["Beginner", "Easy", "Medium", "Hard", "Challenge"];
const DIFFICULTY_COLORS: [[f32; 4]; 5] = [
    [1.0, 0.745, 0.0, 1.0],   // #FFBE00 Beginner
    [1.0, 0.490, 0.0, 1.0],   // #FF7D00 Easy
    [1.0, 0.365, 0.278, 1.0], // #FF5D47 Medium
    [1.0, 0.341, 0.494, 1.0], // #FF577E Hard
    [1.0, 0.278, 0.702, 1.0], // #FF47B3 Challenge
];

/* ==================================================================
 *                            STATE & DATA
 * ================================================================== */
#[derive(Clone, Debug)]
pub struct SongData {
    pub title: String,
    pub artist: String,
    pub banner_path: Option<&'static str>,
    pub charts: Vec<ChartData>,
}

#[derive(Clone, Debug)]
pub struct ChartData {
    pub difficulty: String,
    pub meter: u32,
    pub step_artist: String,
}

#[derive(Clone, Debug)]
pub enum MusicWheelEntry {
    PackHeader { name: String, color: [f32; 4] },
    Song(Arc<SongData>),
}

pub struct State {
    pub entries: Vec<MusicWheelEntry>,
    pub selected_index: usize,
    pub selected_difficulty_index: usize,
    pub active_color_index: i32,   // ← comes from SelectColor via app.rs transition
    pub selection_animation_timer: f32,

    bg: heart_bg::State,           // animated hearts background
}

pub fn init() -> State {
    let mut entries = vec![];
    entries.push(MusicWheelEntry::PackHeader {
        name: "Awesome Pack 1".to_string(),
        color: color::simply_love_rgba(2),
    });
    entries.push(MusicWheelEntry::Song(Arc::new(SongData {
        title: "Super Driver".to_string(), artist: "Aya Hirano".to_string(),
        banner_path: Some("fallback_banner.png"),
        charts: vec![
            ChartData { difficulty: "Easy".to_string(), meter: 5, step_artist: "Val".to_string() },
            ChartData { difficulty: "Hard".to_string(), meter: 9, step_artist: "Val".to_string() },
            ChartData { difficulty: "Challenge".to_string(), meter: 13, step_artist: "Val".to_string() },
        ],
    })));
    entries.push(MusicWheelEntry::Song(Arc::new(SongData {
        title: "Another Great Song".to_string(), artist: "Some Artist".to_string(),
        banner_path: Some("fallback_banner.png"),
        charts: vec![
            ChartData { difficulty: "Challenge".to_string(), meter: 12, step_artist: "Community".to_string() },
        ],
    })));
    entries.push(MusicWheelEntry::PackHeader {
        name: "Community Pack".to_string(),
        color: color::simply_love_rgba(5),
    });

    State {
        entries,
        selected_index: 0,
        selected_difficulty_index: 2,
        active_color_index: color::DEFAULT_COLOR_INDEX,
        selection_animation_timer: 0.0,
        bg: heart_bg::State::new(),
    }
}

/* ==================================================================
 *                         INPUT & UPDATE
 * ================================================================== */

pub fn handle_key_press(state: &mut State, event: &KeyEvent) -> ScreenAction {
    if event.state != ElementState::Pressed { return ScreenAction::None; }
    let num_entries = state.entries.len();
    if num_entries == 0 { return ScreenAction::None; }

    match event.physical_key {
        PhysicalKey::Code(KeyCode::ArrowRight) | PhysicalKey::Code(KeyCode::KeyD) => {
            state.selected_index = (state.selected_index + 1) % num_entries;
        }
        PhysicalKey::Code(KeyCode::ArrowLeft) | PhysicalKey::Code(KeyCode::KeyA) => {
            state.selected_index = (state.selected_index + num_entries - 1) % num_entries;
        }
        PhysicalKey::Code(KeyCode::ArrowDown) | PhysicalKey::Code(KeyCode::KeyS) => {
            state.selected_difficulty_index = (state.selected_difficulty_index + 1).min(DIFFICULTY_NAMES.len() - 1);
        }
        PhysicalKey::Code(KeyCode::ArrowUp) | PhysicalKey::Code(KeyCode::KeyW) => {
            state.selected_difficulty_index = state.selected_difficulty_index.saturating_sub(1);
        }
        PhysicalKey::Code(KeyCode::Enter) => {
            if let MusicWheelEntry::Song(_) = &state.entries[state.selected_index] {
                return ScreenAction::Navigate(Screen::Gameplay);
            }
        }
        PhysicalKey::Code(KeyCode::Escape) => return ScreenAction::Navigate(Screen::Menu),
        _ => {}
    }
    ScreenAction::None
}

pub fn update(state: &mut State, dt: f32) {
    state.selection_animation_timer += dt;
}

/* ==================================================================
 *                           DRAWING
 * ================================================================== */

pub fn get_actors(state: &State) -> Vec<Actor> {
    let mut actors = Vec::with_capacity(256);

    // --- 0) Animated heart background using the selected color index ---
    actors.extend(state.bg.build(heart_bg::Params {
        active_color_index: state.active_color_index,
        backdrop_rgba: [0.0, 0.0, 0.0, 1.0],
        alpha_mul: 1.0,
    }));

    // --- 1) Header & Footer bars ---
    actors.push(screen_bar::build(ScreenBarParams {
        title: "SELECT MUSIC",
        title_placement: ScreenBarTitlePlacement::Left,
        position: ScreenBarPosition::Top,
        transparent: false,
        fg_color: [1.0; 4],
        left_text: None,
        center_text: None,
        right_text: None,
    }));
    actors.push(screen_bar::build(ScreenBarParams {
        title: "EVENT MODE",
        title_placement: ScreenBarTitlePlacement::Center,
        position: ScreenBarPosition::Bottom,
        transparent: false,
        fg_color: [1.0; 4],
        left_text: None,
        center_text: None,
        right_text: Some("P1: Ready"),
    }));

    // --- 2) Primary accent box (left, flush to top of footer bar) ---
    // NOTE: keep this in sync with ScreenBar height (32.0 in screen_bar.rs)
    const BAR_H: f32 = 32.0;

    let accent = color::simply_love_rgba(state.active_color_index);
    let box_w = 374.0;
    let box_h = 60.0;

    let box_bottom = screen_height() - BAR_H;     // top edge of the bottom bar
    let box_top    = box_bottom - box_h;

    // Rectangle: bottom-left anchored at the bar’s top edge
    actors.push(act!(quad:
        align(0.0, 1.0):
        xy(0.0, box_bottom):
        zoomto(box_w, box_h):
        diffuse(accent[0], accent[1], accent[2], 1.0):
        z(50)
    ));

    // --- 3) Step Info & Rating boxes (above the accent box) ---
    const RATING_BOX_W: f32 = 31.8;
    const RATING_BOX_H: f32 = 151.8;
    const STEP_INFO_BOX_W: f32 = 286.2;
    const STEP_INFO_BOX_H: f32 = 63.6;

    const GAP: f32 = 3.0;
    const MARGIN_ABOVE_STATS_BOX: f32 = 9.0;

    let ui_box_color = col_music_wheel_box();
    let z_layer = 51; // Just above the accent box (50)

    // The bottom of these new boxes aligns to a Y position above the accent box.
    let boxes_bottom_y = box_top - MARGIN_ABOVE_STATS_BOX;

    // The rating box is right-aligned with the accent box below it.
    let rating_box_right_x = box_w;
    actors.push(act!(quad:
        align(1.0, 1.0): // bottom-right pivot
        xy(rating_box_right_x, boxes_bottom_y):
        zoomto(RATING_BOX_W, RATING_BOX_H):
        diffuse(ui_box_color[0], ui_box_color[1], ui_box_color[2], 1.0):
        z(z_layer)
    ));

    // The step info box is to the left of the rating box, with a gap.
    let step_info_box_right_x = rating_box_right_x - RATING_BOX_W - GAP;
    actors.push(act!(quad:
        align(1.0, 1.0): // bottom-right pivot
        xy(step_info_box_right_x, boxes_bottom_y):
        zoomto(STEP_INFO_BOX_W, STEP_INFO_BOX_H):
        diffuse(ui_box_color[0], ui_box_color[1], ui_box_color[2], 1.0):
        z(z_layer)
    ));

    // --- 4) Density Graph Box ---
    // Aligned to the top of the rating box, with the same dimensions as the step info box.
    let rating_box_top_y = boxes_bottom_y - RATING_BOX_H;
    actors.push(act!(quad:
        align(1.0, 0.0): // top-right pivot
        xy(step_info_box_right_x, rating_box_top_y):
        zoomto(STEP_INFO_BOX_W, STEP_INFO_BOX_H):
        diffuse(ui_box_color[0], ui_box_color[1], ui_box_color[2], 1.0):
        z(z_layer)
    ));

    // --- 5) Step Artist Box ---
    const STEP_ARTIST_BOX_W: f32 = 175.2;
    const STEP_ARTIST_BOX_H: f32 = 16.8;
    const GAP_ABOVE_DENSITY: f32 = 1.0;

    let step_artist_box_color = color::simply_love_rgba(state.active_color_index);
    let density_graph_left_x = step_info_box_right_x - STEP_INFO_BOX_W;
    let step_artist_box_bottom_y = rating_box_top_y - GAP_ABOVE_DENSITY;

    actors.push(act!(quad:
        align(0.0, 1.0): // bottom-right pivot
        xy(density_graph_left_x, step_artist_box_bottom_y):
        zoomto(STEP_ARTIST_BOX_W, STEP_ARTIST_BOX_H):
        diffuse(step_artist_box_color[0], step_artist_box_color[1], step_artist_box_color[2], 1.0):
        z(z_layer)
    ));

    // --- 6) Banner Box ---
    const BANNER_BOX_W: f32 = 319.8;
    const BANNER_BOX_H: f32 = 126.0;
    const GAP_BELOW_TOP_BAR: f32 = 2.0;

    let banner_box_top_y = BAR_H + GAP_BELOW_TOP_BAR;
    let banner_box_left_x = density_graph_left_x; // Align with boxes below

    actors.push(act!(sprite("fallback_banner.png"):
        align(0.0, 0.0): // top-left pivot
        xy(banner_box_left_x, banner_box_top_y):
        zoomto(BANNER_BOX_W, BANNER_BOX_H):
        z(z_layer)
    ));

    // Label inside the box (top-left, small)
    let pad_x = 10.0;
    let pad_y = 8.0;
    actors.push(act!(text:
        align(0.0, 0.0):
        xy(pad_x, box_top + pad_y):
        zoomtoheight(15.0):
        font("miso"): settext("STEP STATS"): horizalign(left):
        diffuse(1.0, 1.0, 1.0, 1.0):
        z(51)
    ));

    actors
}
