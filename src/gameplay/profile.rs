use configparser::ini::Ini;
use log::{info, warn};
use once_cell::sync::Lazy;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::sync::Mutex;

// --- Profile Data ---
const PROFILE_DIR: &str = "save/profiles/00000000";
const PROFILE_INI_PATH: &str = "save/profiles/00000000/profile.ini";
const GROOVESTATS_INI_PATH: &str = "save/profiles/00000000/groovestats.ini";

// This enum is now part of the profile system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundFilter {
    Off,
    Dark,
    Darker,
    Darkest,
}

impl Default for BackgroundFilter {
    fn default() -> Self { BackgroundFilter::Darkest }
}

impl FromStr for BackgroundFilter {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "off" => Ok(Self::Off),
            "dark" => Ok(Self::Dark),
            "darker" => Ok(Self::Darker),
            "darkest" => Ok(Self::Darkest),
            _ => Err(format!("'{}' is not a valid BackgroundFilter setting", s)),
        }
    }
}

impl core::fmt::Display for BackgroundFilter {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Off => write!(f, "Off"),
            Self::Dark => write!(f, "Dark"),
            Self::Darker => write!(f, "Darker"),
            Self::Darkest => write!(f, "Darkest"),
        }
    }
}


#[derive(Debug, Clone)]
pub struct Profile {
    pub display_name: String,
    pub player_initials: String,
    pub groovestats_api_key: String,
    pub groovestats_is_pad_player: bool,
    pub groovestats_username: String,
    pub background_filter: BackgroundFilter,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            display_name: "Player 1".to_string(),
            player_initials: "P1".to_string(),
            groovestats_api_key: "".to_string(),
            groovestats_is_pad_player: false,
            groovestats_username: "".to_string(),
            background_filter: BackgroundFilter::default(),
        }
    }
}

// Global static for the current profile.
static PROFILE: Lazy<Mutex<Profile>> = Lazy::new(|| Mutex::new(Profile::default()));

/// Creates the default profile directory and .ini files if they don't exist.
fn create_default_files() -> Result<(), std::io::Error> {
    info!("Profile files not found, creating defaults in '{}'.", PROFILE_DIR);
    fs::create_dir_all(PROFILE_DIR)?;

    // Create profile.ini
    if !Path::new(PROFILE_INI_PATH).exists() {
        let mut profile_conf = Ini::new();
        let default_profile = Profile::default();
        profile_conf.set("userprofile", "DisplayName", Some(default_profile.display_name));
        profile_conf.set("userprofile", "PlayerInitials", Some(default_profile.player_initials));
        profile_conf.set("PlayerOptions", "BackgroundFilter", Some(default_profile.background_filter.to_string()));
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
    if !Path::new(PROFILE_INI_PATH).exists() || !Path::new(GROOVESTATS_INI_PATH).exists() {
        if let Err(e) = create_default_files() {
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
        profile.player_initials =
            profile_conf.get("userprofile", "PlayerInitials").unwrap_or(default_profile.player_initials.clone());
        profile.background_filter = profile_conf.get("PlayerOptions", "BackgroundFilter")
            .and_then(|s| BackgroundFilter::from_str(&s).ok())
            .unwrap_or(default_profile.background_filter);
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

/// Returns a copy of the currently loaded profile data.
pub fn get() -> Profile {
    PROFILE.lock().unwrap().clone()
}
