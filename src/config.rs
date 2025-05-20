use std::time::Duration;

// Window
pub const WINDOW_TITLE: &str = "DeadSync";
pub const WINDOW_WIDTH: u32 = 1280;
pub const WINDOW_HEIGHT: u32 = 720;

// Asset Paths
pub const WENDY_MSDF_JSON_PATH: &str = "assets/fonts/wendy_msdf.json";
pub const WENDY_MSDF_TEXTURE_PATH: &str = "assets/fonts/wendy_msdf.png";
pub const MISO_MSDF_JSON_PATH: &str = "assets/fonts/miso_msdf.json";
pub const MISO_MSDF_TEXTURE_PATH: &str = "assets/fonts/miso_msdf.png";
pub const CJK_MSDF_JSON_PATH: &str = "assets/fonts/notosans_cjk_msdf.json";
pub const CJK_MSDF_TEXTURE_PATH: &str = "assets/fonts/notosans_cjk_msdf.png";
pub const LOGO_TEXTURE_PATH: &str = "assets/graphics/logo.png";
pub const DANCE_TEXTURE_PATH: &str = "assets/graphics/dance.png";
pub const ARROW_TEXTURE_PATH: &str = "assets/noteskins/cel/down_arrow_cel.png";
pub const SFX_CHANGE_PATH: &str = "assets/sounds/change.ogg";
pub const SFX_START_PATH: &str = "assets/sounds/start.ogg";
pub const EXPLOSION_W1_TEXTURE_PATH: &str = "assets/noteskins/cel/down_tap_explosion_dim_w1.png";
pub const EXPLOSION_W2_TEXTURE_PATH: &str = "assets/noteskins/cel/down_tap_explosion_dim_w2.png";
pub const EXPLOSION_W3_TEXTURE_PATH: &str = "assets/noteskins/cel/down_tap_explosion_dim_w3.png";
pub const EXPLOSION_W4_TEXTURE_PATH: &str = "assets/noteskins/cel/down_tap_explosion_dim_w4.png";
pub const EXPLOSION_W5_TEXTURE_PATH: &str = "assets/noteskins/cel/down_tap_explosion_dim_w5.png";

// Gameplay Constants
// pub const TARGET_Y_POS: f32 = 150.0; // Replaced by dynamic calculation
// pub const TARGET_SIZE: f32 = 120.0;  // Replaced by dynamic calculation
// pub const ARROW_SIZE: f32 = 120.0;   // Replaced by dynamic calculation
pub const ARROW_SPEED: f32 = 1300.0; // Speed of arrows scrolling up
pub const AUDIO_SYNC_OFFSET_MS: i64 = 30;
pub const SPAWN_LOOKAHEAD_BEATS: f32 = 10.0;
pub const W1_WINDOW_MS: f32 = 21.5;
pub const W2_WINDOW_MS: f32 = 43.0;
pub const W3_WINDOW_MS: f32 = 102.0;
pub const W4_WINDOW_MS: f32 = 135.0;
pub const MAX_HIT_WINDOW_MS: f32 = 180.0;
pub const MISS_WINDOW_MS: f32 = 200.0;

// Gameplay Layout Reference Constants (for 1280x720)
pub const GAMEPLAY_REF_WIDTH: f32 = 1280.0;
pub const GAMEPLAY_REF_HEIGHT: f32 = 720.0;
pub const TARGET_VISUAL_SIZE_REF: f32 = 96.0; // Visual size (height and width assuming square) of target receptor at reference resolution
pub const TARGET_TOP_MARGIN_REF: f32 = 125.0; // Distance from window top to target's top edge at reference resolution
pub const TARGET_SPACING_REF: f32 = 0.0;      // Horizontal gap between targets at reference resolution
pub const FIRST_TARGET_LEFT_MARGIN_REF: f32 = 128.0; // Distance from window left to first target's left edge at reference resolution
pub const EXPLOSION_SIZE_MULTIPLIER: f32 = 1.5; // Explosion size relative to target size

// Health Meter Reference Constants (for 1280x720)
pub const HEALTH_METER_LEFT_MARGIN_REF: f32 = 103.0;
pub const HEALTH_METER_TOP_MARGIN_REF: f32 = 14.0;
pub const HEALTH_METER_WIDTH_REF: f32 = 210.0;
pub const HEALTH_METER_HEIGHT_REF: f32 = 33.0;
pub const HEALTH_METER_BORDER_THICKNESS_REF: f32 = 3.0;
pub const HEALTH_METER_BORDER_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0]; // White
pub const HEALTH_METER_FILL_COLOR: [f32; 4] = [193.0/255.0, 0.0/255.0, 111.0/255.0, 1.0]; // c1006f
pub const HEALTH_METER_EMPTY_COLOR: [f32; 4] = UI_BOX_DARK_COLOR; // Use existing dark box color

// Visual Constants
pub const TARGET_TINT: [f32; 4] = [0.7, 0.7, 0.7, 0.5];
pub const ARROW_TINT_QUARTER: [f32; 4] = [1.0, 0.6, 0.6, 1.0];
pub const ARROW_TINT_EIGHTH: [f32; 4] = [0.6, 0.6, 1.0, 1.0];
pub const ARROW_TINT_SIXTEENTH: [f32; 4] = [0.6, 1.0, 0.6, 1.0];
pub const ARROW_TINT_TWELFTH: [f32; 4] = [0.8, 0.5, 1.0, 1.0];
pub const ARROW_TINT_TWENTYFOURTH: [f32; 4] = [0.7, 0.4, 0.9, 1.0];
pub const ARROW_TINT_OTHER: [f32; 4] = [0.9, 0.9, 0.9, 1.0];
pub const EXPLOSION_DURATION: Duration = Duration::from_millis(80);

// Menu Constants
pub const LOGO_HEIGHT_RATIO_TO_WINDOW_HEIGHT: f32 = 0.55;
pub const MENU_OPTIONS: [&str; 3] = ["GAMEPLAY", "OPTIONS", "EXIT"];
pub const MENU_SELECTED_COLOR: [f32; 4] = [1.0, 1.0, 0.5, 1.0];
pub const MENU_NORMAL_COLOR: [f32; 4] = [0.8, 0.8, 0.8, 1.0];

// --- UI Constants ---
pub const UI_BAR_COLOR: [f32; 4] = [166.0 / 255.0, 166.0 / 255.0, 166.0 / 255.0, 1.0];
pub const UI_BAR_TEXT_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
pub const UI_REFERENCE_HEIGHT: f32 = 768.0; // General UI reference height, gameplay might use its own
pub const UI_BAR_REFERENCE_HEIGHT: f32 = 51.0;

// MSDF Shader Parameters
pub const MSDF_PX_RANGE: f32 = 4.0;

// Misc
pub const MAX_DELTA_TIME: f32 = 0.1;

// Select Music Screen Colors & Palette
pub const MUSIC_WHEEL_BOX_COLOR: [f32;4] = [10.0/255.0, 20.0/255.0, 27.0/255.0, 1.0];
pub const PACK_HEADER_BOX_COLOR: [f32; 4] = [83.0/255.0, 92.0/255.0, 99.0/255.0, 1.0];
pub const SELECTED_PACK_HEADER_BOX_COLOR: [f32; 4] = [95.0/255.0, 104.0/255.0, 110.0/255.0, 1.0];
pub const SELECTED_SONG_BOX_COLOR: [f32; 4] = [39.0/255.0, 47.0/255.0, 53.0/255.0, 1.0];
pub const MUSIC_WHEEL_TEXT_TARGET_PX_HEIGHT_AT_REF_RES: f32 = 23.0;
pub const MUSIC_WHEEL_TEXT_VERTICAL_NUDGE_PX_AT_REF_RES: f32 = 2.0;
pub const SONG_TEXT_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
pub const MUSIC_WHEEL_SONG_TEXT_LEFT_PADDING_REF: f32 = 118.0;
pub const MUSIC_WHEEL_NAV_INITIAL_HOLD_DELAY_MS: u64 = 300;
pub const MUSIC_WHEEL_NAV_REPEAT_SCROLL_INTERVAL_MS: u64 = 40;

pub const PINK_BOX_COLOR: [f32; 4] = [1.0, 71.0 / 255.0, 179.0 / 255.0, 1.0];
pub const TOP_LEFT_BOX_COLOR: [f32; 4] = [230.0 / 255.0, 230.0 / 255.0, 250.0 / 255.0, 1.0];
pub const UI_BOX_DARK_COLOR: [f32;4] = [30.0/255.0, 40.0/255.0, 47.0/255.0, 1.0];

// Reference resolution for select_music.rs layout calculations
pub const LAYOUT_BOXES_REF_RES_WIDTH: f32 = 1280.0;
pub const LAYOUT_BOXES_REF_RES_HEIGHT: f32 = 720.0;

// Gaps for Select Music Screen Layout
pub const VERTICAL_GAP_TOPMOST_TO_ARTIST_BOX_REF: f32 = 5.0;
pub const BAR_TEXT_VERTICAL_NUDGE_PX_AT_REF_RES: f32 = 2.0;

// Artist/BPM Detail Area
pub const DETAIL_HEADER_TEXT_TARGET_PX_HEIGHT_AT_REF_RES: f32 = 27.0;
pub const DETAIL_VALUE_TEXT_TARGET_PX_HEIGHT_AT_REF_RES: f32 = 27.0;
pub const DETAIL_HEADER_TEXT_COLOR: [f32; 4] = [128.0/255.0, 128.0/255.0, 128.0/255.0, 1.0];
pub const ARTIST_HEADER_LEFT_PADDING_REF: f32 = 11.0;
pub const ARTIST_HEADER_TOP_PADDING_REF: f32 = 10.0;
pub const BPM_HEADER_LEFT_PADDING_REF: f32 = 36.0;
pub const HEADER_TO_VALUE_HORIZONTAL_GAP_REF: f32 = 6.0;
pub const ARTIST_TO_BPM_VERTICAL_GAP_REF: f32 = 5.0;
pub const BPM_TO_LENGTH_HORIZONTAL_GAP_REF: f32 = 214.0;

// Simply Love / ITGMania Color Palette for Pack Name TEXTS
pub const PACK_NAME_COLOR_PALETTE: [[f32; 4]; 12] = [
    [1.0, 93.0 / 255.0, 71.0 / 255.0, 1.0],   // #FF5D47
    [1.0, 87.0 / 255.0, 126.0 / 255.0, 1.0],  // #FF577E
    [1.0, 71.0 / 255.0, 179.0 / 255.0, 1.0],  // #FF47B3
    [221.0 / 255.0, 87.0 / 255.0, 1.0, 1.0],  // #DD57FF
    [136.0 / 255.0, 133.0 / 255.0, 1.0, 1.0], // #8885FF
    [61.0 / 255.0, 148.0 / 255.0, 1.0, 1.0],  // #3D94FF
    [0.0, 184.0 / 255.0, 204.0 / 255.0, 1.0], // #00B8CC
    [92.0 / 255.0, 224.0 / 255.0, 135.0 / 255.0, 1.0], // #5CE087
    [174.0 / 255.0, 250.0 / 255.0, 68.0 / 255.0, 1.0], // #AEFA44
    [1.0, 1.0, 0.0, 1.0],                     // #FFFF00
    [1.0, 190.0 / 255.0, 0.0, 1.0],           // #FFBE00
    [1.0, 125.0 / 255.0, 0.0, 1.0],           // #FF7D00
];

pub const GRAPH_BOTTOM_COLOR: [f32; 4] = [0.0 / 255.0, 184.0 / 255.0, 204.0 / 255.0, 1.0]; // Cyan
pub const GRAPH_TOP_COLOR: [f32; 4]    = [130.0 / 255.0, 0.0 / 255.0, 161.0 / 255.0, 1.0]; // Purple

     /*   ITG = {
            color("#21CCE8"),	-- blue
            color("#e29c18"),	-- gold
            color("#66c955"),	-- green
            color("#b45cff"),	-- purple (greatly lightened)
            color("#c9855e"),	-- peach?
            color("#ff3030")	-- red (slightly lightened)
        }, */