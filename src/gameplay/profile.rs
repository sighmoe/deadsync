use configparser::ini::Ini;
use log::{info, warn};
use once_cell::sync::Lazy;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

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

/// Creates the default profile directory and .ini files if they don't exist.
fn create_default_files() -> Result<(), std::io::Error> {
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
