// src/config.rs
use std::time::Duration;

// Window
pub const WINDOW_TITLE: &str = "DeadSync";
pub const WINDOW_WIDTH: u32 = 1280;
pub const WINDOW_HEIGHT: u32 = 720;

// Asset Paths
// MSDF Fonts
pub const WENDY_MSDF_JSON_PATH: &str = "assets/fonts/wendy_msdf.json";
pub const WENDY_MSDF_TEXTURE_PATH: &str = "assets/fonts/wendy_msdf.png";
pub const MISO_MSDF_JSON_PATH: &str = "assets/fonts/miso_msdf.json";
pub const MISO_MSDF_TEXTURE_PATH: &str = "assets/fonts/miso_msdf.png";
pub const CJK_MSDF_JSON_PATH: &str = "assets/fonts/notosans_cjk_msdf.json";
pub const CJK_MSDF_TEXTURE_PATH: &str = "assets/fonts/notosans_cjk_msdf.png";
// Standard Textures
pub const LOGO_TEXTURE_PATH: &str = "assets/graphics/logo.png";
pub const DANCE_TEXTURE_PATH: &str = "assets/graphics/dance.png";
pub const ARROW_TEXTURE_PATH: &str = "assets/noteskins/cel/down_arrow_cel.png";
// Sounds
pub const SFX_CHANGE_PATH: &str = "assets/sounds/change.ogg";
pub const SFX_START_PATH: &str = "assets/sounds/start.ogg";
// Song related (examples, parsing handles actuals)
pub const SONG_FOLDER_PATH: &str = "songs/Pack/About Tonight";
pub const SONG_AUDIO_FILENAME: &str = "about_tonight.ogg";

// Gameplay Explosion Textures (Down direction, W1=Marvelous to W5=Okay/Boo)
pub const EXPLOSION_W1_TEXTURE_PATH: &str = "assets/noteskins/cel/down_tap_explosion_dim_w1.png";
pub const EXPLOSION_W2_TEXTURE_PATH: &str = "assets/noteskins/cel/down_tap_explosion_dim_w2.png";
pub const EXPLOSION_W3_TEXTURE_PATH: &str = "assets/noteskins/cel/down_tap_explosion_dim_w3.png";
pub const EXPLOSION_W4_TEXTURE_PATH: &str = "assets/noteskins/cel/down_tap_explosion_dim_w4.png";
pub const EXPLOSION_W5_TEXTURE_PATH: &str = "assets/noteskins/cel/down_tap_explosion_dim_w5.png";


// Gameplay Constants
pub const TARGET_Y_POS: f32 = 150.0;
pub const TARGET_SIZE: f32 = 120.0;
pub const ARROW_SIZE: f32 = 120.0;
pub const ARROW_SPEED: f32 = 1300.0; // Pixels per second
pub const AUDIO_SYNC_OFFSET_MS: i64 = 30;
pub const SPAWN_LOOKAHEAD_BEATS: f32 = 10.0;

// Judgment Windows (milliseconds)
// W1 (Marvelous) : 0 to W1_WINDOW_MS
// W2 (Perfect)   : >W1_WINDOW_MS to W2_WINDOW_MS
// W3 (Great)     : >W2_WINDOW_MS to W3_WINDOW_MS
// W4 (Good)      : >W3_WINDOW_MS to W4_WINDOW_MS
// W5 (Okay/Boo)  : >W4_WINDOW_MS to MAX_HIT_WINDOW_MS (formerly W4 outer range)
pub const W1_WINDOW_MS: f32 = 21.5;  // Marvelous
pub const W2_WINDOW_MS: f32 = 43.0;  // Perfect
pub const W3_WINDOW_MS: f32 = 102.0; // Great
pub const W4_WINDOW_MS: f32 = 135.0; // Good
pub const MAX_HIT_WINDOW_MS: f32 = 180.0; // Okay/Boo (this is W5's outer limit)
pub const MISS_WINDOW_MS: f32 = 200.0; // Time after target beat until considered a miss

// Visual Constants
pub const TARGET_TINT: [f32; 4] = [0.7, 0.7, 0.7, 0.5]; // Default target tint
pub const ARROW_TINT_QUARTER: [f32; 4] = [1.0, 0.6, 0.6, 1.0]; // Red-ish
pub const ARROW_TINT_EIGHTH: [f32; 4] = [0.6, 0.6, 1.0, 1.0];  // Blue-ish
pub const ARROW_TINT_SIXTEENTH: [f32; 4] = [0.6, 1.0, 0.6, 1.0]; // Green-ish
pub const ARROW_TINT_TWELFTH: [f32; 4] = [0.8, 0.5, 1.0, 1.0]; // Purple-ish for 12ths (triplets)
pub const ARROW_TINT_TWENTYFOURTH: [f32; 4] = [0.7, 0.4, 0.9, 1.0]; // Lighter Purple-ish for 24ths
pub const ARROW_TINT_OTHER: [f32; 4] = [0.9, 0.9, 0.9, 1.0];    // White/Gray for other quantizations

// pub const FLASH_COLOR_W1: [f32; 4] = [0.2, 0.7, 1.0, 0.9]; // Marvelous // No longer used for receptor
// pub const FLASH_COLOR_W2: [f32; 4] = [1.0, 0.8, 0.2, 0.9]; // Perfect   // No longer used for receptor
// pub const FLASH_COLOR_W3: [f32; 4] = [0.2, 1.0, 0.2, 0.9]; // Great     // No longer used for receptor
// pub const FLASH_COLOR_W4: [f32; 4] = [0.8, 0.4, 1.0, 0.9]; // Good      // No longer used for receptor
// pub const FLASH_DURATION: Duration = Duration::from_millis(120); // REPLACED by EXPLOSION_DURATION

pub const EXPLOSION_DURATION: Duration = Duration::from_millis(80); // How long explosion images stay on screen
pub const EXPLOSION_SIZE: f32 = TARGET_SIZE * 1.5;

// Menu Constants
pub const LOGO_HEIGHT_RATIO_TO_WINDOW_HEIGHT: f32 = 0.55;
pub const MENU_OPTIONS: [&str; 3] = ["GAMEPLAY", "OPTIONS", "EXIT"];
pub const MENU_SELECTED_COLOR: [f32; 4] = [1.0, 1.0, 0.5, 1.0];
pub const MENU_NORMAL_COLOR: [f32; 4] = [0.8, 0.8, 0.8, 1.0];

// --- UI Constants ---
pub const UI_BAR_COLOR: [f32; 4] = [166.0 / 255.0, 166.0 / 255.0, 166.0 / 255.0, 1.0];
pub const UI_BAR_TEXT_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
pub const UI_REFERENCE_HEIGHT: f32 = 768.0;
pub const UI_BAR_REFERENCE_HEIGHT: f32 = 51.0;

// MSDF Shader Parameters
pub const MSDF_PX_RANGE: f32 = 4.0;

// Misc
pub const MAX_DELTA_TIME: f32 = 0.1;

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
