use crate::act;
use crate::core::space::globals::*;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::color;
use crate::ui::components::heart_bg;
use crate::ui::components::screen_bar::{self, ScreenBarParams, ScreenBarPosition, ScreenBarTitlePlacement};
use std::collections::HashSet;
use std::sync::{Arc, LazyLock};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use log::info;
use std::fs;

/* ---------------------------- transitions ---------------------------- */
const TRANSITION_IN_DURATION: f32 = 0.5;
const TRANSITION_OUT_DURATION: f32 = 0.3;

use crate::core::song_loading::{SongData, get_song_cache, ChartData, SongPack};

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

// --- NEW CONSTANTS FOR BEHAVIORS ---
const DOUBLE_TAP_WINDOW: Duration = Duration::from_millis(300);
const NAV_INITIAL_HOLD_DELAY: Duration = Duration::from_millis(250);
const NAV_REPEAT_SCROLL_INTERVAL: Duration = Duration::from_millis(80);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum NavDirection { Left, Right }

#[derive(Clone, Debug)]
pub enum MusicWheelEntry {
    PackHeader { name: String, original_index: usize, banner_path: Option<PathBuf> },
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
    pub last_requested_banner_path: Option<PathBuf>,
    pub current_banner_key: String,
    pub last_requested_chart_hash: Option<String>,
    pub current_graph_key: String,
    // --- NEW STATE FOR BEHAVIORS ---
    pub active_chord_keys: HashSet<KeyCode>,
    pub last_difficulty_nav_key: Option<KeyCode>,
    pub last_difficulty_nav_time: Option<Instant>,
    pub nav_key_held_direction: Option<NavDirection>,
    pub nav_key_held_since: Option<Instant>,
    pub nav_key_last_scrolled_at: Option<Instant>,
}

/// Helper function to check if a specific difficulty index has a playable chart
pub(crate) fn is_difficulty_playable(song: &Arc<SongData>, difficulty_index: usize) -> bool {
    if difficulty_index >= DIFFICULTY_NAMES.len() { return false; }
    let target_difficulty_name = DIFFICULTY_NAMES[difficulty_index];
    // Our parser doesn't expose stepstype, but for now we assume all charts are dance-single.
    // We check if notes exist.
    song.charts.iter().any(|c| {
        c.difficulty.eq_ignore_ascii_case(target_difficulty_name) && !c.notes.is_empty()
    })
}

fn find_pack_banner(pack: &SongPack) -> Option<PathBuf> {
    // We need to determine the pack's directory. We can get it from the first song.
    let Some(first_song) = pack.songs.first() else { return None; };
    let Some(song_folder) = first_song.banner_path.as_ref().and_then(|p| p.parent()) else { return None; };
    let Some(pack_folder_path) = song_folder.parent() else { return None; };

    if !pack_folder_path.is_dir() { return None; }
    
    let banner_name_patterns = ["banner", "bn", "ban"];
    for pattern in banner_name_patterns {
        let entries = fs::read_dir(pack_folder_path).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(filename_str) = path.file_name().and_then(|s| s.to_str()) {
                    let filename_lower = filename_str.to_lowercase();
                    if filename_lower.contains(pattern) && (filename_lower.ends_with(".png") || filename_lower.ends_with(".jpg")) {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}

fn rebuild_displayed_entries(state: &mut State) {
    let mut new_entries = Vec::new();
    let mut current_pack_name: Option<String> = None;
    for entry in &state.all_entries {
        match entry {
            MusicWheelEntry::PackHeader { .. } => {
                current_pack_name = if let MusicWheelEntry::PackHeader { name, .. } = entry { Some(name.clone()) } else { None };
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
            banner_path: find_pack_banner(pack),
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
        last_requested_banner_path: None,
        current_banner_key: "banner1.png".to_string(),
        last_requested_chart_hash: None,
        current_graph_key: "__white".to_string(),
        // --- INITIALIZE NEW STATE ---
        active_chord_keys: HashSet::new(),
        last_difficulty_nav_key: None,
        last_difficulty_nav_time: None,
        nav_key_held_direction: None,
        nav_key_held_since: None,
        nav_key_last_scrolled_at: None,
    };

    rebuild_displayed_entries(&mut state);
    state
}

fn lerp_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t, a[3] + (b[3] - a[3]) * t,
    ]
}

pub fn handle_key_press(state: &mut State, event: &KeyEvent) -> ScreenAction {
    let num_entries = state.entries.len();
    let is_song_selected = num_entries > 0
        && matches!(state.entries.get(state.selected_index), Some(MusicWheelEntry::Song(_)));
    
    let Some(PhysicalKey::Code(key_code)) = Some(event.physical_key) else { return ScreenAction::None; };

    if event.state == ElementState::Pressed {
        if matches!(key_code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            state.active_chord_keys.insert(key_code);
        }

        if !event.repeat {
            let mut combo_action_taken = false;
            // Chord to collapse pack
            if state.active_chord_keys.contains(&KeyCode::ArrowUp) && state.active_chord_keys.contains(&KeyCode::ArrowDown) {
                if state.expanded_pack_name.is_some() {
                    info!("Up+Down combo: Collapsing pack.");
                    state.expanded_pack_name = None;
                    rebuild_displayed_entries(state); // Rebuild is important
                    // Sound effect would go here
                    combo_action_taken = true;
                }
            }

            if combo_action_taken { return ScreenAction::None; }

            match key_code {
                KeyCode::ArrowRight | KeyCode::KeyD => {
                    if num_entries > 0 {
                        state.selected_index = (state.selected_index + 1) % num_entries;
                        state.selection_animation_timer = 0.0;
                        state.nav_key_held_direction = Some(NavDirection::Right);
                        state.nav_key_held_since = Some(Instant::now());
                        state.nav_key_last_scrolled_at = Some(Instant::now());
                    }
                }
                KeyCode::ArrowLeft | KeyCode::KeyA => {
                    if num_entries > 0 {
                        state.selected_index = (state.selected_index + num_entries - 1) % num_entries;
                        state.selection_animation_timer = 0.0;
                        state.nav_key_held_direction = Some(NavDirection::Left);
                        state.nav_key_held_since = Some(Instant::now());
                        state.nav_key_last_scrolled_at = Some(Instant::now());
                    }
                }
                KeyCode::ArrowUp | KeyCode::KeyW => {
                    if is_song_selected {
                        let now = Instant::now();
                        if state.last_difficulty_nav_key == Some(key_code) && state.last_difficulty_nav_time.map_or(false, |t| now.duration_since(t) < DOUBLE_TAP_WINDOW) {
                            if let Some(MusicWheelEntry::Song(song)) = state.entries.get(state.selected_index) {
                                let mut new_idx = state.selected_difficulty_index;
                                while new_idx > 0 {
                                    new_idx -= 1;
                                    if is_difficulty_playable(song, new_idx) {
                                        state.selected_difficulty_index = new_idx;
                                        break;
                                    }
                                }
                            }
                            state.last_difficulty_nav_key = None;
                            state.last_difficulty_nav_time = None;
                        } else {
                            state.last_difficulty_nav_key = Some(key_code);
                            state.last_difficulty_nav_time = Some(now);
                        }
                    }
                }
                KeyCode::ArrowDown | KeyCode::KeyS => {
                    if is_song_selected {
                        let now = Instant::now();
                        if state.last_difficulty_nav_key == Some(key_code) && state.last_difficulty_nav_time.map_or(false, |t| now.duration_since(t) < DOUBLE_TAP_WINDOW) {
                            if let Some(MusicWheelEntry::Song(song)) = state.entries.get(state.selected_index) {
                                let mut new_idx = state.selected_difficulty_index;
                                while new_idx < DIFFICULTY_NAMES.len() - 1 {
                                    new_idx += 1;
                                    if is_difficulty_playable(song, new_idx) {
                                        state.selected_difficulty_index = new_idx;
                                        break;
                                    }
                                }
                            }
                            state.last_difficulty_nav_key = None;
                            state.last_difficulty_nav_time = None;
                        } else {
                            state.last_difficulty_nav_key = Some(key_code);
                            state.last_difficulty_nav_time = Some(now);
                        }
                    }
                }
                KeyCode::Enter => {
                    if let Some(entry) = state.entries.get(state.selected_index).cloned() {
                        match entry {
                            MusicWheelEntry::Song(song) => {
                                info!("Selected song: '{}'. It has {} charts.", song.title, song.charts.len());
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
                KeyCode::Escape => return ScreenAction::Navigate(Screen::Menu),
                _ => {}
            }
        }
    } else if event.state == ElementState::Released {
        if matches!(key_code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            state.active_chord_keys.remove(&key_code);
        }
        if matches!(key_code, KeyCode::ArrowLeft | KeyCode::KeyA | KeyCode::ArrowRight | KeyCode::KeyD) {
            state.nav_key_held_direction = None;
            state.nav_key_held_since = None;
            state.nav_key_last_scrolled_at = None;
        }
    }
    ScreenAction::None
}

pub fn update(state: &mut State, dt: f32) -> ScreenAction {
    state.selection_animation_timer += dt;
    if state.selection_animation_timer > SELECTION_ANIMATION_CYCLE_DURATION {
        state.selection_animation_timer -= SELECTION_ANIMATION_CYCLE_DURATION;
    }
    
    // Auto-scroll logic
    if let (Some(direction), Some(held_since), Some(last_scrolled_at)) =
        (state.nav_key_held_direction.clone(), state.nav_key_held_since, state.nav_key_last_scrolled_at)
    {
        let now = Instant::now();
        if now.duration_since(held_since) > NAV_INITIAL_HOLD_DELAY {
            if now.duration_since(last_scrolled_at) >= NAV_REPEAT_SCROLL_INTERVAL {
                let num_entries = state.entries.len();
                if num_entries > 0 {
                    match direction {
                        NavDirection::Left => state.selected_index = (state.selected_index + num_entries - 1) % num_entries,
                        NavDirection::Right => state.selected_index = (state.selected_index + 1) % num_entries,
                    }
                    state.nav_key_last_scrolled_at = Some(now);
                }
            }
        }
    }


    let (selected_song, selected_pack) = if let Some(entry) = state.entries.get(state.selected_index) {
        match entry {
            MusicWheelEntry::Song(song) => (Some(song.clone()), None),
            MusicWheelEntry::PackHeader { name, banner_path, .. } => (None, Some((name.clone(), banner_path.clone()))),
        }
    } else {
        (None, None)
    };

    // Auto-adjust difficulty
    if let Some(song) = &selected_song {
        if !is_difficulty_playable(song, state.selected_difficulty_index) {
            for i in 0..DIFFICULTY_NAMES.len() {
                if is_difficulty_playable(song, i) {
                    state.selected_difficulty_index = i;
                    break;
                }
            }
        }
    }

    let chart_to_display = selected_song.as_ref().and_then(|song| {
        let difficulty_name = DIFFICULTY_NAMES[state.selected_difficulty_index];
        song.charts.iter().find(|c| c.difficulty.eq_ignore_ascii_case(difficulty_name)).cloned()
    });
    
    let new_chart_hash = chart_to_display.as_ref().map(|c| c.short_hash.clone());
    if state.last_requested_chart_hash != new_chart_hash {
        state.last_requested_chart_hash = new_chart_hash;
        return ScreenAction::RequestDensityGraph(chart_to_display);
    }
    
    let new_banner_path = selected_song.as_ref().and_then(|s| s.banner_path.clone()).or_else(|| selected_pack.and_then(|(_, path)| path));
    if state.last_requested_banner_path != new_banner_path {
        state.last_requested_banner_path = new_banner_path.clone();
        return ScreenAction::RequestBanner(new_banner_path);
    }

    ScreenAction::None
}

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

pub fn get_actors(state: &State) -> Vec<Actor> {
    let mut actors = Vec::with_capacity(256);

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

    const BAR_H: f32 = 32.0;
    let accent = color::simply_love_rgba(state.active_color_index);
    let box_w = 373.8;
    let box_h = 60.0;
    let box_bottom = screen_height() - BAR_H;
    let box_top = box_bottom - box_h;

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
    
    let boxes_top_y = boxes_bottom_y - RATING_BOX_H;
    let boxes_left_x = step_info_box_right_x - STEP_INFO_BOX_W;
    let density_graph_top_y = boxes_top_y;

    actors.push(act!(quad:
        align(0.0, 0.0):
        xy(boxes_left_x, density_graph_top_y):
        zoomto(STEP_INFO_BOX_W, STEP_INFO_BOX_H):
        diffuse(UI_BOX_BG_COLOR[0], UI_BOX_BG_COLOR[1], UI_BOX_BG_COLOR[2], 1.0):
        z(z_layer)
    ));
    actors.push(act!(sprite(state.current_graph_key.as_str()):
        align(0.0, 0.0):
        xy(boxes_left_x, density_graph_top_y):
        zoomto(STEP_INFO_BOX_W, STEP_INFO_BOX_H):
        z(z_layer + 1)
    ));

    const STEP_ARTIST_BOX_W: f32 = 175.2;
    const STEP_ARTIST_BOX_H: f32 = 16.8;
    const GAP_ABOVE_DENSITY: f32 = 1.0;
    let step_artist_box_color = color::simply_love_rgba(state.active_color_index);
    let step_artist_box_bottom_y = density_graph_top_y - GAP_ABOVE_DENSITY;
    actors.push(act!(quad:
        align(0.0, 1.0):
        xy(boxes_left_x, step_artist_box_bottom_y):
        zoomto(STEP_ARTIST_BOX_W, STEP_ARTIST_BOX_H):
        diffuse(step_artist_box_color[0], step_artist_box_color[1], step_artist_box_color[2], 1.0):
        z(z_layer)
    ));

    const BANNER_BOX_W: f32 = 319.8;
    const BANNER_BOX_H: f32 = 126.0;
    const GAP_BELOW_TOP_BAR: f32 = 1.0;
    let banner_box_top_y = BAR_H + GAP_BELOW_TOP_BAR;
    let banner_box_left_x = boxes_left_x;
    
    let selected_entry = state.entries.get(state.selected_index);
    let banner_key_to_draw = &state.current_banner_key;
    actors.push(act!(sprite(banner_key_to_draw.clone()): align(0.0, 0.0): xy(banner_box_left_x, banner_box_top_y): zoomto(BANNER_BOX_W, BANNER_BOX_H): z(z_layer) ));

    let rating_box_left_x = rating_box_right_x - RATING_BOX_W;
    const INNER_BOX_SIZE: f32 = 28.2;
    const HORIZONTAL_PADDING: f32 = 1.8;
    const VERTICAL_GAP: f32 = 1.8;

    for i in 0..DIFFICULTY_NAMES.len() {
        let inner_box_x = rating_box_left_x + HORIZONTAL_PADDING;
        let inner_box_y = boxes_top_y + VERTICAL_GAP + (i as f32 * (INNER_BOX_SIZE + VERTICAL_GAP));

        actors.push(act!(quad:
            align(0.0, 0.0): xy(inner_box_x, inner_box_y): zoomto(INNER_BOX_SIZE, INNER_BOX_SIZE):
            diffuse(DIFFICULTY_DISPLAY_INNER_BOX_COLOR[0], DIFFICULTY_DISPLAY_INNER_BOX_COLOR[1], DIFFICULTY_DISPLAY_INNER_BOX_COLOR[2], 1.0):
            z(z_layer + 1)
        ));

        if let Some(MusicWheelEntry::Song(song)) = selected_entry {
            if is_difficulty_playable(song, i) {
                if let Some(chart) = song.charts.iter().find(|c| c.difficulty.eq_ignore_ascii_case(DIFFICULTY_NAMES[i])) {
                    if chart.meter > 0 {
                        let color_offset = (DIFFICULTY_NAMES.len() - 1 - i) as i32;
                        let text_color = color::simply_love_rgba(state.active_color_index - color_offset);
                        const METER_TEXT_PX: f32 = 20.0;
                        let text_center_x = inner_box_x + 0.5 * INNER_BOX_SIZE;
                        let text_center_y = inner_box_y + 0.5 * INNER_BOX_SIZE;
                        actors.push(act!(text:
                            align(0.5, 0.5): xy(text_center_x, text_center_y): zoomtoheight(METER_TEXT_PX):
                            font("wendy"): settext(format!("{}", chart.meter)): horizalign(center):
                            diffuse(text_color[0], text_color[1], text_color[2], 1.0): z(z_layer + 2)
                        ));
                    }
                }
            }
        }
    }

    const BPM_BOX_H: f32 = 49.8;
    const GAP_BELOW_BANNER: f32 = 1.0;
    let bpm_box_top_y = banner_box_top_y + BANNER_BOX_H + GAP_BELOW_BANNER;
    actors.push(act!(quad: align(0.0, 0.0): xy(banner_box_left_x, bpm_box_top_y): zoomto(BANNER_BOX_W, BPM_BOX_H): diffuse(UI_BOX_BG_COLOR[0], UI_BOX_BG_COLOR[1], UI_BOX_BG_COLOR[2], 1.0): z(z_layer) ));

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
                        MusicWheelEntry::PackHeader { name, original_index, .. } => {
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

    let pad_x = 10.0;
    let pad_y = 8.0;
    actors.push(act!(text: align(0.0, 0.0): xy(pad_x, box_top + pad_y): zoomtoheight(15.0): font("miso"): settext("STEP STATS"): horizalign(left): diffuse(1.0, 1.0, 1.0, 1.0): z(51) ));

    actors
}
