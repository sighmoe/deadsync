use crate::act;
use crate::core::audio;
use crate::core::space::globals::*;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::color;
use crate::ui::components::{heart_bg, pad_display, music_wheel};
use crate::ui::components::screen_bar::{self, ScreenBarParams, ScreenBarPosition, ScreenBarTitlePlacement};
use crate::ui::actors::SizeSpec;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use crate::core::font;
use log::info;
use std::fs;

// --- engine imports ---
use crate::core::space::{is_wide, widescale};
use crate::core::song_loading::{SongData, get_song_cache, ChartData, SongPack};


/* ---------------------------- transitions ---------------------------- */
const TRANSITION_IN_DURATION: f32 = 0.5;
const TRANSITION_OUT_DURATION: f32 = 0.3;

// --- THEME LAYOUT CONSTANTS (unscaled, native dimensions) ---
const BANNER_NATIVE_WIDTH: f32 = 418.0;
const BANNER_NATIVE_HEIGHT: f32 = 164.0;

// --- Other UI Constants ---
static UI_BOX_BG_COLOR: LazyLock<[f32; 4]> = LazyLock::new(|| color::rgba_hex("#1E282F"));
static DIFFICULTY_DISPLAY_INNER_BOX_COLOR: LazyLock<[f32; 4]> = LazyLock::new(|| color::rgba_hex("#0f0f0f"));
pub const DIFFICULTY_NAMES: [&str; 5] = ["Beginner", "Easy", "Medium", "Hard", "Challenge"];
const SELECTION_ANIMATION_CYCLE_DURATION: f32 = 1.0;
const DOUBLE_TAP_WINDOW: Duration = Duration::from_millis(300);
const NAV_INITIAL_HOLD_DELAY: Duration = Duration::from_millis(200);
const NAV_REPEAT_SCROLL_INTERVAL: Duration = Duration::from_millis(40);
const PREVIEW_DELAY_SECONDS: f32 = 0.25;

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
    pub preferred_difficulty_index: usize,
    pub active_color_index: i32,
    pub selection_animation_timer: f32,
    pub expanded_pack_name: Option<String>,
    bg: heart_bg::State,
    pub last_requested_banner_path: Option<PathBuf>,
    pub current_banner_key: String,
    pub last_requested_chart_hash: Option<String>,
    pub current_graph_key: String,
    pub active_chord_keys: HashSet<KeyCode>,
    pub last_difficulty_nav_key: Option<KeyCode>,
    pub last_difficulty_nav_time: Option<Instant>,
    pub nav_key_held_direction: Option<NavDirection>,
    pub nav_key_held_since: Option<Instant>,
    pub nav_key_last_scrolled_at: Option<Instant>,
    pub currently_playing_preview_path: Option<PathBuf>,
    pub session_elapsed: f32,
    prev_selected_index: usize,
    pub time_since_selection_change: f32,
}

// ... (init, handle_key_press, update, etc. functions remain unchanged) ...
/// Helper function to check if a specific difficulty index has a playable chart
pub(crate) fn is_difficulty_playable(song: &Arc<SongData>, difficulty_index: usize) -> bool {
    if difficulty_index >= DIFFICULTY_NAMES.len() { return false; }
    let target_difficulty_name = DIFFICULTY_NAMES[difficulty_index];
    song.charts.iter().any(|c| {
        c.difficulty.eq_ignore_ascii_case(target_difficulty_name) && !c.notes.is_empty()
    })
}

fn find_pack_banner(pack: &SongPack) -> Option<PathBuf> {
    let Some(first_song) = pack.songs.first() else { return None; };
    // A song's banner_path might not exist. music_path is more reliable for finding its folder.
    let song_folder = first_song.music_path.as_ref()
        .or(first_song.banner_path.as_ref())
        .or(first_song.background_path.as_ref())
        .and_then(|p| p.parent());
        
    let Some(song_folder) = song_folder else { return None; };
    let Some(pack_folder_path) = song_folder.parent() else { return None; };

    if !pack_folder_path.is_dir() { return None; }
    
    // --- Step 1: Collect all image files in the pack directory ---
    let Ok(entries) = fs::read_dir(pack_folder_path) else { return None; };
    
    let image_files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            if !p.is_file() { return false; }
            p.extension()
                .and_then(|s| s.to_str())
                .map_or(false, |ext| {
                    let ext_lower = ext.to_lowercase();
                    ext_lower == "png" || ext_lower == "jpg" || ext_lower == "jpeg"
                })
        })
        .collect();

    if image_files.is_empty() { return None; }
    
    // --- Step 2: Search by filename hints (case-insensitive) ---
    // These hints are checked against the file stem (no extension).
    // A simple `contains` check for "bn" is more effective than a strict
    // `ends_with` and catches common variations like "-bn" or "abn".
    let name_hints = ["banner", "bn"];

    for path in &image_files {
        if let Some(filename_str) = path.file_stem().and_then(|s| s.to_str()) {
            let filename_lower = filename_str.to_lowercase();
            if name_hints.iter().any(|&hint| filename_lower.contains(hint)) {
                info!("Found pack banner by name hint: {:?}", path);
                return Some(path.clone());
            }
        }
    }

    // --- Step 3: Fallback to searching by image dimensions ---
    for path in &image_files {
        if let Ok((width, height)) = image::image_dimensions(path) {
            // Condition 1: Standard banner dimensions
            let is_standard_banner = (100..=320).contains(&width) && (50..=240).contains(&height);
            
            // Condition 2: Overlarge banner with a wide aspect ratio
            let is_overlarge_banner = width > 200 && height > 0 && (width as f32 / height as f32) > 2.0;

            if is_standard_banner || is_overlarge_banner {
                info!("Found pack banner by dimension hint: {:?}", path);
                return Some(path.clone());
            }
        }
    }

    // --- Step 4: No banner found ---
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
        preferred_difficulty_index: 2,
        active_color_index: color::DEFAULT_COLOR_INDEX,
        selection_animation_timer: 0.0,
        expanded_pack_name: None,
        bg: heart_bg::State::new(),
        last_requested_banner_path: None,
        current_banner_key: "banner1.png".to_string(),
        last_requested_chart_hash: None,
        current_graph_key: "__white".to_string(),
        active_chord_keys: HashSet::new(),
        last_difficulty_nav_key: None,
        last_difficulty_nav_time: None,
        nav_key_held_direction: None,
        nav_key_held_since: None,
        nav_key_last_scrolled_at: None,
        currently_playing_preview_path: None,
        session_elapsed: 0.0,
        prev_selected_index: 0,
        time_since_selection_change: 0.0,
    };

    rebuild_displayed_entries(&mut state);
    state.prev_selected_index = state.selected_index;
    state
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
            if state.active_chord_keys.contains(&KeyCode::ArrowUp) && state.active_chord_keys.contains(&KeyCode::ArrowDown) {
                // This combo collapses the currently open pack.
                if let Some(pack_to_collapse) = state.expanded_pack_name.clone() {
                    info!("Up+Down combo: Collapsing pack '{}'.", pack_to_collapse);
                    state.expanded_pack_name = None;
                    rebuild_displayed_entries(state);

                    // After collapsing, we must update the selected index to point to the
                    // header of the pack we were just in.
                    let new_selection_index = state.entries.iter().position(|e| {
                        if let MusicWheelEntry::PackHeader { name, .. } = e {
                            *name == pack_to_collapse
                        } else {
                            false
                        }
                    }).unwrap_or(0); // Fallback to 0 if the pack isn't found.

                    state.selected_index = new_selection_index;
                    // Sync previous index to prevent the 'change' sound from playing.
                    state.prev_selected_index = new_selection_index;
                    state.time_since_selection_change = 0.0;
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
                        state.time_since_selection_change = 0.0;
                    }
                }
                KeyCode::ArrowLeft | KeyCode::KeyA => {
                    if num_entries > 0 {
                        state.selected_index = (state.selected_index + num_entries - 1) % num_entries;
                        state.selection_animation_timer = 0.0;
                        state.nav_key_held_direction = Some(NavDirection::Left);
                        state.nav_key_held_since = Some(Instant::now());
                        state.nav_key_last_scrolled_at = Some(Instant::now());
                        state.time_since_selection_change = 0.0;
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
                                        state.preferred_difficulty_index = new_idx;
                                        audio::play_sfx("assets/sounds/easier.ogg");
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
                                        state.preferred_difficulty_index = new_idx;
                                        audio::play_sfx("assets/sounds/harder.ogg");
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
                                audio::play_sfx("assets/sounds/expand.ogg");
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
                                state.time_since_selection_change = 0.0;
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
    // Increment the timer every frame since the last selection change.
    state.time_since_selection_change += dt;
    
    // Handle the visual pulsing animation of the selected wheel item.
    state.selection_animation_timer += dt;
    if state.selection_animation_timer > SELECTION_ANIMATION_CYCLE_DURATION {
        state.selection_animation_timer -= SELECTION_ANIMATION_CYCLE_DURATION;
    }

    // Handle rapid scrolling when a navigation key is held down.
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
                    // Reset the preview delay timer each time we auto-scroll.
                    state.time_since_selection_change = 0.0;
                }
            }
        }
    }

    // --- Song/Difficulty Change Logic ---
    if state.selected_index != state.prev_selected_index {
        audio::play_sfx("assets/sounds/change.ogg");
        state.prev_selected_index = state.selected_index;
        state.time_since_selection_change = 0.0; // Reset preview timer on any change.

        // When the song changes, find the best matching difficulty based on the user's PREFERENCE.
        if let Some(MusicWheelEntry::Song(song)) = state.entries.get(state.selected_index) {
            let preferred_difficulty = state.preferred_difficulty_index; // Use the stored preference
            let mut best_match_index = None;
            let mut min_diff = i32::MAX;

            // Iterate through difficulties in canonical order (Beginner -> Challenge)
            // to ensure tie-breaking favors easier charts.
            for i in 0..DIFFICULTY_NAMES.len() {
                if is_difficulty_playable(song, i) {
                    let diff = (i as i32 - preferred_difficulty as i32).abs();
                    if diff < min_diff {
                        min_diff = diff;
                        best_match_index = Some(i);
                    }
                }
            }
            // Update the *current* selection, but NOT the preference.
            if let Some(best_index) = best_match_index { state.selected_difficulty_index = best_index; }
        }
    }    

    // Get the currently selected song or pack header.
    let (selected_song, selected_pack) = if let Some(entry) = state.entries.get(state.selected_index) {
        match entry {
            MusicWheelEntry::Song(song) => (Some(song.clone()), None),
            MusicWheelEntry::PackHeader { name, banner_path, .. } => (None, Some((name.clone(), banner_path.clone()))),
        }
    } else {
        (None, None)
    };

    // --- MUSIC PREVIEW LOGIC WITH DELAY ---
    if state.time_since_selection_change >= PREVIEW_DELAY_SECONDS {
        let music_path_for_preview = selected_song.as_ref().and_then(|s| s.music_path.clone());
        if state.currently_playing_preview_path != music_path_for_preview {
            state.currently_playing_preview_path = music_path_for_preview;

            let mut played = false;
            if let Some(song) = &selected_song {
                if let (Some(path), Some(start), Some(length)) = (&song.music_path, song.sample_start, song.sample_length) {
                    if length > 0.0 {
                        info!("Playing preview for '{}' at {:.2}s for {:.2}s", song.title, start, length);
                        let cut = audio::Cut { start_sec: start as f64, length_sec: length as f64 };
                        audio::play_music(path.clone(), cut, true);
                        played = true;
                    }
                }
            }
            if !played {
                audio::stop_music();
            }
        }
    } else if state.currently_playing_preview_path.is_some() {
        // If we haven't met the delay yet but music is playing, stop it.
        state.currently_playing_preview_path = None;
        audio::stop_music();
    }
    
    // --- DYNAMIC TEXTURE REQUEST LOGIC ---
    // Request a new density graph if the selected chart has changed.
    let chart_to_display = selected_song.as_ref().and_then(|song| {
        let difficulty_name = DIFFICULTY_NAMES[state.selected_difficulty_index];
        song.charts.iter().find(|c| c.difficulty.eq_ignore_ascii_case(difficulty_name)).cloned()
    });
    
    let new_chart_hash = chart_to_display.as_ref().map(|c| c.short_hash.clone());
    if state.last_requested_chart_hash != new_chart_hash {
        state.last_requested_chart_hash = new_chart_hash;
        return ScreenAction::RequestDensityGraph(chart_to_display);
    }
    
    // Request a new banner if the selected song or pack has changed.
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
        left_text: Some("PerfectTaste"), center_text: None, right_text: Some("PRESS START"),
    }));
 
    // Calculate the color for the currently selected difficulty based on the active theme color
    let selected_difficulty_color_index = state.active_color_index - (4 - state.selected_difficulty_index) as i32;
    let selected_difficulty_color = color::simply_love_rgba(selected_difficulty_color_index);

    // --- Build pack song counts for music wheel ---
    let mut pack_song_counts = HashMap::new();
    let song_cache = get_song_cache();
    for pack in song_cache.iter() {
        let count = pack.songs.iter().filter(|song| {
            song.charts.iter().any(|chart| chart.chart_type.eq_ignore_ascii_case("dance-single"))
        }).count();
        pack_song_counts.insert(pack.name.clone(), count);
    }

    // Session Timer, centered in the top bar.
    let timer_text = format_session_time(state.session_elapsed);

    actors.push(act!(text:
        font("wendy_monospace_numbers"):
        settext(timer_text):
        align(0.5, 0.5): // center, v-center
        xy(screen_center_x(), 10.0): // Slightly higher, matching Lua theme
        zoom(widescale(0.3, 0.36)):
        z(121): // Draw above the bar
        diffuse(1.0, 1.0, 1.0, 1.0):
        horizalign(center)
    ));

    // --- "ITG" text and Pads (top right), matching Simply Love layout ---
    {
        // "ITG" text, positioned to the left of the pads.
        let itg_text_x = screen_width() - widescale(55.0, 62.0);
        actors.push(act!(text:
            font("wendy"):
            settext("ITG"):
            align(1.0, 0.5): // right, v-center
            xy(itg_text_x, 15.0):
            zoom(widescale(0.5, 0.6)):
            z(121):
            diffuse(1.0, 1.0, 1.0, 1.0)
        ));

        // Calculate the final combined zoom for the pads.
        let final_pad_zoom = 0.24 * widescale(0.435, 0.525);

        // P1 Pad
        actors.push(pad_display::build(pad_display::PadDisplayParams {
            center_x: screen_width() - widescale(35.0, 41.0),
            center_y: widescale(22.0, 23.5),
            zoom: final_pad_zoom,
            z: 121,
            is_active: true,
        }));
        // P2 Pad
        actors.push(pad_display::build(pad_display::PadDisplayParams {
            center_x: screen_width() - widescale(15.0, 17.0),
            center_y: widescale(22.0, 23.5),
            zoom: final_pad_zoom,
            z: 121,
            is_active: false,
        }));
    }

    // --- BANNER (center-anchored like SL's ActorFrame) ---
    let (banner_zoom, banner_cx, banner_cy) = if is_wide() {
        (0.7655, screen_center_x() - 170.0, 96.0)
    } else {
        (0.75,   screen_center_x() - 166.0, 96.0) // <- keep -166 like the Lua
    };

    actors.push(act!(sprite(state.current_banner_key.clone()):
        align(0.5, 0.5):                 // <- match SL (center)
        xy(banner_cx, banner_cy):
        setsize(BANNER_NATIVE_WIDTH, BANNER_NATIVE_HEIGHT):
        zoom(banner_zoom):
        z(51)
    ));

// --- ARTIST / BPM / LENGTH INFO BOX (Verbatim Implementation) ---
    let (box_width, frame_x, frame_y) = if is_wide() {
        (320.0, screen_center_x() - 170.0, screen_center_y() - 55.0)
    } else {
        (310.0, screen_center_x() - 165.0, screen_center_y() - 55.0)
    };

    // Data for the box
    let selected_entry = state.entries.get(state.selected_index);
    let (artist_text, bpm_text, length_text) = if let Some(entry) = selected_entry {
        match entry {
            MusicWheelEntry::Song(song) => {
                let minutes = song.total_length_seconds / 60;
                let seconds = song.total_length_seconds % 60;
                let formatted_bpm = {
                    let min = song.min_bpm.round() as i32;
                    let max = song.max_bpm.round() as i32;
                    if (song.min_bpm - song.max_bpm).abs() < 1e-6 {
                        format!("{}", min)
                    } else {
                        format!("{} - {}", min, max)
                    }
                };
                (
                    song.artist.clone(),
                    formatted_bpm,
                    format!("{}:{:02}", minutes, seconds)
                )
            }
            MusicWheelEntry::PackHeader { original_index, .. } => {
                let total_length_sec = if let Some(pack) = song_cache.get(*original_index) {
                    pack.songs.iter().map(|s| s.total_length_seconds as u64).sum()
                } else {
                    0
                };
                ("".to_string(), "".to_string(), format_session_time(total_length_sec as f32))
            }
        }
    } else {
        // Fallback text for empty list
        ("".to_string(), "".to_string(), "".to_string())
    };

    let label_color = [0.5, 0.5, 0.5, 1.0];
    let value_color = [1.0, 1.0, 1.0, 1.0];

    let artist_max_w = box_width - 60.0;

    let main_frame = Actor::Frame {
        align: [0.0, 0.0],
        offset: [frame_x, frame_y],
        size: [SizeSpec::Px(box_width), SizeSpec::Px(50.0)],
        background: None,
        z: 51,
        children: vec![
            // Background Quad
            act!(quad:
                setsize(box_width, 50.0):
                diffuse(UI_BOX_BG_COLOR[0], UI_BOX_BG_COLOR[1], UI_BOX_BG_COLOR[2], UI_BOX_BG_COLOR[3])
            ),
            // Inner Frame for Text (unchanged structure)
            Actor::Frame {
                align: [0.0, 0.0],
                offset: [-110.0, -6.0],
                size: [SizeSpec::Fill, SizeSpec::Fill],
                background: None,
                z: 0,
                children: vec![
                    // --- Artist ---
                    act!(text: font("miso"): settext("ARTIST"):
                        align(1.0, 0.0): y(-11.0):
                        maxwidth(44.0):
                        diffuse(label_color[0], label_color[1], label_color[2], label_color[3]):
                        z(52)
                    ),
                    act!(text: font("miso"): settext(artist_text):
                        align(0.0, 0.0): xy(5.0, -11.0):
                        maxwidth(artist_max_w): // maxwidth is applied before final size calc
                        zoomtoheight(15.0):     // Enforce a consistent height for alignment
                        diffuse(value_color[0], value_color[1], value_color[2], value_color[3]):
                        z(52)
                    ),

                    // --- BPM ---
                    act!(text: font("miso"): settext("BPM"):
                        align(1.0, 0.0): y(10.0):
                        diffuse(label_color[0], label_color[1], label_color[2], label_color[3]):
                        z(52)
                    ),
                    act!(text: font("miso"): settext(bpm_text):
                        align(0.0, 0.0): xy(5.0, 10.0):
                        zoomtoheight(15.0):     // Base height for alignment
                        diffuse(value_color[0], value_color[1], value_color[2], value_color[3]):
                        z(52)
                    ),

                    // --- Length ---
                    act!(text: font("miso"): settext("LENGTH"):
                        align(1.0, 0.0): xy(box_width - 130.0, 10.0):
                        diffuse(label_color[0], label_color[1], label_color[2], label_color[3]):
                        z(52)
                    ),
                    act!(text: font("miso"): settext(length_text):
                        align(0.0, 0.0): xy(box_width - 125.0, 10.0):
                        zoomtoheight(15.0):     // Enforce a consistent height for alignment
                        diffuse(value_color[0], value_color[1], value_color[2], value_color[3]):
                        z(52)
                    ),
                ],
            },
        ],
    };

    actors.push(main_frame);

    // --- Get data for the selected chart ---
        let selected_entry = state.entries.get(state.selected_index);
        let selected_chart_data = if let Some(MusicWheelEntry::Song(song)) = selected_entry {
            song.charts.iter().find(|c| c.difficulty.eq_ignore_ascii_case(DIFFICULTY_NAMES[state.selected_difficulty_index])).cloned()
        } else {
            None
        };

        let step_artist_text = selected_chart_data.as_ref().map_or("".to_string(), |c| c.step_artist.clone());
        let peak_nps_text = selected_chart_data.as_ref().map_or("Peak NPS: --".to_string(), |c| format!("Peak NPS: {:.1}", c.max_nps));
        let breakdown_text = if let Some(chart) = &selected_chart_data {
            font::with_font("miso", |miso_font| {
                let panel_w = if is_wide() { 286.0 } else { 276.0 };
                let text_zoom = 0.8;
                let horizontal_padding = 16.0; // 8px padding on each side
                let max_allowed_width = panel_w - horizontal_padding;

                let check_width = |text: &str| {
                    let logical_width = font::measure_line_width_logical(miso_font, text) as f32;
                    let final_width = logical_width * text_zoom;
                    final_width <= max_allowed_width
                };
        
                if check_width(&chart.detailed_breakdown) {
                    chart.detailed_breakdown.clone()
                } else if check_width(&chart.partial_breakdown) {
                    chart.partial_breakdown.clone()
                } else if check_width(&chart.simple_breakdown) {
                    chart.simple_breakdown.clone()
                } else {
                    format!("{} Total", chart.total_streams)
                }
            }).unwrap_or_else(|| chart.simple_breakdown.clone()) // Fallback if font isn't found
        } else {
            "".to_string()
        };

    // --- Get stats for the PaneDisplay ---
    let (steps_text, jumps_text, holds_text, mines_text, hands_text, rolls_text) =
        if let Some(chart) = &selected_chart_data {
            (
                chart.stats.total_steps.to_string(),
                chart.stats.jumps.to_string(),
                chart.stats.holds.to_string(),
                chart.stats.mines.to_string(),
                chart.stats.hands.to_string(),
                chart.stats.rolls.to_string(),
            )
        } else {
            (
                "?".to_string(), "?".to_string(), "?".to_string(),
                "?".to_string(), "?".to_string(), "?".to_string(),
            )
        };

// --- Step credit panel (P1, song mode) — positioned directly above the density graph ---

    // The component's bottom edge should align with the density graph's top edge.
    // Density Graph Top Y = (screen_center_y() + 23.0) - (64.0 / 2.0) = screen_center_y() - 9.0
    let graph_top_y = screen_center_y() - 9.0;
    
    // This component's height is defined in SL as screen.h / 28
    let component_h = screen_height() / 28.0;

    // The center of this component will be its height / 2 above the graph's top.
    let y_center = graph_top_y - (0.5 * component_h);

    if is_wide() {
        // --- Widescreen Layout (X positions from StepArtistAF.lua) ---
        let quad_cx = screen_center_x() - 243.0; // Lua: (cx-356) + 113
        let steps_label_x = screen_center_x() - 326.0; // Lua: (cx-356) + 30
        let artist_text_x = screen_center_x() - 281.0; // Lua: (cx-356) + 75

        // Background quad
        actors.push(act!(quad:
            align(0.5, 0.5):
            xy(quad_cx, y_center):
            setsize(175.0, component_h):
            z(120):
            diffuse(selected_difficulty_color[0], selected_difficulty_color[1], selected_difficulty_color[2], 1.0)
        ));

        // "STEPS" label
        actors.push(act!(text:
            font("miso"):
            settext("STEPS"):
            align(0.0, 0.5):
            xy(steps_label_x, y_center):
            zoom(0.8):
            maxwidth(40.0):
            z(121):
            diffuse(0.0, 0.0, 0.0, 1.0)
        ));

        // Step artist text
        actors.push(act!(text:
            font("miso"):
            settext(step_artist_text):
            align(0.0, 0.5):
            xy(artist_text_x, y_center):
            zoom(0.8):
            maxwidth(124.0):
            z(121):
            diffuse(0.0, 0.0, 0.0, 1.0)
        ));
    } else {
        // --- 4:3 Layout (X positions from StepArtistAF.lua) ---
        let quad_cx = screen_center_x() - 233.0; // Lua: (cx-346) + 113
        let steps_label_x = screen_center_x() - 316.0; // Lua: (cx-346) + 30
        let artist_text_x = screen_center_x() - 271.0; // Lua: (cx-346) + 75

        // Background quad
        actors.push(act!(quad:
            align(0.5, 0.5):
            xy(quad_cx, y_center):
            setsize(175.0, component_h):
            z(120):
            diffuse(selected_difficulty_color[0], selected_difficulty_color[1], selected_difficulty_color[2], 1.0)
        ));

        // "STEPS" label
        actors.push(act!(text:
            font("miso"):
            settext("STEPS"):
            align(0.0, 0.5):
            xy(steps_label_x, y_center):
            zoom(0.8):
            maxwidth(40.0):
            z(121):
            diffuse(0.0, 0.0, 0.0, 1.0)
        ));

        // Step artist text
        actors.push(act!(text:
            font("miso"):
            settext(step_artist_text):
            align(0.0, 0.5):
            xy(artist_text_x, y_center):
            zoom(0.8):
            maxwidth(124.0):
            z(121):
            diffuse(0.0, 0.0, 0.0, 1.0)
        ));
    }

    // --- Density graph panel (SL 1:1, Player 1, top-left anchored) ---
    // NEW: detect if a pack is selected
    let is_pack_selected =
        matches!(state.entries.get(state.selected_index), Some(MusicWheelEntry::PackHeader { .. }));

    let panel_w = if is_wide() { 286.0 } else { 276.0 };
    let panel_h = 64.0;

    let mut graph_children: Vec<Actor> = Vec::new();

    // Background quad (#1e282f), always drawn and exactly panel-sized
    graph_children.push(act!(quad:
        align(0.0, 0.0):
        xy(0.0, 0.0):
        setsize(panel_w, panel_h):
        diffuse(UI_BOX_BG_COLOR[0], UI_BOX_BG_COLOR[1], UI_BOX_BG_COLOR[2], UI_BOX_BG_COLOR[3])
    ));

    // Only draw the graph sprite + labels + breakdown when a SONG is selected
    if !is_pack_selected {
        // Density graph image fills the panel
        graph_children.push(act!(sprite(state.current_graph_key.clone()):
            align(0.0, 0.0):
            xy(0.0, 0.0):
            setsize(panel_w, panel_h)
        ));

        // Peak NPS text
        graph_children.push(act!(text: font("miso"): settext(peak_nps_text):
            align(0.0, 0.5):
            xy(0.5 * panel_w + 60.0, 0.5 * panel_h - 41.0):
            zoom(0.8):
            diffuse(1.0, 1.0, 1.0, 1.0)
        ));

        // Breakdown strip + centered text at the bottom of the panel
        graph_children.push(act!(quad:
            align(0.0, 0.0):
            xy(0.0, panel_h - 17.0):
            setsize(panel_w, 17.0):
            diffuse(0.0, 0.0, 0.0, 0.5)
        ));
        graph_children.push(act!(text: font("miso"): settext(breakdown_text):
            align(0.5, 0.5):
            xy(0.5 * panel_w, panel_h - 17.0 + 8.5):
            zoom(0.8):
            maxheight(15.0) // Use maxheight to ensure it doesn't overflow, but width is handled by string selection.
        ));
    }

    let density_graph_panel = Actor::Frame {
        align: [0.0, 0.0],
        offset: [
            (screen_center_x() - 182.0 - if is_wide() { 5.0 } else { 0.0 }) - 0.5 * panel_w,
            (screen_center_y() + 23.0) - 0.5 * panel_h,
        ],
        size: [SizeSpec::Px(panel_w), SizeSpec::Px(panel_h)],
        background: None,
        z: 51,
        children: graph_children,
    };
    actors.push(density_graph_panel);


    // --- PaneDisplay (P1) just above footer — absolute placement, SL 1:1 layout ---

    // pane anchor (center-top), same as SL
    let pane_cx = screen_width() * 0.25 - 5.0;
    let pane_top = screen_height() - 32.0 - 60.0;

    // background bar — spans half-screen minus 10, height 60
    actors.push(act!(quad:
        align(0.5, 0.0):
        xy(pane_cx, pane_top):
        setsize(screen_width() / 2.0 - 10.0, 60.0):
        z(120):
        diffuse(selected_difficulty_color[0], selected_difficulty_color[1], selected_difficulty_color[2], 1.0)
    ));

    // --- Stats Grid, High Scores, and Meter (SL Parity) ---
    {
        let text_zoom = widescale(0.8, 0.9);
        let cols_x = [
            widescale(-104.0, -133.0), // col 1
            widescale(-36.0, -38.0),   // col 2
            widescale(54.0, 76.0),     // col 3
            widescale(150.0, 190.0),   // col 4
        ];
        let rows_y = [13.0, 31.0, 49.0];

        // --- Main Stats Grid ---
        let items = [
            ("Steps", &steps_text), ("Mines", &mines_text),
            ("Jumps", &jumps_text), ("Hands", &hands_text),
            ("Holds", &holds_text), ("Rolls", &rolls_text),
        ];

        for (i, (label, value)) in items.iter().enumerate() {
            let (col_idx, row_idx) = (i % 2, i / 2);
            let (col_x, row_y) = (cols_x[col_idx], rows_y[row_idx]);

            // Stat value (right-aligned)
            actors.push(act!(text: font("miso"): settext(*value):
                align(1.0, 0.5): xy(pane_cx + col_x, pane_top + row_y):
                zoom(text_zoom): z(121): diffuse(0.0, 0.0, 0.0, 1.0)
            ));
            // Stat label (left-aligned, +3px offset)
            actors.push(act!(text: font("miso"): settext(*label):
                align(0.0, 0.5): xy(pane_cx + col_x + 3.0, pane_top + row_y):
                zoom(text_zoom): z(121): diffuse(0.0, 0.0, 0.0, 1.0)
            ));
        }
        
        // --- High Score Placeholders ---
        // Machine High Score
        actors.push(act!(text: font("miso"): settext("----"):
            align(0.5, 0.5): // Centered, like default BitmapText in SM
            xy(pane_cx + cols_x[2] - (50.0 * text_zoom), pane_top + rows_y[0]):
            maxwidth(30.0): zoom(text_zoom): z(121): diffuse(0.0, 0.0, 0.0, 1.0)
        ));
        actors.push(act!(text: font("miso"): settext("??.??%"):
            align(1.0, 0.5): // Right-aligned
            xy(pane_cx + cols_x[2] + (25.0 * text_zoom), pane_top + rows_y[0]):
            zoom(text_zoom): z(121): diffuse(0.0, 0.0, 0.0, 1.0)
        ));

        // Player High Score
        actors.push(act!(text: font("miso"): settext("----"):
            align(0.5, 0.5): // Centered, like default BitmapText in SM
            xy(pane_cx + cols_x[2] - (50.0 * text_zoom), pane_top + rows_y[1]):
            maxwidth(30.0): zoom(text_zoom): z(121): diffuse(0.0, 0.0, 0.0, 1.0)
        ));
        actors.push(act!(text: font("miso"): settext("??.??%"):
            align(1.0, 0.5): // Right-aligned
            xy(pane_cx + cols_x[2] + (25.0 * text_zoom), pane_top + rows_y[1]):
            zoom(text_zoom): z(121): diffuse(0.0, 0.0, 0.0, 1.0)
        ));

        // --- Difficulty Meter ---
        let meter_text = if let Some(MusicWheelEntry::Song(_)) = selected_entry {
            // It's a song, show meter or "?" if no chart exists for the difficulty
            selected_chart_data.as_ref().map_or("?".to_string(), |c| c.meter.to_string())
        } else {
            // It's a pack header, show nothing
            "".to_string()
        };
        
        let mut meter_actor = act!(text: font("wendy"): settext(meter_text):
            align(1.0, 0.5):
            xy(pane_cx + cols_x[3], pane_top + rows_y[1]):
            z(121): diffuse(0.0, 0.0, 0.0, 1.0)
        );
        if !is_wide() {
            if let Actor::Text { max_width, .. } = &mut meter_actor {
                *max_width = Some(66.0);
            }
        }
        actors.push(meter_actor);
    }

    // --- Pattern Info (P1) — SL 1:1 geometry ---
    let pat_cx = screen_center_x() - 182.0 - if is_wide() { 5.0 } else { 0.0 };
    let pat_cy = screen_center_y() + 23.0 + 88.0;
    let pat_w  = if is_wide() { 286.0 } else { 276.0 };
    let pat_h  = 64.0;

    // background
    actors.push(act!(quad:
        align(0.5, 0.5):
        xy(pat_cx, pat_cy):
        setsize(pat_w, pat_h):
        z(120):
        diffuse(UI_BOX_BG_COLOR[0], UI_BOX_BG_COLOR[1], UI_BOX_BG_COLOR[2], UI_BOX_BG_COLOR[3])
    ));

    let base_val_x   = pat_cx - pat_w * 0.5 + 40.0; // values (right-aligned)
    let base_label_x = pat_cx - pat_w * 0.5 + 50.0; // labels (left-aligned)
    let base_y       = pat_cy - pat_h * 0.5 + 13.0;

    let col_spacing = 150.0;
    let row_spacing = 20.0;
    let text_zoom   = 0.8;

    // helper: push one (value, label) cell — ONLY the value supports maxwidth
    let mut add_item = |col_idx: i32,
                        row_idx: i32,
                        value_text: &str,
                        label_text: &str,
                        max_value_w: Option<f32>| {
        let x_val   = base_val_x   + (col_idx as f32) * col_spacing;
        let x_label = base_label_x + (col_idx as f32) * col_spacing;
        let y       = base_y       + (row_idx as f32) * row_spacing;

        // value (right-aligned), optionally clamped
        match max_value_w {
            Some(maxw) => actors.push(act!(text: font("miso"): settext(value_text):
                align(1.0, 0.5):
                xy(x_val, y):
                maxwidth(maxw):
                zoom(text_zoom):
                z(121):
                diffuse(1.0, 1.0, 1.0, 1.0)
            )),
            None => actors.push(act!(text: font("miso"): settext(value_text):
                align(1.0, 0.5):
                xy(x_val, y):
                zoom(text_zoom):
                z(121):
                diffuse(1.0, 1.0, 1.0, 1.0)
            )),
        }

        // label (left-aligned) — no clamp for SL parity
        actors.push(act!(text: font("miso"): settext(label_text):
            align(0.0, 0.5):
            xy(x_label, y):
            zoom(text_zoom):
            z(121):
            diffuse(1.0, 1.0, 1.0, 1.0)
        ));
    };

    // Row 0: Crossovers | Footswitches
    add_item(0, 0, "69",  "Crossovers",   None);
    add_item(1, 0, "64",  "Footswitches", None);

    // Row 1: Sideswitches | Jacks
    add_item(0, 1, "123", "Sideswitches", None);
    add_item(1, 1, "124", "Jacks",        None);

    // Row 2: Brackets | Total Stream
    add_item(0, 2, "90",  "Brackets",     None);

    // Total Stream value text (only this one is clamped to 100 like SL)
    let total_stream_value = if let Some(chart) = &selected_chart_data {
        if chart.total_measures > 0 {
            let pct = (chart.total_streams as f32 / chart.total_measures as f32) * 100.0;
            format!("{}/{} ({:.1}%)", chart.total_streams, chart.total_measures, pct)
        } else {
            "None (0.0%)".to_string()
        }
    } else {
        "None (0.0%)".to_string()
    };

    // clamp ONLY the value to 100
    add_item(1, 2, &total_stream_value, "Total Stream", Some(100.0));

    // --- StepsDisplayList (Difficulty Meter Grid, SL parity) ---
    // Center at (_screen.cx - 26, _screen.cy + 67) with a 32x152 background,
    // five 28x28 squares spaced by 2px.

    let panel_cx = screen_center_x() - 26.0;
    let panel_cy = screen_center_y() + 67.0;

    // Background panel (#1e282f), 32x152
    actors.push(act!(quad:
        align(0.5, 0.5):
        xy(panel_cx, panel_cy):
        setsize(32.0, 152.0):
        z(120):
        diffuse(UI_BOX_BG_COLOR[0], UI_BOX_BG_COLOR[1], UI_BOX_BG_COLOR[2], UI_BOX_BG_COLOR[3])
    ));

    // Prepare meters per difficulty (Beginner..Challenge)
    let mut meters: [Option<i32>; 5] = [None, None, None, None, None];
    if let Some(MusicWheelEntry::Song(song)) = state.entries.get(state.selected_index) {
        for (i, name) in DIFFICULTY_NAMES.iter().enumerate() {
            if let Some(chart) = song.charts.iter().find(|c| c.difficulty.eq_ignore_ascii_case(name)) {
                meters[i] = Some(chart.meter as i32);
            }
        }
    }

    // Draw five rows: RowNumber = -2..2  (centered list)
    for (row_num, row_i) in (-2..=2).zip(0..5) {
        let y_off = (28.0 + 2.0) * (row_num as f32); // -60, -30, 0, +30, +60

        // Square background (#0f0f0f), 28x28
        actors.push(act!(quad:
            align(0.5, 0.5):
            xy(panel_cx, panel_cy + y_off):
            setsize(28.0, 28.0):
            z(121):
            diffuse(0.0588, 0.0588, 0.0588, 1.0) // #0f0f0f
        ));

        // Meter text, centered, zoom 0.45
        let (r, g, b, a) = if meters[row_i].is_some() {
            let meter_color_index = state.active_color_index - (4 - row_i) as i32;
            let c = color::simply_love_rgba(meter_color_index);
            (c[0], c[1], c[2], 1.0) // difficulty color
        } else {
            // dim when no chart: #182025
            (0.0941, 0.1255, 0.1451, 1.0)
        };
        let meter_text = meters[row_i].map(|m| m.to_string()).unwrap_or_else(|| "".to_string());

        actors.push(act!(text:
            font("wendy"):
            settext(meter_text):
            align(0.5, 0.5):                   // centered in the square
            xy(panel_cx, panel_cy + y_off):
            zoom(0.45):
            z(122):
            diffuse(r, g, b, a)
        ));
    }

    // --- MUSIC WHEEL (Now a component) ---
    actors.extend(music_wheel::build(music_wheel::MusicWheelParams {
        entries: &state.entries,
        selected_index: state.selected_index,
        active_color_index: state.active_color_index,
        selection_animation_timer: state.selection_animation_timer,
        pack_song_counts: &pack_song_counts,
    }));

    // --- Pulsating Meter Arrow (P1) ---
    let arrow_x_base = screen_center_x() - 53.0;
    let arrow_zoom = 0.575;

    // Determine which difficulty index to use for the arrow's position.
    // For packs, use the preferred difficulty. For songs, use the actual selected difficulty.
    let difficulty_index_for_arrow = if matches!(state.entries.get(state.selected_index), Some(MusicWheelEntry::PackHeader { .. })) {
        state.preferred_difficulty_index
    } else {
        state.selected_difficulty_index
    };

    // Y position must match the corresponding difficulty meter's Y.
    let row_num = difficulty_index_for_arrow as i32 - 2;
    let y_off = (28.0 + 2.0) * (row_num as f32);
    // The +1 matches the offset in the original Lua code.
    let arrow_y = panel_cy + y_off + 1.0;

    // The bounce animation is synced to the song's beat if a song is selected.
    let selected_song_for_bpm = if let Some(MusicWheelEntry::Song(song)) = state.entries.get(state.selected_index) {
        Some(song.clone())
    } else {
        None
    };

    // Use song's max BPM if available, otherwise default to a common value like 150.
    let bpm = selected_song_for_bpm.as_ref().map_or(150.0, |s| s.max_bpm.max(1.0)) as f32;
    let beat_duration_secs = 60.0 / bpm;

    // Calculate the phase for a 1-beat period cosine oscillation.
    // This simulates StepMania's `effectmagnitude(-3)` bounce effect.
    let phase = (state.session_elapsed / beat_duration_secs) * 2.0 * std::f32::consts::PI;
    let bounce_offset_x = -1.5 + 1.5 * phase.cos();

    // The arrow is always visible, pointing at the current song's difficulty
    // or the preferred difficulty if a pack is selected.
    actors.push(act!(sprite("meter_arrow.png"):
        align(0.0, 0.5): // Left-center align to match Lua's halign(0)
        xy(arrow_x_base + bounce_offset_x, arrow_y):
        zoom(arrow_zoom):
        z(122) // Above the difficulty display panel
    ));

    actors
}
