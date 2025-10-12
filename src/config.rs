// ===== FILE: src/config.rs =====
use crate::core::gfx::BackendType;
use configparser::ini::Ini;
use log::{info, warn};
use once_cell::sync::Lazy;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::sync::Mutex;

const CONFIG_PATH: &str = "deadsync.ini";

#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub vsync: bool,
    pub windowed: bool,
    pub show_stats: bool,
    pub display_width: u32,
    pub display_height: u32,
    pub video_renderer: BackendType,
    pub simply_love_color: i32,
    pub global_offset_seconds: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            vsync: false,
            windowed: true,
            show_stats: false,
            display_width: 1600,
            display_height: 900,
            video_renderer: BackendType::Vulkan,
            simply_love_color: 2, // Corresponds to DEFAULT_COLOR_INDEX
            global_offset_seconds: -0.008,
        }
    }
}

// Global, mutable configuration instance.
static CONFIG: Lazy<Mutex<Config>> = Lazy::new(|| Mutex::new(Config::default()));

// --- Profile Data ---
const PROFILE_DIR: &str = "save/profiles/00000000";
const PROFILE_INI_PATH: &str = "save/profiles/00000000/profile.ini";
const GROOVESTATS_INI_PATH: &str = "save/profiles/00000000/groovestats.ini";

#[derive(Debug, Clone)]
pub struct Profile {
    pub display_name: String,
    pub groovestats_api_key: String,
    pub groovestats_is_pad_player: bool,
    pub groovestats_username: String,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            display_name: "Player 1".to_string(),
            groovestats_api_key: "".to_string(),
            groovestats_is_pad_player: false,
            groovestats_username: "".to_string(),
        }
    }
}

// Global static for the current profile.
static PROFILE: Lazy<Mutex<Profile>> = Lazy::new(|| Mutex::new(Profile::default()));


// --- File I/O ---

fn create_default_config_file() -> Result<(), std::io::Error> {
    info!("'{}' not found, creating with default values.", CONFIG_PATH);
    let mut conf = Ini::new();
    let default = Config::default();

    conf.set("Options", "Vsync", Some((if default.vsync { "1" } else { "0" }).to_string()));
    conf.set("Options", "Windowed", Some((if default.windowed { "1" } else { "0" }).to_string()));
    conf.set("Options", "ShowStats", Some((if default.show_stats { "1" } else { "0" }).to_string()));
    conf.set("Options", "DisplayWidth", Some(default.display_width.to_string()));
    conf.set("Options", "DisplayHeight", Some(default.display_height.to_string()));
    conf.set("Options", "VideoRenderer", Some(default.video_renderer.to_string()));
    conf.set("Options", "GlobalOffsetSeconds", Some(default.global_offset_seconds.to_string()));
    conf.set("Theme", "SimplyLoveColor", Some(default.simply_love_color.to_string()));

    conf.write(CONFIG_PATH)
}

/// Creates the default profile directory and .ini files if they don't exist.
fn create_default_profile_files() -> Result<(), std::io::Error> {
    info!("Profile files not found, creating defaults in '{}'.", PROFILE_DIR);
    fs::create_dir_all(PROFILE_DIR)?;

    // Create profile.ini
    if !Path::new(PROFILE_INI_PATH).exists() {
        let mut profile_conf = Ini::new();
        profile_conf.set("userprofile", "DisplayName", Some("Player 1".to_string()));
        profile_conf.write(PROFILE_INI_PATH)?;
    }

    // Create groovestats.ini
    if !Path::new(GROOVESTATS_INI_PATH).exists() {
        let mut gs_conf = Ini::new();
        gs_conf.set("GrooveStats", "ApiKey", Some("".to_string()));
        gs_conf.set("GrooveStats", "IsPadPlayer", Some("0".to_string()));
        gs_conf.set("GrooveStats", "Username", Some("".to_string()));
        gs_conf.write(GROOVESTATS_INI_PATH)?;
    }

    Ok(())
}

pub fn load() {
    // --- Load main deadsync.ini ---
    if !std::path::Path::new(CONFIG_PATH).exists() {
        if let Err(e) = create_default_config_file() {
            warn!("Failed to create default config file: {}", e);
        }
    }

    let mut conf = Ini::new();
    match conf.load(CONFIG_PATH) {
        Ok(_) => {
            let mut cfg = CONFIG.lock().unwrap();
            let default = Config::default();
            
            cfg.vsync = conf.get("Options", "Vsync").and_then(|v| v.parse::<u8>().ok()).map_or(default.vsync, |v| v != 0);
            cfg.windowed = conf.get("Options", "Windowed").and_then(|v| v.parse::<u8>().ok()).map_or(default.windowed, |v| v != 0);
            cfg.show_stats = conf.get("Options", "ShowStats").and_then(|v| v.parse::<u8>().ok()).map_or(default.show_stats, |v| v != 0);
            cfg.display_width = conf.get("Options", "DisplayWidth").and_then(|v| v.parse().ok()).unwrap_or(default.display_width);
            cfg.display_height = conf.get("Options", "DisplayHeight").and_then(|v| v.parse().ok()).unwrap_or(default.display_height);
            cfg.video_renderer = conf.get("Options", "VideoRenderer")
                .and_then(|s| BackendType::from_str(&s).ok())
                .unwrap_or(default.video_renderer);
            cfg.global_offset_seconds = conf.get("Options", "GlobalOffsetSeconds").and_then(|v| v.parse().ok()).unwrap_or(default.global_offset_seconds);
            cfg.simply_love_color = conf.get("Theme", "SimplyLoveColor").and_then(|v| v.parse().ok()).unwrap_or(default.simply_love_color);
            
            info!("Configuration loaded from '{}'.", CONFIG_PATH);
        }
        Err(e) => {
            warn!("Failed to load '{}': {}. Using default values.", CONFIG_PATH, e);
        }
    }

    // --- Load profile files ---
    if !Path::new(PROFILE_INI_PATH).exists() || !Path::new(GROOVESTATS_INI_PATH).exists() {
        if let Err(e) = create_default_profile_files() {
            warn!("Failed to create default profile files: {}", e);
            // Proceed with default struct values.
            return;
        }
    }

    let mut profile = PROFILE.lock().unwrap();
    let default_profile = Profile::default();

    // Load profile.ini
    let mut profile_conf = Ini::new();
    if profile_conf.load(PROFILE_INI_PATH).is_ok() {
        profile.display_name =
            profile_conf.get("userprofile", "DisplayName").unwrap_or(default_profile.display_name.clone());
    } else {
        warn!("Failed to load '{}', using default display name.", PROFILE_INI_PATH);
    }

    // Load groovestats.ini
    let mut gs_conf = Ini::new();
    if gs_conf.load(GROOVESTATS_INI_PATH).is_ok() {
        profile.groovestats_api_key =
            gs_conf.get("GrooveStats", "ApiKey").unwrap_or(default_profile.groovestats_api_key.clone());
        profile.groovestats_is_pad_player = gs_conf
            .get("GrooveStats", "IsPadPlayer")
            .and_then(|v| v.parse::<u8>().ok())
            .map_or(default_profile.groovestats_is_pad_player, |v| v != 0);
        profile.groovestats_username =
            gs_conf.get("GrooveStats", "Username").unwrap_or(default_profile.groovestats_username.clone());
    } else {
        warn!("Failed to load '{}', using default GrooveStats info.", GROOVESTATS_INI_PATH);
    }
}

fn save() {
    let cfg = CONFIG.lock().unwrap();
    let mut conf = Ini::new();

    conf.set("Options", "Vsync", Some((if cfg.vsync { "1" } else { "0" }).to_string()));
    conf.set("Options", "Windowed", Some((if cfg.windowed { "1" } else { "0" }).to_string()));
    conf.set("Options", "ShowStats", Some((if cfg.show_stats { "1" } else { "0" }).to_string()));
    conf.set("Options", "DisplayWidth", Some(cfg.display_width.to_string()));
    conf.set("Options", "DisplayHeight", Some(cfg.display_height.to_string()));
    conf.set("Options", "VideoRenderer", Some(cfg.video_renderer.to_string()));
    conf.set("Options", "GlobalOffsetSeconds", Some(cfg.global_offset_seconds.to_string()));
    conf.set("Theme", "SimplyLoveColor", Some(cfg.simply_love_color.to_string()));
    
    if let Err(e) = conf.write(CONFIG_PATH) {
        warn!("Failed to save config file: {}", e);
    }
}

pub fn get() -> Config {
    *CONFIG.lock().unwrap()
}

/// Returns a copy of the currently loaded profile data.
pub fn get_profile() -> Profile {
    PROFILE.lock().unwrap().clone()
}

pub fn update_simply_love_color(index: i32) {
    {
        let mut cfg = CONFIG.lock().unwrap();
        // No change, no need to write to disk.
        if cfg.simply_love_color == index { return; }
        cfg.simply_love_color = index;
    }
    save();
}

#[allow(dead_code)]
pub fn update_global_offset(offset: f32) {
    {
        let mut cfg = CONFIG.lock().unwrap();
        if (cfg.global_offset_seconds - offset).abs() < f32::EPSILON { return; }
        cfg.global_offset_seconds = offset;
    }
    save();
}