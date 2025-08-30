// Path: /mnt/c/Users/perfe/Documents/GitHub/new-engine/src/screens/select_music.rs

use crate::act;
use crate::core::space::globals::*;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::color;
use crate::ui::components::heart_bg;
use crate::ui::components::screen_bar::{self, ScreenBarParams, ScreenBarPosition, ScreenBarTitlePlacement};
use std::sync::{Arc, LazyLock};
use std::path::PathBuf;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use log::info;

// --- IMPORT DATA STRUCTURES FROM THE NEW MODULE ---
use crate::core::song_loading::{ChartData, SongData, get_song_cache};

// --- (UI layout constants remain the same) ---
#[allow(dead_code)] fn col_music_wheel_box() -> [f32; 4] { color::rgba_hex("#0a141b") }
#[allow(dead_code)] fn col_pack_header_box() -> [f32; 4] { color::rgba_hex("#4c565d") }
#[allow(dead_code)] fn col_selected_song_box() -> [f32; 4] { color::rgba_hex("#272f35") }
#[allow(dead_code)] fn col_selected_pack_header_box() -> [f32; 4] { color::rgba_hex("#5f686e") }
#[allow(dead_code)] fn col_pink_box() -> [f32; 4] { color::rgba_hex("#ff47b3") }
static UI_BOX_BG_COLOR: LazyLock<[f32; 4]> = LazyLock::new(|| color::rgba_hex("#1E282F"));
const MUSIC_WHEEL_TEXT_TARGET_PX: f32 = 15.0;
const DETAIL_HEADER_TEXT_TARGET_PX: f32 = 22.0;
const DETAIL_VALUE_TEXT_TARGET_PX: f32 = 15.0;
const NUM_WHEEL_ITEMS: usize = 13;
const CENTER_WHEEL_SLOT_INDEX: usize = NUM_WHEEL_ITEMS / 2;
const SELECTION_ANIMATION_CYCLE_DURATION: f32 = 1.0;
const SONG_TEXT_LEFT_PADDING: f32 = 66.0;
const PACK_COUNT_RIGHT_PADDING: f32 = 11.0;
const PACK_COUNT_TEXT_TARGET_PX: f32 = 14.0;
static DIFFICULTY_DISPLAY_INNER_BOX_COLOR: LazyLock<[f32; 4]> = LazyLock::new(|| color::rgba_hex("#0f0f0f"));
const DIFFICULTY_NAMES: [&str; 5] = ["Beginner", "Easy", "Medium", "Hard", "Challenge"];

const BANNER_KEYS: [&'static str; 12] = [
    "banner1.png", "banner2.png", "banner3.png", "banner4.png",
    "banner5.png", "banner6.png", "banner7.png", "banner8.png",
    "banner9.png", "banner10.png", "banner11.png", "banner12.png",
];

/* ==================================================================
 *                            STATE & DATA
 * ================================================================== */
// Note: SongData and ChartData are now imported from core::song_loading

#[derive(Clone, Debug)]
pub enum MusicWheelEntry {
    PackHeader { name: String, original_index: usize },
    Song(Arc<SongData>),
}

pub struct State {
    pub all_entries: Vec<MusicWheelEntry>,
    pub entries: Vec<MusicWheelEntry>,
    pub selected_index: usize,
    pub selected_difficulty_index: usize,
    pub active_color_index: i32,
    pub selection_animation_timer: f32,
    pub expanded_pack_name: Option<String>,
    bg: heart_bg::State,
    pub last_checked_banner_path: Option<PathBuf>,
    pub current_banner_key: &'static str,
}

/// Rebuilds the visible `entries` list based on which pack is expanded.
fn rebuild_displayed_entries(state: &mut State) {
    let mut new_entries = Vec::new();
    let mut current_pack_name: Option<String> = None;

    for entry in &state.all_entries {
        match entry {
            MusicWheelEntry::PackHeader { name, .. } => {
                current_pack_name = Some(name.clone());
                new_entries.push(entry.clone());
            }
            MusicWheelEntry::Song(_) => {
                if state.expanded_pack_name.as_ref() == current_pack_name.as_ref() {
                    new_entries.push(entry.clone());
                }
            }
        }
    }
    state.entries = new_entries;
}

pub fn init() -> State {
    info!("Initializing SelectMusic screen, reading from song cache...");
    let mut all_entries = vec![];
    let song_cache = get_song_cache();

    for (i, pack) in song_cache.iter().enumerate() {
        all_entries.push(MusicWheelEntry::PackHeader {
            name: pack.name.clone(),
            original_index: i,
        });
        for song in &pack.songs {
            all_entries.push(MusicWheelEntry::Song(song.clone()));
        }
    }
    
    let total_songs: usize = song_cache.iter().map(|p| p.songs.len()).sum();
    info!("Read {} packs and {} total songs from cache.", song_cache.len(), total_songs);

    let mut state = State {
        all_entries,
        entries: Vec::new(),
        selected_index: 0,
        selected_difficulty_index: 2,
        active_color_index: color::DEFAULT_COLOR_INDEX,
        selection_animation_timer: 0.0,
        expanded_pack_name: None,
        bg: heart_bg::State::new(),
        last_checked_banner_path: None,
        current_banner_key: "banner1.png", // Default fallback
    };

    rebuild_displayed_entries(&mut state);
    state
}

/* ==================================================================
 *                         INPUT & UPDATE
 * ================================================================== */
fn lerp_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t, a[3] + (b[3] - a[3]) * t,
    ]
}

pub fn handle_key_press(state: &mut State, event: &KeyEvent) -> ScreenAction {
    if event.state != ElementState::Pressed { return ScreenAction::None; }
    let num_entries = state.entries.len();
    if num_entries == 0 { return ScreenAction::None; }

    match event.physical_key {
        PhysicalKey::Code(KeyCode::ArrowRight) | PhysicalKey::Code(KeyCode::KeyD) => {
            state.selected_index = (state.selected_index + 1) % num_entries;
            state.selection_animation_timer = 0.0;
        }
        PhysicalKey::Code(KeyCode::ArrowLeft) | PhysicalKey::Code(KeyCode::KeyA) => {
            state.selected_index = (state.selected_index + num_entries - 1) % num_entries;
            state.selection_animation_timer = 0.0;
        }
        PhysicalKey::Code(KeyCode::ArrowDown) | PhysicalKey::Code(KeyCode::KeyS) => {
            state.selected_difficulty_index = (state.selected_difficulty_index + 1).min(DIFFICULTY_NAMES.len() - 1);
        }
        PhysicalKey::Code(KeyCode::ArrowUp) | PhysicalKey::Code(KeyCode::KeyW) => {
            state.selected_difficulty_index = state.selected_difficulty_index.saturating_sub(1);
        }
        PhysicalKey::Code(KeyCode::Enter) => {
            if let Some(entry) = state.entries.get(state.selected_index).cloned() {
                match entry {
                    MusicWheelEntry::Song(song) => {
                        info!("Selected song: '{}'. It has {} charts.", song.title, song.charts.len());
                        // Future: Pass the selected song/chart to the gameplay screen
                        return ScreenAction::Navigate(Screen::Gameplay);
                    }
                    MusicWheelEntry::PackHeader { name, .. } => {
                        let pack_name_to_focus = name.clone();
                        if state.expanded_pack_name.as_ref() == Some(&pack_name_to_focus) {
                            state.expanded_pack_name = None;
                        } else {
                            state.expanded_pack_name = Some(pack_name_to_focus.clone());
                        }
                        rebuild_displayed_entries(state);
                        let new_selection = state.entries.iter().position(|e| {
                            if let MusicWheelEntry::PackHeader{ name: n, .. } = e { n == &pack_name_to_focus } else { false }
                        }).unwrap_or(0);
                        state.selected_index = new_selection;
                    }
                }
            }
        }
        PhysicalKey::Code(KeyCode::Escape) => return ScreenAction::Navigate(Screen::Menu),
        _ => {}
    }
    ScreenAction::None
}

pub fn update(state: &mut State, dt: f32) -> ScreenAction {
    state.selection_animation_timer += dt;
    if state.selection_animation_timer > SELECTION_ANIMATION_CYCLE_DURATION {
        state.selection_animation_timer -= SELECTION_ANIMATION_CYCLE_DURATION;
    }

    let mut requested_action = ScreenAction::None;
    let mut new_path: Option<PathBuf> = None;

    if let Some(entry) = state.entries.get(state.selected_index) {
        if let MusicWheelEntry::Song(song) = entry {
            if let Some(banner_path) = &song.banner_path {
                new_path = Some(banner_path.clone());
            }
        }
    }

    if state.last_checked_banner_path != new_path {
        if let Some(path) = new_path.clone() {
             requested_action = ScreenAction::RequestBanner(path);
        }
        state.last_checked_banner_path = new_path;
    }
    
    requested_action
}

/* ==================================================================
 *                           DRAWING
 * ================================================================== */
pub fn get_actors(state: &State) -> Vec<Actor> {
    let mut actors = Vec::with_capacity(256);

    // 1. Draw background and standard screen bars
    actors.extend(state.bg.build(heart_bg::Params {
        active_color_index: state.active_color_index,
        backdrop_rgba: [0.0, 0.0, 0.0, 1.0],
        alpha_mul: 1.0,
    }));
    actors.push(screen_bar::build(ScreenBarParams {
        title: "SELECT MUSIC",
        title_placement: ScreenBarTitlePlacement::Left,
        position: ScreenBarPosition::Top,
        transparent: false,
        fg_color: [1.0; 4],
        left_text: None, center_text: None, right_text: None,
    }));
    actors.push(screen_bar::build(ScreenBarParams {
        title: "EVENT MODE",
        title_placement: ScreenBarTitlePlacement::Center,
        position: ScreenBarPosition::Bottom,
        transparent: false,
        fg_color: [1.0; 4],
        left_text: Some("PerfectTaste"), center_text: None, right_text: None,
    }));

    // --- Layout Constants & Calculations ---
    const BAR_H: f32 = 32.0;
    let accent = color::simply_love_rgba(state.active_color_index);
    let box_w = 373.8;
    let box_h = 60.0;
    let box_bottom = screen_height() - BAR_H;
    let box_top = box_bottom - box_h;

    // 2. Draw the main UI panels on the left
    actors.push(act!(quad: align(0.0, 1.0): xy(0.0, box_bottom): zoomto(box_w, box_h): diffuse(accent[0], accent[1], accent[2], 1.0): z(50) ));

    const RATING_BOX_W: f32 = 31.8;
    const RATING_BOX_H: f32 = 151.8;
    const STEP_INFO_BOX_W: f32 = 286.2;
    const STEP_INFO_BOX_H: f32 = 63.6;
    const GAP: f32 = 1.8;
    const MARGIN_ABOVE_STATS_BOX: f32 = 5.4;
    let z_layer = 51;
    let boxes_bottom_y = box_top - MARGIN_ABOVE_STATS_BOX;
    let rating_box_right_x = box_w;

    actors.push(act!(quad: align(1.0, 1.0): xy(rating_box_right_x, boxes_bottom_y): zoomto(RATING_BOX_W, RATING_BOX_H): diffuse(UI_BOX_BG_COLOR[0], UI_BOX_BG_COLOR[1], UI_BOX_BG_COLOR[2], 1.0): z(z_layer) ));

    let step_info_box_right_x = rating_box_right_x - RATING_BOX_W - GAP;
    actors.push(act!(quad: align(1.0, 1.0): xy(step_info_box_right_x, boxes_bottom_y): zoomto(STEP_INFO_BOX_W, STEP_INFO_BOX_H): diffuse(UI_BOX_BG_COLOR[0], UI_BOX_BG_COLOR[1], UI_BOX_BG_COLOR[2], 1.0): z(z_layer) ));
    let rating_box_top_y = boxes_bottom_y - RATING_BOX_H;
    actors.push(act!(quad: align(1.0, 0.0): xy(step_info_box_right_x, rating_box_top_y): zoomto(STEP_INFO_BOX_W, STEP_INFO_BOX_H): diffuse(UI_BOX_BG_COLOR[0], UI_BOX_BG_COLOR[1], UI_BOX_BG_COLOR[2], 1.0): z(z_layer) ));

    const STEP_ARTIST_BOX_W: f32 = 175.2;
    const STEP_ARTIST_BOX_H: f32 = 16.8;
    const GAP_ABOVE_DENSITY: f32 = 1.0;
    let step_artist_box_color = color::simply_love_rgba(state.active_color_index);
    let density_graph_left_x = step_info_box_right_x - STEP_INFO_BOX_W;
    let step_artist_box_bottom_y = rating_box_top_y - GAP_ABOVE_DENSITY;
    actors.push(act!(quad: align(0.0, 1.0): xy(density_graph_left_x, step_artist_box_bottom_y): zoomto(STEP_ARTIST_BOX_W, STEP_ARTIST_BOX_H): diffuse(step_artist_box_color[0], step_artist_box_color[1], step_artist_box_color[2], 1.0): z(z_layer) ));

    const BANNER_BOX_W: f32 = 319.8;
    const BANNER_BOX_H: f32 = 126.0;
    const GAP_BELOW_TOP_BAR: f32 = 1.0;
    let banner_box_top_y = BAR_H + GAP_BELOW_TOP_BAR;
    let banner_box_left_x = density_graph_left_x;
    
    // 3. --- Add difficulty rating boxes and numbers AND determine banner key ---
    let selected_song: Option<&Arc<SongData>> = if let Some(entry) = state.entries.get(state.selected_index) {
        if let MusicWheelEntry::Song(song_info) = entry {
            Some(song_info)
        } else {
            None
        }
    } else {
        None
    };

    let banner_key = if let Some(song) = selected_song {
        if song.banner_path.is_some() {
            state.current_banner_key
        } else {
            let num_banners = BANNER_KEYS.len();
            let wrapped_index = (state.active_color_index.rem_euclid(num_banners as i32)) as usize;
            BANNER_KEYS[wrapped_index]
        }
    } else {
        let num_banners = BANNER_KEYS.len();
        let wrapped_index = (state.active_color_index.rem_euclid(num_banners as i32)) as usize;
        BANNER_KEYS[wrapped_index]
    };

    actors.push(act!(sprite(banner_key): align(0.0, 0.0): xy(banner_box_left_x, banner_box_top_y): zoomto(BANNER_BOX_W, BANNER_BOX_H): z(z_layer) ));

    let rating_box_left_x = rating_box_right_x - RATING_BOX_W;
    
    const INNER_BOX_SIZE: f32 = 28.2;
    const HORIZONTAL_PADDING: f32 = 1.8;
    const VERTICAL_GAP: f32 = 1.8;

    for i in 0..DIFFICULTY_NAMES.len() {
        let inner_box_x = rating_box_left_x + HORIZONTAL_PADDING;
        let inner_box_y = rating_box_top_y + VERTICAL_GAP + (i as f32 * (INNER_BOX_SIZE + VERTICAL_GAP));

        actors.push(act!(quad:
            align(0.0, 0.0):
            xy(inner_box_x, inner_box_y):
            zoomto(INNER_BOX_SIZE, INNER_BOX_SIZE):
            diffuse(DIFFICULTY_DISPLAY_INNER_BOX_COLOR[0], DIFFICULTY_DISPLAY_INNER_BOX_COLOR[1], DIFFICULTY_DISPLAY_INNER_BOX_COLOR[2], 1.0):
            z(z_layer + 1)
        ));

        if let Some(song) = selected_song {
            let difficulty_name = DIFFICULTY_NAMES[i];
            if let Some(chart) = song.charts.iter().find(|c| c.difficulty.eq_ignore_ascii_case(difficulty_name)) {
                let meter = chart.meter;
                if meter > 0 {
                    let color_offset = (DIFFICULTY_NAMES.len() - 1 - i) as i32;
                    let text_color = color::simply_love_rgba(state.active_color_index - color_offset);
                    const METER_TEXT_PX: f32 = 20.0;
                    let text_center_x = inner_box_x + 0.5 * INNER_BOX_SIZE;
                    let text_center_y = inner_box_y + 0.5 * INNER_BOX_SIZE;
                    actors.push(act!(text:
                        align(0.5, 0.5):
                        xy(text_center_x, text_center_y):
                        zoomtoheight(METER_TEXT_PX):
                        font("wendy"):
                        settext(format!("{}", meter)):
                        horizalign(center):
                        diffuse(text_color[0], text_color[1], text_color[2], 1.0):
                        z(z_layer + 2)
                    ));
                }
            }
        }
    }

    const BPM_BOX_H: f32 = 49.8;
    const GAP_BELOW_BANNER: f32 = 1.0;
    let bpm_box_top_y = banner_box_top_y + BANNER_BOX_H + GAP_BELOW_BANNER;
    actors.push(act!(quad: align(0.0, 0.0): xy(banner_box_left_x, bpm_box_top_y): zoomto(BANNER_BOX_W, BPM_BOX_H): diffuse(UI_BOX_BG_COLOR[0], UI_BOX_BG_COLOR[1], UI_BOX_BG_COLOR[2], 1.0): z(z_layer) ));

    // 4. Draw the music wheel on the right
    const WHEEL_W: f32 = 352.8;
    const WHEEL_ITEM_GAP: f32 = 1.0;
    let wheel_right_x = screen_width();
    let wheel_left_x = wheel_right_x - WHEEL_W;
    let content_area_y_start = BAR_H;
    let content_area_y_end = screen_height() - BAR_H;
    let total_available_h = content_area_y_end - content_area_y_start;
    let total_gap_h = (NUM_WHEEL_ITEMS + 1) as f32 * WHEEL_ITEM_GAP;
    let total_items_h = total_available_h - total_gap_h;
    let item_h = total_items_h / NUM_WHEEL_ITEMS as f32;
    let anim_t_unscaled = (state.selection_animation_timer / SELECTION_ANIMATION_CYCLE_DURATION) * std::f32::consts::PI * 2.0;
    let anim_t = (anim_t_unscaled.sin() + 1.0) / 2.0;
    if item_h > 0.0 {
        let num_entries = state.entries.len();
        for i_slot in 0..NUM_WHEEL_ITEMS {
            let item_top_y = content_area_y_start + WHEEL_ITEM_GAP + (i_slot as f32 * (item_h + WHEEL_ITEM_GAP));
            let is_selected_slot = i_slot == CENTER_WHEEL_SLOT_INDEX;
            let (display_text, box_color, text_color, text_x_pos, song_count) = if num_entries > 0 {
                let list_index = (state.selected_index as isize + i_slot as isize - CENTER_WHEEL_SLOT_INDEX as isize + num_entries as isize) as usize % num_entries;
                if let Some(entry) = state.entries.get(list_index) {
                    match entry {
                        MusicWheelEntry::Song(song_info) => {
                            let base_color = col_music_wheel_box();
                            let selected_color = col_selected_song_box();
                            let final_box_color = if is_selected_slot { lerp_color(base_color, selected_color, anim_t) } else { base_color };
                            (song_info.title.clone(), final_box_color, [1.0; 4], wheel_left_x + SONG_TEXT_LEFT_PADDING, None)
                        }
                        MusicWheelEntry::PackHeader { name, original_index } => {
                            let song_cache = get_song_cache();
                            let count = song_cache.iter().find(|p| &p.name == name).map(|p| p.songs.len()).unwrap_or(0);
                            let base_color = col_pack_header_box();
                            let selected_color = col_selected_pack_header_box();
                            let final_box_color = if is_selected_slot { lerp_color(base_color, selected_color, anim_t) } else { base_color };
                            let text_x = wheel_left_x + 0.5 * WHEEL_W;
                            let pack_color = color::simply_love_rgba(state.active_color_index + *original_index as i32);
                            (name.clone(), final_box_color, pack_color, text_x, Some(count))
                        }
                    }
                } else { ("".to_string(), col_music_wheel_box(), [1.0; 4], wheel_left_x, None) }
            } else { ("".to_string(), col_music_wheel_box(), [1.0; 4], wheel_left_x, None) };
            actors.push(act!(quad: align(0.0, 0.0): xy(wheel_left_x, item_top_y): zoomto(WHEEL_W, item_h): diffuse(box_color[0], box_color[1], box_color[2], 1.0): z(z_layer) ));
            let is_pack = song_count.is_some();
            if is_pack {
                actors.push(act!(text: align(0.5, 0.5): xy(text_x_pos, item_top_y + 0.5 * item_h): zoomtoheight(MUSIC_WHEEL_TEXT_TARGET_PX): font("miso"): settext(display_text): horizalign(center): diffuse(text_color[0], text_color[1], text_color[2], 1.0): z(z_layer + 1) ));
            } else {
                actors.push(act!(text: align(0.0, 0.5): xy(text_x_pos, item_top_y + 0.5 * item_h): zoomtoheight(MUSIC_WHEEL_TEXT_TARGET_PX): font("miso"): settext(display_text): horizalign(left): diffuse(text_color[0], text_color[1], text_color[2], 1.0): z(z_layer + 1) ));
            }
            if let Some(count) = song_count {
                if count > 0 {
                    actors.push(act!(text: align(1.0, 0.5): xy(wheel_right_x - PACK_COUNT_RIGHT_PADDING, item_top_y + 0.5 * item_h): zoomtoheight(PACK_COUNT_TEXT_TARGET_PX): font("miso"): settext(format!("{}", count)): horizalign(right): diffuse(1.0, 1.0, 1.0, 0.8): z(z_layer + 1) ));
                }
            }
        }
    }

    // 5. Draw placeholder text in the "STEP STATS" box
    let pad_x = 10.0;
    let pad_y = 8.0;
    actors.push(act!(text: align(0.0, 0.0): xy(pad_x, box_top + pad_y): zoomtoheight(15.0): font("miso"): settext("STEP STATS"): horizalign(left): diffuse(1.0, 1.0, 1.0, 1.0): z(51) ));

    actors
}
