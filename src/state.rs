use crate::config;
use crate::parsing::simfile::SongInfo;
use cgmath::Matrix4;
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use winit::keyboard::{Key, NamedKey};

// --- Core Application State --- (Same)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Menu,
    SelectMusic,
    Options,
    Gameplay,
    Exiting,
}

// --- Screen Specific States ---

// MenuState (Same)
#[derive(Debug, Clone)]
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


#[derive(Debug, Clone)]
pub struct SelectMusicState {
    // Change Vec<String> to Vec<SongInfo>
    pub songs: Vec<SongInfo>,
    pub selected_index: usize,
}

impl Default for SelectMusicState {
    fn default() -> Self {
        SelectMusicState {
            // Initialize empty, App::transition_state will populate it
            songs: Vec::new(),
            selected_index: 0,
        }
    }
}

// OptionsState (Same)
#[derive(Debug, Clone, Default)]
pub struct OptionsState {
    pub placeholder: bool,
}

// GameState (Add fields for BPM etc. later if needed)
#[derive(Debug, Clone)]
pub struct GameState {
    pub targets: Vec<TargetInfo>,
    pub arrows: HashMap<ArrowDirection, Vec<Arrow>>,
    pub pressed_keys: HashSet<VirtualKeyCode>,
    pub last_spawned_16th_index: i32,
    pub last_spawned_direction: Option<ArrowDirection>,
    pub current_beat: f32,
    pub window_size: (f32, f32),
    pub flash_states: HashMap<ArrowDirection, FlashState>,
    pub audio_start_time: Option<Instant>,
    // pub song_bpm: f32, // Example: Add BPM here
    // pub song_offset_secs: f32, // Example: Add offset here
}

// --- Gameplay Elements --- (Same)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ArrowDirection { Left, Down, Up, Right, }
pub const ALL_ARROW_DIRECTIONS: [ArrowDirection; 4] = [ ArrowDirection::Left, ArrowDirection::Down, ArrowDirection::Up, ArrowDirection::Right, ];
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum NoteType { Quarter, Eighth, Sixteenth, }
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum Judgment { W1, W2, W3, W4, Miss, }
#[derive(Debug, Clone)] pub struct TargetInfo { pub x: f32, pub y: f32, pub direction: ArrowDirection, }
#[derive(Debug, Clone)] pub struct Arrow { pub x: f32, pub y: f32, pub direction: ArrowDirection, pub note_type: NoteType, pub target_beat: f32, }
#[derive(Debug, Clone, Copy)] pub struct FlashState { pub color: [f32; 4], pub end_time: Instant, }

// --- Input --- (Same)
#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)] #[repr(u32)] pub enum VirtualKeyCode { Left, Down, Up, Right, Enter, Escape, }
pub fn key_to_virtual_keycode(key: Key) -> Option<VirtualKeyCode> { /* ... same ... */
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

// --- Graphics Related --- (Same)
#[repr(C)] #[derive(Debug, Clone, Copy)] pub struct PushConstantData { /* ... same ... */
    pub model: Matrix4<f32>,
    pub color: [f32; 4],
    pub uv_offset: [f32; 2],
    pub uv_scale: [f32; 2],
    pub px_range: f32,
 }