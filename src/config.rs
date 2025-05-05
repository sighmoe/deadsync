use std::time::Duration;

// Window
pub const WINDOW_TITLE: &str = "DeadSync";
pub const WINDOW_WIDTH: u32 = 1024;
pub const WINDOW_HEIGHT: u32 = 768;

// Asset Paths
//pub const FONT_INI_PATH: &str = "assets/fonts/miso.ini";
pub const FONT_INI_PATH: &str = "assets/fonts/wendy.ini";
//pub const FONT_TEXTURE_PATH: &str = "assets/fonts/_miso light 15x15 (res 360x360).png";
pub const FONT_TEXTURE_PATH: &str = "assets/fonts/_wendy small 13x8 (doubleres).png";
pub const LOGO_TEXTURE_PATH: &str = "assets/graphics/logo.png";
pub const DANCE_TEXTURE_PATH: &str = "assets/graphics/dance.png";
pub const ARROW_TEXTURE_PATH: &str = "assets/noteskins/down_arrow_cel.png";
pub const SFX_CHANGE_PATH: &str = "assets/sounds/change.ogg";
pub const SFX_START_PATH: &str = "assets/sounds/start.ogg";
pub const SONG_FOLDER_PATH: &str = "songs/Pack/About Tonight";
pub const SONG_AUDIO_FILENAME: &str = "about_tonight.ogg";

// Gameplay Constants
pub const TARGET_Y_POS: f32 = 150.0;
pub const TARGET_SIZE: f32 = 120.0;
pub const ARROW_SIZE: f32 = 120.0;
pub const ARROW_SPEED: f32 = 600.0; // Pixels per second
pub const SONG_BPM: f32 = 174.0;
pub const AUDIO_SYNC_OFFSET_MS: i64 = 60;
pub const SPAWN_LOOKAHEAD_BEATS: f32 = 4.0; // How many beats ahead to spawn notes (Reduced from 10.0)
pub const DIFFICULTY: u32 = 2; // 0:Q, 1:Q+50%E, 2:Q+E, 3:Q+E+S+NoRepeat, 4+:Q+E+S

// Judgment Windows (milliseconds)
pub const W1_WINDOW_MS: f32 = 22.5;
pub const W2_WINDOW_MS: f32 = 45.0;
pub const W3_WINDOW_MS: f32 = 90.0;
pub const W4_WINDOW_MS: f32 = 135.0;
pub const MAX_HIT_WINDOW_MS: f32 = 180.0; // W4 outer edge
pub const MISS_WINDOW_MS: f32 = 200.0;     // Time after target beat until considered a miss

// Visual Constants
pub const TARGET_TINT: [f32; 4] = [0.7, 0.7, 0.7, 0.5]; // Default target tint
pub const ARROW_TINT_QUARTER: [f32; 4] = [1.0, 0.6, 0.6, 1.0];
pub const ARROW_TINT_EIGHTH: [f32; 4] = [0.6, 0.6, 1.0, 1.0];
pub const ARROW_TINT_SIXTEENTH: [f32; 4] = [0.6, 1.0, 0.6, 1.0];
pub const FLASH_COLOR_W1: [f32; 4] = [0.2, 0.7, 1.0, 0.9]; // Marvelous
pub const FLASH_COLOR_W2: [f32; 4] = [1.0, 0.8, 0.2, 0.9]; // Perfect
pub const FLASH_COLOR_W3: [f32; 4] = [0.2, 1.0, 0.2, 0.9]; // Great
pub const FLASH_COLOR_W4: [f32; 4] = [0.8, 0.4, 1.0, 0.9]; // Good
pub const FLASH_DURATION: Duration = Duration::from_millis(120);

// Menu Constants
pub const LOGO_DISPLAY_WIDTH: f32 = 500.0;
pub const LOGO_Y_POS: f32 = WINDOW_HEIGHT as f32 - 700.0; // Adjust as needed
pub const MENU_OPTIONS: [&str; 2] = ["Play!", "Exit"];
pub const MENU_ITEM_SPACING: f32 = 4.5; // Multiplier for font line height
pub const MENU_START_Y_OFFSET: f32 = 140.0; // Offset from window center Y
pub const MENU_SELECTED_COLOR: [f32; 4] = [1.0, 1.0, 0.5, 1.0];
pub const MENU_NORMAL_COLOR: [f32; 4] = [0.8, 0.8, 0.8, 1.0];

// Misc
pub const MAX_DELTA_TIME: f32 = 0.1; // Clamp dt to avoid large jumps