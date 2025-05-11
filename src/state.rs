use crate::config;
use cgmath::Matrix4;
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use winit::keyboard::{Key, NamedKey};

// --- Core Application State ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Menu,
    SelectMusic, // NEW - Was missing in your provided file
    Options,     // NEW - Was missing in your provided file
    Gameplay,
    Exiting,
}

// --- Screen Specific States ---

#[derive(Debug, Clone)]
pub struct MenuState {
    pub options: Vec<String>,
    pub selected_index: usize,
}

// Define the menu options here or load from config if preferred
const MAIN_MENU_OPTIONS: [&str; 3] = ["Gameplay", "Options", "Exit"]; // UPDATED

impl Default for MenuState {
    fn default() -> Self {
        MenuState {
            // CORRECTED: Use the locally defined MAIN_MENU_OPTIONS constant
            options: MAIN_MENU_OPTIONS.iter().map(|&s| s.to_string()).collect(),
            selected_index: 0,
        }
    }
}

// NEW: State for the music selection screen - Was missing in your provided file
#[derive(Debug, Clone)]
pub struct SelectMusicState {
    pub songs: Vec<String>, // For now, just the names
    pub selected_index: usize,
    // Later: Add paths, BPM, etc. here or in a SongInfo struct
}

impl Default for SelectMusicState {
    fn default() -> Self {
        SelectMusicState {
            // TODO: Later, scan a directory for songs
            songs: vec![config::SONG_FOLDER_PATH
                .split('/')
                .last()
                .unwrap_or("Unknown Song")
                .to_string()], // Derive name from path for now
            selected_index: 0,
        }
    }
}

// NEW: State for the options screen (can be simple for now) - Was missing in your provided file
#[derive(Debug, Clone, Default)]
pub struct OptionsState {
    // Add fields for options later (e.g., volume, keybinds)
    pub placeholder: bool,
}

#[derive(Debug, Clone)]
pub struct GameState {
    pub targets: Vec<TargetInfo>,
    pub arrows: HashMap<ArrowDirection, Vec<Arrow>>, // Keyed by direction for easy access
    pub pressed_keys: HashSet<VirtualKeyCode>,
    pub last_spawned_16th_index: i32,
    pub last_spawned_direction: Option<ArrowDirection>, // For difficulty > 3 anti-repeat logic
    pub current_beat: f32,
    pub window_size: (f32, f32), // Store current window size
    pub flash_states: HashMap<ArrowDirection, FlashState>, // Target flash on hit
    pub audio_start_time: Option<Instant>, // When the gameplay music started
}

// --- Gameplay Elements ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)] // Ensure consistent representation if needed
pub enum ArrowDirection {
    Left,
    Down,
    Up,
    Right,
}

pub const ALL_ARROW_DIRECTIONS: [ArrowDirection; 4] = [
    ArrowDirection::Left,
    ArrowDirection::Down,
    ArrowDirection::Up,
    ArrowDirection::Right,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteType {
    Quarter, // Red
    Eighth,  // Blue
    Sixteenth, // Green
             // Consider ThirtySecond, SixtyFourth etc. if needed
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Judgment {
    W1, // Marvelous / Fantastic
    W2, // Perfect / Excellent
    W3, // Great
    W4, // Good / Decent
    Miss,
    // None // Optional: Represents no judgment yet
}

#[derive(Debug, Clone)]
pub struct TargetInfo {
    pub x: f32,
    pub y: f32,
    pub direction: ArrowDirection,
}

#[derive(Debug, Clone)]
pub struct Arrow {
    pub x: f32,
    pub y: f32,
    pub direction: ArrowDirection,
    pub note_type: NoteType,
    pub target_beat: f32, // The beat this arrow should be hit on
}

#[derive(Debug, Clone, Copy)]
pub struct FlashState {
    pub color: [f32; 4],
    pub end_time: Instant, // When the flash should disappear
}

// --- Input ---

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
#[repr(u32)]
pub enum VirtualKeyCode {
    Left,
    Down,
    Up,
    Right,
    Enter, // Added for menu
    Escape,
    // Add other keys if needed
}

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

// --- Graphics Related (used by Renderer) ---

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PushConstantData {
    pub model: Matrix4<f32>,
    pub color: [f32; 4],
    pub uv_offset: [f32; 2],
    pub uv_scale: [f32; 2],
    pub px_range: f32, // NEW: For MSDF shader
                       // pub screen_px_range: f32, // Optional if you implement advanced aa
}
