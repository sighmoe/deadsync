use std::time::Duration;

// Window
pub const WINDOW_TITLE: &str = "DeadSync";
pub const WINDOW_WIDTH: u32 = 1024;
pub const WINDOW_HEIGHT: u32 = 768;

// Asset Paths
// MSDF Fonts - NEW
pub const WENDY_MSDF_JSON_PATH: &str = "assets/fonts/wendy_msdf.json";
pub const WENDY_MSDF_TEXTURE_PATH: &str = "assets/fonts/wendy_msdf.png";
pub const MISO_MSDF_JSON_PATH: &str = "assets/fonts/miso_msdf.json";
pub const MISO_MSDF_TEXTURE_PATH: &str = "assets/fonts/miso_msdf.png";
pub const CJK_MSDF_JSON_PATH: &str = "assets/fonts/notosans_cjk_msdf.json";
pub const CJK_MSDF_TEXTURE_PATH: &str = "assets/fonts/notosans_cjk_msdf.png";
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
pub const W1_WINDOW_MS: f32 = 21.5;
pub const W2_WINDOW_MS: f32 = 43.0;
pub const W3_WINDOW_MS: f32 = 102.0;
pub const W4_WINDOW_MS: f32 = 135.0;
pub const MAX_HIT_WINDOW_MS: f32 = 180.0; // W4 outer edge
pub const MISS_WINDOW_MS: f32 = 200.0;    // Time after target beat until considered a miss

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
// Logo's display height will be this fraction of the window's current height.
pub const LOGO_HEIGHT_RATIO_TO_WINDOW_HEIGHT: f32 = 0.55;
pub const MENU_OPTIONS: [&str; 3] = ["GAMEPLAY", "OPTIONS", "EXIT"];
pub const MENU_SELECTED_COLOR: [f32; 4] = [1.0, 1.0, 0.5, 1.0];
pub const MENU_NORMAL_COLOR: [f32; 4] = [0.8, 0.8, 0.8, 1.0];

// --- UI Constants ---
// Color #A6A6A6 converted to normalized float RGBA
pub const UI_BAR_COLOR: [f32; 4] = [166.0 / 255.0, 166.0 / 255.0, 166.0 / 255.0, 1.0];
pub const UI_BAR_TEXT_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0]; // CHANGED TO BLACK
pub const UI_REFERENCE_HEIGHT: f32 = 768.0; // Reference screen height for scaling UI elements
pub const UI_BAR_REFERENCE_HEIGHT: f32 = 51.0; // Desired bar height at reference screen height

// MSDF Shader Parameters (NEW)
pub const MSDF_PX_RANGE: f32 = 4.0; // Should match the -pxrange used in msdf-atlas-gen, or be configurable per font

// Misc
pub const MAX_DELTA_TIME: f32 = 0.1; // Clamp dt to avoid large jumps

// Colors
/* Colors = {
    "#FF5D47",
    "#FF577E",
    "#FF47B3",
    "#DD57FF",
    "#8885ff",
    "#3D94FF",
    "#00B8CC",
    "#5CE087",
    "#AEFA44",
    "#FFFF00",
    "#FFBE00",
    "#FF7D00",
},

		ITG = {
			color("#21CCE8"),	-- blue
			color("#e29c18"),	-- gold
			color("#66c955"),	-- green
			color("#b45cff"),	-- purple (greatly lightened)
			color("#c9855e"),	-- peach?
			color("#ff3030")	-- red (slightly lightened)
		}, */