use crate::act;
use crate::core::audio;
use crate::core::space::globals::*;
use crate::screens::{Screen, ScreenAction};
use crate::ui::actors::Actor;
use crate::ui::color;
use crate::ui::components::heart_bg;
use crate::ui::components::screen_bar::{self, ScreenBarParams, ScreenBarPosition, ScreenBarTitlePlacement};
use crate::ui::actors::SizeSpec;
use std::collections::HashSet;
use std::sync::{Arc, LazyLock};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use log::info;
use std::fs;

// --- engine imports ---
use crate::core::space::is_wide;
use crate::core::song_loading::{SongData, get_song_cache, ChartData, SongPack};


#[allow(dead_code)] fn col_music_wheel_box() -> [f32; 4] { color::rgba_hex("#0a141b") }
#[allow(dead_code)] fn col_pack_header_box() -> [f32; 4] { color::rgba_hex("#4c565d") }
#[allow(dead_code)] fn col_selected_song_box() -> [f32; 4] { color::rgba_hex("#272f35") }
#[allow(dead_code)] fn col_selected_pack_header_box() -> [f32; 4] { color::rgba_hex("#5f686e") }
#[allow(dead_code)] fn col_pink_box() -> [f32; 4] { color::rgba_hex("#ff47b3") }

/* ---------------------------- transitions ---------------------------- */
const TRANSITION_IN_DURATION: f32 = 0.5;
const TRANSITION_OUT_DURATION: f32 = 0.3;

// --- THEME LAYOUT CONSTANTS (unscaled, native dimensions) ---
const BANNER_NATIVE_WIDTH: f32 = 418.0;
const BANNER_NATIVE_HEIGHT: f32 = 164.0;
const BAR_H: f32 = 32.0;

// --- Other UI Constants ---
static UI_BOX_BG_COLOR: LazyLock<[f32; 4]> = LazyLock::new(|| color::rgba_hex("#1E282F"));
const MUSIC_WHEEL_TEXT_TARGET_PX: f32 = 15.0;
const NUM_WHEEL_ITEMS: usize = 17;
const CENTER_WHEEL_SLOT_INDEX: usize = NUM_WHEEL_ITEMS / 2;
const SELECTION_ANIMATION_CYCLE_DURATION: f32 = 1.0;
const SONG_TEXT_LEFT_PADDING: f32 = 66.0;
const PACK_COUNT_RIGHT_PADDING: f32 = 11.0;
const PACK_COUNT_TEXT_TARGET_PX: f32 = 14.0;
static DIFFICULTY_DISPLAY_INNER_BOX_COLOR: LazyLock<[f32; 4]> = LazyLock::new(|| color::rgba_hex("#0f0f0f"));
pub const DIFFICULTY_NAMES: [&str; 5] = ["Beginner", "Easy", "Medium", "Hard", "Challenge"];
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
    pub active_chord_keys: HashSet<KeyCode>,
    pub last_difficulty_nav_key: Option<KeyCode>,
    pub last_difficulty_nav_time: Option<Instant>,
    pub nav_key_held_direction: Option<NavDirection>,
    pub nav_key_held_since: Option<Instant>,
    pub nav_key_last_scrolled_at: Option<Instant>,
    pub currently_playing_preview_path: Option<PathBuf>,
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
        active_chord_keys: HashSet::new(),
        last_difficulty_nav_key: None,
        last_difficulty_nav_time: None,
        nav_key_held_direction: None,
        nav_key_held_since: None,
        nav_key_last_scrolled_at: None,
        currently_playing_preview_path: None,
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
            if state.active_chord_keys.contains(&KeyCode::ArrowUp) && state.active_chord_keys.contains(&KeyCode::ArrowDown) {
                if state.expanded_pack_name.is_some() {
                    info!("Up+Down combo: Collapsing pack.");
                    state.expanded_pack_name = None;
                    rebuild_displayed_entries(state);
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

    let music_path_for_preview = selected_song.as_ref().and_then(|s| s.music_path.clone());
    if state.currently_playing_preview_path != music_path_for_preview {
        state.currently_playing_preview_path = music_path_for_preview;

        let mut played = false;
        if let Some(song) = &selected_song {
            if let (Some(path), Some(start), Some(length)) = (&song.music_path, song.sample_start, song.sample_length) {
                if length > 0.0 {
                    info!("Playing preview for '{}' at {:.2}s for {:.2}s", song.title, start, length);
                    let cut = audio::Cut { start_sec: start as f64, length_sec: length as f64 };
                    audio::play_music(path.clone(), cut);
                    played = true;
                }
            }
        }
        if !played {
            audio::stop_music();
        }
    }

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
        left_text: Some("PerfectTaste"), center_text: None, right_text: Some("NOT PRESENT"),
    }));

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
    let (artist_text, bpm_text, length_text) = if let Some(MusicWheelEntry::Song(song)) = selected_entry {
        let minutes = song.total_length_seconds / 60;
        let seconds = song.total_length_seconds % 60;
        (
            song.artist.clone(),
            song.normalized_bpms.clone(),
            format!("{}:{:02}", minutes, seconds)
        )
    } else {
        // Fallback text for pack headers or empty list
        ("".to_string(), "".to_string(), "".to_string())
    };

    let label_color = [0.5, 0.5, 0.5, 1.0];
    let value_color = [1.0, 1.0, 1.0, 1.0];

    // Build the nested structure manually as per your example.
    let main_frame = Actor::Frame {
        align: [0.0, 0.0],
        offset: [frame_x, frame_y],
        size: [SizeSpec::Px(box_width), SizeSpec::Px(50.0)],
        background: None,
        z: 51,
        children: vec![
            // Background Quad
            {
                let bg_color = color::rgba_hex("#1e282f");
                act!(quad:
                    setsize(box_width, 50.0):
                    diffuse(bg_color[0], bg_color[1], bg_color[2], bg_color[3])
                )
            },
            // Inner Frame for Text
            Actor::Frame {
                align: [0.0, 0.0],
                offset: [-110.0, -6.0],
                size: [SizeSpec::Fill, SizeSpec::Fill],
                background: None,
                z: 0,
                children: vec![
                    act!(text: font("miso"): settext("ARTIST"):
                        align(1.0, 0.0):
                        y(-11.0):
                        zoomtowidth(44.0):
                        diffuse(label_color[0], label_color[1], label_color[2], label_color[3]):
                        z(52)
                    ),
                    act!(text: font("miso"): settext(artist_text):
                        align(0.0, 0.0):
                        xy(5.0, -11.0):
                        zoomtoheight(15.0):
                        diffuse(value_color[0], value_color[1], value_color[2], value_color[3]):
                        z(52)
                    ),
                    act!(text: font("miso"): settext("BPM"):
                        align(1.0, 0.0):
                        y(10.0):
                        zoomtoheight(15.0): // Using zoomtoheight as maxwidth is not available for labels
                        diffuse(label_color[0], label_color[1], label_color[2], label_color[3]):
                        z(52)
                    ),
                    act!(text: font("miso"): settext(bpm_text):
                        align(0.0, 0.5):
                        xy(5.0, 17.0):
                        zoomtoheight(15.0): // vertspacing not supported
                        diffuse(value_color[0], value_color[1], value_color[2], value_color[3]):
                        z(52)
                    ),
                    act!(text: font("miso"): settext("LENGTH"):
                        align(1.0, 0.0):
                        xy(box_width - 130.0, 10.0):
                        zoomtoheight(15.0):
                        diffuse(label_color[0], label_color[1], label_color[2], label_color[3]):
                        z(52)
                    ),
                    act!(text: font("miso"): settext(length_text):
                        align(0.0, 0.0):
                        xy(box_width - 125.0, 10.0):
                        zoomtoheight(15.0):
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
        let peak_nps_text = selected_chart_data.as_ref().map_or("Peak NPS: --".to_string(), |c| format!("Peak NPS: {:.1}", c.meter)); // Using meter as a placeholder for now
        let breakdown_text = selected_chart_data.as_ref().map_or("".to_string(), |c| format!("{}", c.meter)); // Placeholder

    // --- Step credit panel (P1, song mode) — placed just above the density graph, no overlap ---

    if is_wide() {
        // Background quad — center at (cx - 243, Y just above the graph)
        actors.push(act!(quad:
            align(0.5, 0.5):
            xy(
                screen_center_x() - 243.0,
                // graph top = cy + 23 - 32  ;  steps center = graph_top - steps_h/2 - 2
                screen_center_y() - 9.0 - (screen_height() / 28.0) * 0.5 - 2.0
            ):
            setsize(175.0, screen_height() / 28.0):
            z(120):
            diffuse(
                color::simply_love_rgba(state.selected_difficulty_index as i32)[0],
                color::simply_love_rgba(state.selected_difficulty_index as i32)[1],
                color::simply_love_rgba(state.selected_difficulty_index as i32)[2],
                1.0
            )
        ));

        // "STEPS" label — left aligned at (cx - 326, same Y as the quad center)
        actors.push(act!(text:
            font("miso"):
            settext("STEPS"):
            align(0.0, 0.5):
            xy(
                screen_center_x() - 326.0,
                screen_center_y() - 9.0 - (screen_height() / 28.0) * 0.5 - 2.0
            ):
            zoom(0.8):
            zoomtowidth(40.0):
            z(121):
            diffuse(0.0, 0.0, 0.0, 1.0)
        ));

        // Step artist text — left aligned at (cx - 281, same Y as the quad center)
        actors.push(act!(text:
            font("miso"):
            settext(step_artist_text):
            align(0.0, 0.5):
            xy(
                screen_center_x() - 281.0,
                screen_center_y() - 9.0 - (screen_height() / 28.0) * 0.5 - 2.0
            ):
            zoom(0.8):
            zoomtowidth(124.0):
            z(121):
            diffuse(0.0, 0.0, 0.0, 1.0)
        ));
    } else {
        // 4:3 layout — same math, SL X positions for 4:3
        actors.push(act!(quad:
            align(0.5, 0.5):
            xy(
                screen_center_x() - 233.0,
                screen_center_y() - 9.0 - (screen_height() / 28.0) * 0.5 - 2.0
            ):
            setsize(175.0, screen_height() / 28.0):
            z(120):
            diffuse(
                color::simply_love_rgba(state.selected_difficulty_index as i32)[0],
                color::simply_love_rgba(state.selected_difficulty_index as i32)[1],
                color::simply_love_rgba(state.selected_difficulty_index as i32)[2],
                1.0
            )
        ));

        actors.push(act!(text:
            font("miso"):
            settext("STEPS"):
            align(0.0, 0.5):
            xy(
                screen_center_x() - 316.0,
                screen_center_y() - 9.0 - (screen_height() / 28.0) * 0.5 - 2.0
            ):
            zoom(0.8):
            zoomtowidth(40.0):
            z(121):
            diffuse(0.0, 0.0, 0.0, 1.0)
        ));

        actors.push(act!(text:
            font("miso"):
            settext(step_artist_text):
            align(0.0, 0.5):
            xy(
                screen_center_x() - 271.0,
                screen_center_y() - 9.0 - (screen_height() / 28.0) * 0.5 - 2.0
            ):
            zoom(0.8):
            zoomtowidth(124.0):
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
        diffuse(0.1176, 0.1568, 0.1843, 1.0)   // #1e282f
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
            zoomtoheight(15):
            zoomtowidth(panel_w / 0.8)
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
        diffuse(
            color::simply_love_rgba(state.selected_difficulty_index as i32)[0],
            color::simply_love_rgba(state.selected_difficulty_index as i32)[1],
            color::simply_love_rgba(state.selected_difficulty_index as i32)[2],
            1.0
        )
    ));

    // grid metrics (Straight from SL)
    let text_zoom = if is_wide() { 0.9 } else { 0.8 };
    let col1_x = if is_wide() { -133.0 } else { -104.0 };
    let col2_x = if is_wide() {  -38.0 } else {  -36.0 };
    let row1_y = 13.0;
    let row2_y = 31.0;
    let row3_y = 49.0;

    // ---------- Row 1: Taps, Mines ----------
    actors.push(act!(text: font("miso"): settext("432"):
        align(1.0, 0.5):
        xy(pane_cx + col1_x, pane_top + row1_y):
        zoom(text_zoom):
        z(121):
        diffuse(0.0, 0.0, 0.0, 1.0)
    ));
    actors.push(act!(text: font("miso"): settext("Steps"):
        align(0.0, 0.5):
        xy(pane_cx + col1_x + 3.0, pane_top + row1_y):
        zoom(text_zoom):
        z(121):
        diffuse(0.0, 0.0, 0.0, 1.0)
    ));

    actors.push(act!(text: font("miso"): settext("12"):
        align(1.0, 0.5):
        xy(pane_cx + col2_x, pane_top + row1_y):
        zoom(text_zoom):
        z(121):
        diffuse(0.0, 0.0, 0.0, 1.0)
    ));
    actors.push(act!(text: font("miso"): settext("Mines"):
        align(0.0, 0.5):
        xy(pane_cx + col2_x + 3.0, pane_top + row1_y):
        zoom(text_zoom):
        z(121):
        diffuse(0.0, 0.0, 0.0, 1.0)
    ));

    // ---------- Row 2: Jumps, Hands ----------
    actors.push(act!(text: font("miso"): settext("38"):
        align(1.0, 0.5):
        xy(pane_cx + col1_x, pane_top + row2_y):
        zoom(text_zoom):
        z(121):
        diffuse(0.0, 0.0, 0.0, 1.0)
    ));
    actors.push(act!(text: font("miso"): settext("Jumps"):
        align(0.0, 0.5):
        xy(pane_cx + col1_x + 3.0, pane_top + row2_y):
        zoom(text_zoom):
        z(121):
        diffuse(0.0, 0.0, 0.0, 1.0)
    ));

    actors.push(act!(text: font("miso"): settext("5"):
        align(1.0, 0.5):
        xy(pane_cx + col2_x, pane_top + row2_y):
        zoom(text_zoom):
        z(121):
        diffuse(0.0, 0.0, 0.0, 1.0)
    ));
    actors.push(act!(text: font("miso"): settext("Hands"):
        align(0.0, 0.5):
        xy(pane_cx + col2_x + 3.0, pane_top + row2_y):
        zoom(text_zoom):
        z(121):
        diffuse(0.0, 0.0, 0.0, 1.0)
    ));

    // ---------- Row 3: Holds, Rolls ----------
    actors.push(act!(text: font("miso"): settext("22"):
        align(1.0, 0.5):
        xy(pane_cx + col1_x, pane_top + row3_y):
        zoom(text_zoom):
        z(121):
        diffuse(0.0, 0.0, 0.0, 1.0)
    ));
    actors.push(act!(text: font("miso"): settext("Holds"):
        align(0.0, 0.5):
        xy(pane_cx + col1_x + 3.0, pane_top + row3_y):
        zoom(text_zoom):
        z(121):
        diffuse(0.0, 0.0, 0.0, 1.0)
    ));

    actors.push(act!(text: font("miso"): settext("0"):
        align(1.0, 0.5):
        xy(pane_cx + col2_x, pane_top + row3_y):
        zoom(text_zoom):
        z(121):
        diffuse(0.0, 0.0, 0.0, 1.0)
    ));
    actors.push(act!(text: font("miso"): settext("Rolls"):
        align(0.0, 0.5):
        xy(pane_cx + col2_x + 3.0, pane_top + row3_y):
        zoom(text_zoom):
        z(121):
        diffuse(0.0, 0.0, 0.0, 1.0)
    ));

    // --- Pattern Info (P1) — absolute placement, SL layout, static values ---

    // density graph center (from SL): (cx-182, cy+23) with -5px extra on wide
    let pat_cx = screen_center_x() - 182.0 - if is_wide() { 5.0 } else { 0.0 };
    let pat_cy = screen_center_y() + 23.0 + 88.0; // PatternInfo sits 88px below graph center

    let pat_w = if is_wide() { 286.0 } else { 276.0 };
    let pat_h = 64.0;

    // background
    actors.push(act!(quad:
        align(0.5, 0.5):
        xy(pat_cx, pat_cy):
        setsize(pat_w, pat_h):
        z(120):
        diffuse(0.1176, 0.1568, 0.1843, 1.0) // #1e282f
    ));

    // grid baseline (relative to panel center, converted to absolute)
    let base_val_x   = pat_cx - pat_w * 0.5 + 40.0; // values (right-aligned)
    let base_label_x = pat_cx - pat_w * 0.5 + 50.0; // labels (left-aligned, +10)
    let base_y       = pat_cy - pat_h * 0.5 + 13.0;

    let col_spacing = 150.0;
    let row_spacing = 20.0;
    let text_zoom   = 0.8;

    // helper macro-ish closure for one cell (value + label)
    let mut add_item = |col_idx: i32, row_idx: i32, value_text: &str, label_text: &str, max_label_w: Option<f32>| {
        let x_val   = base_val_x   + (col_idx as f32) * col_spacing;
        let x_label = base_label_x + (col_idx as f32) * col_spacing;
        let y       = base_y       + (row_idx as f32) * row_spacing;

        // value
        actors.push(act!(text: font("miso"): settext(value_text):
            align(1.0, 0.5):
            xy(x_val, y):
            zoom(text_zoom):
            z(121):
            diffuse(1.0, 1.0, 1.0, 1.0)
        ));
        // label
        let mut label = act!(text: font("miso"): settext(label_text):
            align(0.0, 0.5):
            xy(x_label, y):
            zoom(text_zoom):
            z(121):
            diffuse(1.0, 1.0, 1.0, 1.0)
        );
        if let Some(maxw) = max_label_w {
            // clamp if you want; SL sometimes maxwidths here
            label = act!(text: font("miso"): settext(label_text):
                align(0.0, 0.5):
                xy(x_label, y):
                zoom(text_zoom):
                zoomtowidth(maxw):
                z(121):
                diffuse(1.0, 1.0, 1.0, 1.0)
            );
        }
        actors.push(label);
    };

    // Row 0: Crossovers | Footswitches
    add_item(0, 0, "69", "Crossovers", None);
    add_item(1, 0, "64", "Footswitches", None);

    // Row 1: Sideswitches | Jacks
    add_item(0, 1, "123", "Sideswitches", None);
    add_item(1, 1, "124", "Jacks", None);

    // Row 2: Brackets | Total Stream
    add_item(0, 2, "90", "Brackets", None);
    // SL shows "None (0.0%)" for Total Stream when empty
    add_item(1, 2, "None (0.0%)", "Total Stream", None);

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
        diffuse(0.1176, 0.1568, 0.1843, 1.0) // #1e282f
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
            let c = color::simply_love_rgba(row_i as i32);
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

    // --- MUSIC WHEEL (tight gap + full-bleed right edge + SL text anchors) ---

    // SL text anchors
    let song_title_x = screen_center_x() + if is_wide() { 104.0 } else { 65.0 };  // songs
    let pack_title_x = screen_center_x() + if is_wide() { 204.0 } else { 150.0 }; // packs

    // Derive left edge from song title X and your 66px padding; right edge = screen right
    let wheel_left_x = song_title_x - SONG_TEXT_LEFT_PADDING;  // 66px left padding for song titles
    let right_edge   = screen_width();                          // full bleed
    let wheel_w      = right_edge - wheel_left_x;

    // Spacing & row height (smaller gap)
    let slot_spacing = screen_height() / 15.0;
    let item_h       = (slot_spacing - 1.0).max(18.0); // set to slot_spacing for zero gap
    let center_y     = screen_center_y();

    // Selection pulse (unchanged)
    let anim_t_unscaled = (state.selection_animation_timer / SELECTION_ANIMATION_CYCLE_DURATION)
        * std::f32::consts::PI * 2.0;
    let anim_t = (anim_t_unscaled.sin() + 1.0) / 2.0;

    let num_entries = state.entries.len();
    if num_entries > 0 {
        for i_slot in 0..NUM_WHEEL_ITEMS {
            let offset_from_center = i_slot as isize - CENTER_WHEEL_SLOT_INDEX as isize;
            let y_center = center_y + (offset_from_center as f32) * slot_spacing;
            let y_top    = y_center - item_h * 0.5;
            let is_selected_slot = i_slot == CENTER_WHEEL_SLOT_INDEX;

            let list_index = (state.selected_index as isize + offset_from_center + num_entries as isize)
                as usize % num_entries;

            let (display_text, is_pack, bg_col, txt_col) = if let Some(entry) = state.entries.get(list_index) {
                match entry {
                    MusicWheelEntry::Song(info) => {
                        let base = col_music_wheel_box();
                        let sel  = col_selected_song_box();
                        let bg   = if is_selected_slot { lerp_color(base, sel, anim_t) } else { base };
                        (info.title.clone(), false, bg, [1.0, 1.0, 1.0])
                    }
                    MusicWheelEntry::PackHeader { name, original_index, .. } => {
                        let base = col_pack_header_box();
                        let sel  = col_selected_pack_header_box();
                        let bg   = if is_selected_slot { lerp_color(base, sel, anim_t) } else { base };
                        let c    = color::simply_love_rgba(state.active_color_index + *original_index as i32);
                        (name.clone(), true, bg, [c[0], c[1], c[2]])
                    }
                }
            } else {
                ("".to_string(), false, col_music_wheel_box(), [1.0, 1.0, 1.0])
            };

            // Full-bleed row background to the right edge
            actors.push(act!(quad:
                align(0.0, 0.0):
                xy(wheel_left_x, y_top):
                zoomto(wheel_w, item_h):
                diffuse(bg_col[0], bg_col[1], bg_col[2], 1.0):
                z(51)
            ));

            let pack_center_x = wheel_left_x + wheel_w * 0.5;

            // Text alignment: songs at song_title_x (left), packs at pack_title_x (left)
            if is_pack {
                actors.push(act!(text:
                    align(0.5, 0.5):
                    xy(pack_center_x, y_center):
                    zoomtoheight(MUSIC_WHEEL_TEXT_TARGET_PX):
                    font("miso"):
                    settext(display_text):
                    horizalign(center):
                    diffuse(txt_col[0], txt_col[1], txt_col[2], 1.0):
                    z(52)
                ));
            } else {
                actors.push(act!(text:
                    align(0.0, 0.5):
                    xy(song_title_x, y_center):
                    zoomtoheight(MUSIC_WHEEL_TEXT_TARGET_PX):
                    font("miso"):
                    settext(display_text):
                    horizalign(left):
                    diffuse(txt_col[0], txt_col[1], txt_col[2], 1.0):
                    z(52)
                ));
            }

            // Pack song counts: right-aligned against the true right edge
            if let Some(MusicWheelEntry::PackHeader { name, .. }) = state.entries.get(list_index) {
                let count = get_song_cache().iter().find(|p| &p.name == name)
                    .map(|p| p.songs.len()).unwrap_or(0);
                if count > 0 {
                    actors.push(act!(text:
                        align(1.0, 0.5):
                        xy(right_edge - PACK_COUNT_RIGHT_PADDING, y_center):
                        zoomtoheight(PACK_COUNT_TEXT_TARGET_PX):
                        font("miso"):
                        settext(format!("{}", count)):
                        horizalign(right):
                        diffuse(1.0, 1.0, 1.0, 0.8):
                        z(52)
                    ));
                }
            }
        }
    }

    actors
}