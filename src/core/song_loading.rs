use log::{info, warn};
use rssp::{analyze, AnalysisOptions};
use rssp::graph::GraphImageData;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;

// --- Data Structures representing a loaded song ---

#[derive(Clone, Debug)]
pub struct SongData {
    pub title: String,
    pub subtitle: String,
    pub artist: String,
    pub banner_path: Option<PathBuf>,
    pub background_path: Option<PathBuf>,
    pub music_path: Option<PathBuf>,
    pub offset: f32,
    pub sample_start: Option<f32>,
    pub sample_length: Option<f32>,
    pub normalized_bpms: String,
    pub total_length_seconds: i32,
    pub charts: Vec<ChartData>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChartData {
    pub chart_type: String,
    pub difficulty: String,
    pub meter: u32,
    pub step_artist: String,
    pub notes: Vec<u8>, // This is the minimized raw data we will parse
    pub density_graph: Option<GraphImageData>,
    pub short_hash: String,
}

#[derive(Clone, Debug)]
pub struct SongPack {
    pub name: String,
    pub songs: Vec<Arc<SongData>>,
}

// --- Global Song Cache ---

// This static variable will hold all loaded song data. It's initialized once
// when first accessed, and the Mutex ensures safe access.
static SONG_CACHE: Lazy<Mutex<Vec<SongPack>>> = Lazy::new(|| Mutex::new(Vec::new()));

/// Scans the provided root directory (e.g., "songs/") for simfiles,
/// parses them, and populates the global cache. This should be run once at startup.
pub fn scan_and_load_songs(root_path_str: &'static str) {
    info!("Starting simfile scan in '{}'...", root_path_str);
    let root_path = Path::new(root_path_str);
    if !root_path.exists() || !root_path.is_dir() {
        warn!("Songs directory '{}' not found. No songs will be loaded.", root_path_str);
        return;
    }

    let mut loaded_packs = Vec::new();

    // Each directory inside the root is considered a "pack"
    for pack_dir_entry in fs::read_dir(root_path).into_iter().flatten().flatten() {
        let pack_path = pack_dir_entry.path();
        if !pack_path.is_dir() {
            continue;
        }

        let pack_name = pack_path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let mut current_pack = SongPack { name: pack_name, songs: Vec::new() };
        info!("Scanning pack: {}", current_pack.name);

        // Each subdirectory in a pack is a song folder
        for song_dir_entry in fs::read_dir(pack_path).into_iter().flatten().flatten() {
            let song_path = song_dir_entry.path();
            if !song_path.is_dir() {
                continue;
            }

            // Find the .sm or .ssc file within the song folder
            if let Ok(files) = fs::read_dir(&song_path) {
                for file in files.flatten() {
                    let file_path = file.path();
                    if let Some(ext) = file_path.extension().and_then(|s| s.to_str()) {
                        if ext.eq_ignore_ascii_case("sm") || ext.eq_ignore_ascii_case("ssc") {
                            match load_song_from_file(&file_path) {
                                Ok(song_data) => {
                                    current_pack.songs.push(Arc::new(song_data));
                                }
                                Err(e) => warn!("Failed to load '{:?}': {}", file_path, e),
                            }
                            // Found the simfile, move to the next song directory
                            break;
                        }
                    }
                }
            }
        }

        if !current_pack.songs.is_empty() {
            // Sort songs within the pack with a more natural order, grouping songs
            // that start with non-alphanumeric characters (like '[Marathon]') at the end.
            current_pack.songs.sort_by(|a, b| {
                let a_title = a.title.to_lowercase();
                let b_title = b.title.to_lowercase();

                let a_first_char = a_title.chars().next();
                let b_first_char = b_title.chars().next();

                // Treat a title as "special" if it starts with a non-alphanumeric character.
                let a_is_special = a_first_char.map_or(false, |c| !c.is_alphanumeric());
                let b_is_special = b_first_char.map_or(false, |c| !c.is_alphanumeric());

                if a_is_special == b_is_special {
                    // If both are special or both are not, sort them alphabetically.
                    a_title.cmp(&b_title)
                } else if a_is_special {
                    // `a` is special and `b` is not, so `b` should come first.
                    std::cmp::Ordering::Greater
                } else {
                    // `b` is special and `a` is not, so `a` should come first.
                    std::cmp::Ordering::Less
                }
            });
            loaded_packs.push(current_pack);
        }
    }

    // Sort the packs themselves alphabetically by name for consistent ordering.
    loaded_packs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    info!("Finished scan. Found {} packs.", loaded_packs.len());
    *SONG_CACHE.lock().unwrap() = loaded_packs;
}

/// Helper function to parse a single simfile.
fn load_song_from_file(path: &Path) -> Result<SongData, String> {
    let simfile_data = fs::read(path).map_err(|e| format!("Could not read file: {}", e))?;
    let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let options = AnalysisOptions::default(); // Use default parsing options

    let summary = analyze(&simfile_data, extension, options)?;

    let charts: Vec<ChartData> = summary
        .charts
        .into_iter()
        .map(|c| {
            info!(
                "  Chart '{}' [{}] loaded with {} bytes of note data.",
                c.difficulty_str,
                c.rating_str,
                c.notes.len()
            );
            ChartData {
                chart_type: c.step_type_str,
                difficulty: c.difficulty_str,
                meter: c.rating_str.parse().unwrap_or(0),
                step_artist: c.step_artist_str.join(", "),
                notes: c.minimized_note_data,
                density_graph: c.density_graph,
                short_hash: c.short_hash,
            }
        })
        .collect();

    let simfile_dir = path.parent().ok_or_else(|| "Could not determine simfile directory".to_string())?;

    let banner_path = if !summary.banner_path.is_empty() {
        Some(simfile_dir.join(summary.banner_path))
    } else {
        None
    };

    let background_path = if !summary.background_path.is_empty() {
        Some(simfile_dir.join(summary.background_path))
    } else {
        None
    };

    let music_path = if !summary.music_path.is_empty() {
        Some(simfile_dir.join(summary.music_path))
    } else {
        None
    };

    Ok(SongData {
        title: summary.title_str,
        subtitle: summary.subtitle_str,
        artist: summary.artist_str,
        banner_path,
        background_path,
        offset: summary.offset as f32,
        sample_start: if summary.sample_start > 0.0 { Some(summary.sample_start as f32) } else { None },
        sample_length: if summary.sample_length > 0.0 { Some(summary.sample_length as f32) } else { None },
        normalized_bpms: summary.normalized_bpms,
        music_path,
        total_length_seconds: summary.total_length,
        charts,
    })
}

/// Provides safe, read-only access to the global song cache.
pub fn get_song_cache() -> std::sync::MutexGuard<'static, Vec<SongPack>> {
    SONG_CACHE.lock().unwrap()
}
