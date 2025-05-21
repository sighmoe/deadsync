use crate::parsing::simfile::{SongInfo, ProcessedChartData, NoteChar};
use crate::screens::gameplay::{TimingData};
use crate::graphics::texture::TextureResource; // Added for graph texture
use cgmath::Matrix4;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use winit::keyboard::{Key, NamedKey};

// --- Core Application State ---
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Menu,
    SelectMusic,
    Options,
    Gameplay,
    Exiting,
}

// --- Screen Specific States ---

// MenuState
#[derive(Debug, Clone)] // MenuState can remain Clone if needed
pub struct MenuState {
    pub options: Vec<String>,
    pub selected_index: usize,
}
const MAIN_MENU_OPTIONS: [&str; 3] = ["Gameplay", "Options", "Exit"];
impl Default for MenuState {
    fn default() -> Self {
        MenuState {
            options: MAIN_MENU_OPTIONS.iter().map(|&s| s.to_string()).collect(),
            selected_index: 0,
        }
    }
}


#[derive(Debug, Clone)] // MusicWheelEntry can be Clone
pub enum MusicWheelEntry {
    Song(Arc<SongInfo>),
    PackHeader { name: String, color: [f32; 4], banner_path: Option<PathBuf> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)] // NavDirection can be Clone
pub enum NavDirection {
    Up,
    Down,
}

#[derive(Debug)] // SelectMusicState is NOT Clone because of TextureResource
pub struct SelectMusicState {
    pub entries: Vec<MusicWheelEntry>, // Vec<T> is Clone if T is Clone
    pub selected_index: usize,
    pub expanded_pack_name: Option<String>, // Option<String> is Clone
    pub selection_animation_timer: f32,
    pub nav_key_held_direction: Option<NavDirection>, // Option<T> is Clone if T is Clone
    pub nav_key_held_since: Option<Instant>,       // Option<Instant> is Clone
    pub nav_key_last_scrolled_at: Option<Instant>,   // Option<Instant> is Clone

    // Music preview fields
    pub preview_audio_path: Option<PathBuf>,    // Option<PathBuf> is Clone
    pub preview_sample_start_sec: Option<f32>,
    pub preview_sample_length_sec: Option<f32>,
    pub preview_playback_started_at: Option<Instant>,
    pub is_awaiting_preview_restart: bool,
    pub preview_restart_delay_timer: f32,

    // Fields for delayed preview actions
    pub selection_landed_at: Option<Instant>,
    pub is_preview_actions_scheduled: bool,

    // NPS Graph texture
    pub current_graph_texture: Option<TextureResource>, // This is why SelectMusicState can't derive Clone
    pub current_graph_song_chart_key: Option<String>,
}

impl Default for SelectMusicState {
    fn default() -> Self {
        SelectMusicState {
            entries: Vec::new(),
            selected_index: 0,
            expanded_pack_name: None,
            selection_animation_timer: 0.0,
            nav_key_held_direction: None,
            nav_key_held_since: None,
            nav_key_last_scrolled_at: None,

            preview_audio_path: None,
            preview_sample_start_sec: None,
            preview_sample_length_sec: None,
            preview_playback_started_at: None,
            is_awaiting_preview_restart: false,
            preview_restart_delay_timer: 0.0,

            selection_landed_at: None,
            is_preview_actions_scheduled: false,
            current_graph_texture: None,
            current_graph_song_chart_key: None,
        }
    }
}

// OptionsState
#[derive(Debug, Clone, Default)] // Can be Clone if it only contains cloneable data
pub struct OptionsState {
    pub placeholder: bool,
}

// GameState
#[derive(Debug)] // GameState also contains Arcs and non-Clone resources effectively
pub struct GameState {
    pub targets: Vec<TargetInfo>,
    pub arrows: HashMap<ArrowDirection, Vec<Arrow>>,
    pub pressed_keys: HashSet<VirtualKeyCode>,
    pub current_beat: f32, // Display beat (accounts for sync offset)
    pub current_chart_beat_actual: f32, // Actual musical beat based on audio time
    pub window_size: (f32, f32),
    pub active_explosions: HashMap<ArrowDirection, ActiveExplosion>,
    pub audio_start_time: Option<Instant>,
    pub song_info: Arc<SongInfo>,
    pub selected_chart_idx: usize,
    pub timing_data: Arc<TimingData>,
    pub processed_chart: Arc<ProcessedChartData>,
    pub current_measure_idx: usize,
    pub current_line_in_measure_idx: usize,
    pub current_processed_beat: f32,
    pub judgment_counts: HashMap<Judgment, u32>, 
}

// --- Gameplay Elements (remain the same) ---
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ArrowDirection { Left, Down, Up, Right, }
pub const ALL_ARROW_DIRECTIONS: [ArrowDirection; 4] = [ ArrowDirection::Left, ArrowDirection::Down, ArrowDirection::Up, ArrowDirection::Right, ];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Judgment { W1, W2, W3, W4, W5, Miss }
pub const ALL_JUDGMENTS: [Judgment; 6] = [Judgment::W1, Judgment::W2, Judgment::W3, Judgment::W4, Judgment::W5, Judgment::Miss];


#[derive(Debug, Clone)] pub struct TargetInfo { pub x: f32, pub y: f32, pub direction: ArrowDirection, }
#[derive(Debug, Clone)]
pub struct Arrow {
    pub x: f32,
    pub y: f32,
    pub direction: ArrowDirection,
    pub note_char: NoteChar,
    pub target_beat: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct ActiveExplosion {
    pub judgment: Judgment,
    pub direction: ArrowDirection,
    pub end_time: Instant,
}


// --- Input (remains the same) ---
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)] #[repr(u32)] pub enum VirtualKeyCode { Left, Down, Up, Right, Enter, Escape, }
pub fn key_to_virtual_keycode(key: Key) -> Option<VirtualKeyCode> {
    match key {
        Key::Named(NamedKey::ArrowLeft) => Some(VirtualKeyCode::Left),
        Key::Named(NamedKey::ArrowDown) => Some(VirtualKeyCode::Down),
        Key::Named(NamedKey::ArrowUp) => Some(VirtualKeyCode::Up),
        Key::Named(NamedKey::ArrowRight) => Some(VirtualKeyCode::Right),
        Key::Named(NamedKey::Enter) => Some(VirtualKeyCode::Enter),
        Key::Named(NamedKey::Escape) => Some(VirtualKeyCode::Escape),
        _ => None,
    }
 }

// --- Graphics Related (remains the same) ---
#[repr(C)] #[derive(Debug, Clone, Copy)] pub struct PushConstantData {
    pub model: Matrix4<f32>,
    pub color: [f32; 4],
    pub uv_offset: [f32; 2],
    pub uv_scale: [f32; 2],
    pub px_range: f32,
 }