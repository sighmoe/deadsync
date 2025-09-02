// ===== FILE: src/config.rs =====
use configparser::ini::Ini;
use log::{info, warn};
use once_cell::sync::Lazy;
use std::sync::Mutex;

const CONFIG_PATH: &str = "deadsync.ini";

#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub vsync: bool,
    pub windowed: bool,
    pub show_stats: bool,
    pub display_width: u32,
    pub display_height: u32,
    pub simply_love_color: i32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            vsync: false,
            windowed: true,
            show_stats: false,
            display_width: 1600,
            display_height: 900,
            simply_love_color: 2, // Corresponds to DEFAULT_COLOR_INDEX
        }
    }
}

// Global, mutable configuration instance.
static CONFIG: Lazy<Mutex<Config>> = Lazy::new(|| Mutex::new(Config::default()));

fn create_default_config_file() -> Result<(), std::io::Error> {
    info!("'{}' not found, creating with default values.", CONFIG_PATH);
    let mut conf = Ini::new();
    let default = Config::default();

    conf.set("Options", "Vsync", Some((if default.vsync { "1" } else { "0" }).to_string()));
    conf.set("Options", "Windowed", Some((if default.windowed { "1" } else { "0" }).to_string()));
    conf.set("Options", "ShowStats", Some((if default.show_stats { "1" } else { "0" }).to_string()));
    conf.set("Options", "DisplayWidth", Some(default.display_width.to_string()));
    conf.set("Options", "DisplayHeight", Some(default.display_height.to_string()));
    conf.set("Theme", "SimplyLoveColor", Some(default.simply_love_color.to_string()));

    conf.write(CONFIG_PATH)
}

pub fn load() {
    if !std::path::Path::new(CONFIG_PATH).exists() {
        if let Err(e) = create_default_config_file() {
            warn!("Failed to create default config file: {}", e);
            // The app will proceed with the default config struct in the global static.
            return;
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
            cfg.simply_love_color = conf.get("Theme", "SimplyLoveColor").and_then(|v| v.parse().ok()).unwrap_or(default.simply_love_color);
            
            info!("Configuration loaded from '{}'.", CONFIG_PATH);
        }
        Err(e) => {
            warn!("Failed to load '{}': {}. Using default values.", CONFIG_PATH, e);
        }
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
    conf.set("Theme", "SimplyLoveColor", Some(cfg.simply_love_color.to_string()));
    
    if let Err(e) = conf.write(CONFIG_PATH) {
        warn!("Failed to save config file: {}", e);
    }
}

pub fn get() -> Config {
    *CONFIG.lock().unwrap()
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