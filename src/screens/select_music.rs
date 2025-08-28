use crate::act;
use crate::core::space::globals::*;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::color;
use crate::ui::components::screen_bar::{self, ScreenBarParams, ScreenBarPosition, ScreenBarTitlePlacement};
use std::sync::Arc;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/* ==================================================================
 *                       LAYOUT CONSTANTS
 * ================================================================== */

// --- Music Wheel ---
const NUM_MUSIC_WHEEL_BOXES: usize = 15;
const CENTER_MUSIC_WHEEL_SLOT_INDEX: usize = 7;
const MUSIC_WHEEL_BOX_WIDTH: f32 = 588.0;
const MUSIC_WHEEL_BOX_HEIGHT: f32 = 46.0;
const MUSIC_WHEEL_VERTICAL_GAP: f32 = 2.0;

// --- Left Panel ---
const LEFT_PANEL_START_X: f32 = 15.0;
const BANNER_WIDTH: f32 = 418.0;
const BANNER_HEIGHT: f32 = 163.0;
const TOP_ELEMENTS_Y_START: f32 = 42.0; // Below top bar
const VERTICAL_SPACING: f32 = 7.0;

// --- Colors (from user feedback) ---
const MUSIC_WHEEL_BOX_COLOR: [f32; 4] = [0.039, 0.078, 0.106, 1.0]; // #0a141b
const PACK_HEADER_BOX_COLOR: [f32; 4] = [0.298, 0.337, 0.365, 1.0]; // #4c565d
const SELECTED_SONG_BOX_COLOR: [f32; 4] = [0.153, 0.184, 0.208, 1.0]; // #272f35
const SELECTED_PACK_HEADER_BOX_COLOR: [f32; 4] = [0.373, 0.408, 0.431, 1.0]; // #5f686e

const UI_BOX_DARK_COLOR: [f32; 4] = [0.118, 0.157, 0.184, 1.0]; // #1e282e
const PINK_BOX_COLOR: [f32; 4] = [1.0, 0.278, 0.702, 1.0];     // #ff47b3

// --- Typography ---
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
    pub active_color_index: i32,
    pub selection_animation_timer: f32,
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
            ChartData { difficulty: "Easy".to_string(), meter: 5, step_artist: "Val" .to_string() },
            ChartData { difficulty: "Hard".to_string(), meter: 9, step_artist: "Val" .to_string() },
            ChartData { difficulty: "Challenge".to_string(), meter: 13, step_artist: "Val".to_string()},
        ],
    })));
    entries.push(MusicWheelEntry::Song(Arc::new(SongData {
        title: "Another Great Song".to_string(), artist: "Some Artist".to_string(),
        banner_path: Some("fallback_banner.png"),
        charts: vec![
            ChartData { difficulty: "Challenge".to_string(), meter: 12, step_artist: "Community".to_string()},
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
 *                            DRAWING
 * ================================================================== */

fn get_chart_for_difficulty(song: &SongData, difficulty_index: usize) -> Option<&ChartData> {
    DIFFICULTY_NAMES.get(difficulty_index)
        .and_then(|name| song.charts.iter().find(|c| &c.difficulty == name))
}

pub fn get_actors(state: &State) -> Vec<Actor> {
    let mut actors = Vec::with_capacity(256);

    // --- 1. Header & Footer ---
    actors.push(screen_bar::build(ScreenBarParams {
        title: "SELECT MUSIC",
        title_placement: ScreenBarTitlePlacement::Left, position: ScreenBarPosition::Top,
        transparent: false, fg_color: [1.0; 4],
        left_text: None, center_text: None, right_text: None,
    }));
    actors.push(screen_bar::build(ScreenBarParams {
        title: "EVENT MODE",
        title_placement: ScreenBarTitlePlacement::Center, position: ScreenBarPosition::Bottom,
        transparent: false, fg_color: [1.0; 4],
        left_text: None, center_text: None, right_text: Some("P1: Ready"),
    }));

    // --- 2. Music Wheel (Right Aligned) ---
    let wheel_stack_h = (NUM_MUSIC_WHEEL_BOXES as f32 * MUSIC_WHEEL_BOX_HEIGHT) + ((NUM_MUSIC_WHEEL_BOXES -1) as f32 * MUSIC_WHEEL_VERTICAL_GAP);
    let wheel_stack_y = (screen_height() - wheel_stack_h) * 0.5;
    let wheel_x = screen_width() - MUSIC_WHEEL_BOX_WIDTH;

    for i in 0..NUM_MUSIC_WHEEL_BOXES {
        let item_y = wheel_stack_y + (i as f32 * (MUSIC_WHEEL_BOX_HEIGHT + MUSIC_WHEEL_VERTICAL_GAP));
        let num_entries = state.entries.len();
        if num_entries == 0 { continue; }
        
        let list_index = (state.selected_index as isize + i as isize - CENTER_MUSIC_WHEEL_SLOT_INDEX as isize + num_entries as isize) as usize % num_entries;

        if let Some(entry) = state.entries.get(list_index) {
            let is_selected = i == CENTER_MUSIC_WHEEL_SLOT_INDEX;
            
            let (base_color, selected_color, text, text_color, text_align, text_pad) = match entry {
                MusicWheelEntry::PackHeader { name, color } => (PACK_HEADER_BOX_COLOR, SELECTED_PACK_HEADER_BOX_COLOR, name.clone(), *color, 0.5, 0.0),
                MusicWheelEntry::Song(song_data) => (MUSIC_WHEEL_BOX_COLOR, SELECTED_SONG_BOX_COLOR, song_data.title.clone(), [1.0; 4], 0.0, 118.0),
            };
            
            let bg_color = if is_selected { selected_color } else { base_color };

            actors.push(act!(quad:
                align(0.0, 0.0): xy(wheel_x, item_y): zoomto(MUSIC_WHEEL_BOX_WIDTH, MUSIC_WHEEL_BOX_HEIGHT):
                diffuse(bg_color[0], bg_color[1], bg_color[2], bg_color[3])
            ));

            actors.push(act!(text:
                align(text_align, 0.5): xy(wheel_x + text_pad + if text_align == 0.5 { MUSIC_WHEEL_BOX_WIDTH * 0.5 } else { 0.0 }, item_y + MUSIC_WHEEL_BOX_HEIGHT * 0.5):
                zoomtoheight(MUSIC_WHEEL_TEXT_TARGET_PX): font("miso"): settext(text):
                diffuse(text_color[0], text_color[1], text_color[2], text_color[3]): z(1)
            ));
        }
    }

    // --- 3. Left Panel (positioned relative to screen edges and wheel) ---
    let selected_song = state.entries.get(state.selected_index).and_then(|e| match e {
        MusicWheelEntry::Song(s) => Some(s.clone()),
        _ => None,
    });

    let left_panel_width = wheel_x - (LEFT_PANEL_START_X * 2.0);
    let banner_w = BANNER_WIDTH.min(left_panel_width);

    // Banner
    actors.push(act!(quad: // Banner dark background
        align(0.0, 0.0): xy(LEFT_PANEL_START_X, TOP_ELEMENTS_Y_START): zoomto(banner_w, BANNER_HEIGHT):
        diffuse(UI_BOX_DARK_COLOR[0], UI_BOX_DARK_COLOR[1], UI_BOX_DARK_COLOR[2], UI_BOX_DARK_COLOR[3])
    ));
    actors.push(act!(sprite(selected_song.as_ref().and_then(|s| s.banner_path).unwrap_or("fallback_banner.png")):
        align(0.0, 0.0): xy(LEFT_PANEL_START_X, TOP_ELEMENTS_Y_START): zoomto(banner_w, BANNER_HEIGHT)
    ));

    // Details Box
    let details_box_y = TOP_ELEMENTS_Y_START + BANNER_HEIGHT + VERTICAL_SPACING;
    let details_box_h = 75.0;
    actors.push(act!(quad:
        align(0.0, 0.0): xy(LEFT_PANEL_START_X, details_box_y): zoomto(banner_w, details_box_h):
        diffuse(UI_BOX_DARK_COLOR[0], UI_BOX_DARK_COLOR[1], UI_BOX_DARK_COLOR[2], UI_BOX_DARK_COLOR[3])
    ));
    
    // Difficulty Meter
    let meter_outer_x = LEFT_PANEL_START_X + banner_w + VERTICAL_SPACING;
    let meter_outer_w = 68.0;
    let meter_outer_h = BANNER_HEIGHT + VERTICAL_SPACING + details_box_h;
    actors.push(act!(quad:
        align(0.0, 0.0): xy(meter_outer_x, TOP_ELEMENTS_Y_START): zoomto(meter_outer_w, meter_outer_h):
        diffuse(UI_BOX_DARK_COLOR[0], UI_BOX_DARK_COLOR[1], UI_BOX_DARK_COLOR[2], UI_BOX_DARK_COLOR[3])
    ));

    let pad = 3.0;
    let box_size_w = meter_outer_w - (2.0 * pad);
    let box_size_h = (meter_outer_h - (6.0 * pad)) / 5.0;
    for i in 0..5 {
        let box_y = TOP_ELEMENTS_Y_START + pad + i as f32 * (box_size_h + pad);
        actors.push(act!(quad:
            align(0.0, 0.0): xy(meter_outer_x + pad, box_y): zoomto(box_size_w, box_size_h):
            diffuse(DIFFICULTY_DISPLAY_INNER_BOX_COLOR[0], DIFFICULTY_DISPLAY_INNER_BOX_COLOR[1], DIFFICULTY_DISPLAY_INNER_BOX_COLOR[2], DIFFICULTY_DISPLAY_INNER_BOX_COLOR[3])
        ));
        if let Some(song) = &selected_song {
            if let Some(chart) = get_chart_for_difficulty(song, i) {
                actors.push(act!(text:
                    align(0.5, 0.5): xy(meter_outer_x + pad + 0.5 * box_size_w, box_y + 0.5 * box_size_h):
                    zoomtoheight(39.0): font("wendy"): settext(format!("{}", chart.meter)): horizalign(center):
                    diffuse(DIFFICULTY_COLORS[i][0], DIFFICULTY_COLORS[i][1], DIFFICULTY_COLORS[i][2], DIFFICULTY_COLORS[i][3])
                ));
            }
        }
    }

    actors
}