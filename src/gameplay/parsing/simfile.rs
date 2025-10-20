use crate::gameplay::{
    chart::ChartData,
    song::{set_song_cache, SongData, SongPack},
};
use log::{info, warn};
use rssp::{analyze, AnalysisOptions};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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
    set_song_cache(loaded_packs);
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
                c.minimized_note_data.len()
            );
            ChartData {
                chart_type: c.step_type_str,
                difficulty: c.difficulty_str,
                meter: c.rating_str.parse().unwrap_or(0),
                step_artist: c.step_artist_str.join(", "),
                notes: c.minimized_note_data,
                short_hash: c.short_hash,
                stats: c.stats,
                tech_counts: c.tech_counts,
                total_streams: c.total_streams,
                total_measures: c.total_measures,
                max_nps: c.max_nps,
                detailed_breakdown: c.detailed,
                partial_breakdown: c.partial,
                simple_breakdown: c.simple,
                measure_nps_vec: c.measure_nps_vec,
            }
        })
        .collect();

    let simfile_dir = path.parent().ok_or_else(|| "Could not determine simfile directory".to_string())?;

    // --- Background Path Logic (with autodetection) ---
    let mut background_path_opt: Option<PathBuf> = if !summary.background_path.is_empty() {
        let p = simfile_dir.join(&summary.background_path);
        if p.exists() { Some(p) } else { None }
    } else {
        None
    };

    if background_path_opt.is_none() {
        info!("'{}' - BG path is missing or empty, attempting autodetection.", summary.title_str);
        if let Ok(entries) = fs::read_dir(simfile_dir) {
            let image_files: Vec<PathBuf> = entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file() &&
                    p.extension().and_then(|s| s.to_str()).map_or(false, |ext| {
                        matches!(ext.to_lowercase().as_str(), "png" | "jpg" | "jpeg" | "bmp")
                    })
                })
                .collect();
            
            let mut found_bg: Option<String> = None;

            // Hint-based search first
            for file in &image_files {
                if let Some(file_name) = file.file_name().and_then(|s| s.to_str()) {
                    let file_name_lower = file_name.to_lowercase();
                    if file_name_lower.contains("background") || file_name_lower.contains("bg") {
                        found_bg = Some(file_name.to_string());
                        break;
                    }
                }
            }

            // Dimension-based search if no hint match
            if found_bg.is_none() {
                for file in &image_files {
                    if let Some(file_name) = file.file_name().and_then(|s| s.to_str()) {
                         if let Ok((w, h)) = image::image_dimensions(file) {
                             if w >= 320 && h >= 240 {
                                let aspect = if h > 0 { w as f32 / h as f32 } else { 0.0 };
                                if aspect < 2.0 { // Banners are usually wider than 2:1
                                    found_bg = Some(file_name.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            
            if let Some(bg_filename) = found_bg {
                info!("Autodetected background: '{}'", bg_filename);
                background_path_opt = Some(simfile_dir.join(bg_filename));
            }
        }
    }

    let banner_path = if !summary.banner_path.is_empty() {
        let p = simfile_dir.join(&summary.banner_path);
        if p.exists() { Some(p) } else { None }
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
        banner_path, // Keep original logic for banner
        background_path: background_path_opt,
        offset: summary.offset as f32,
        sample_start: if summary.sample_start > 0.0 { Some(summary.sample_start as f32) } else { None },
        sample_length: if summary.sample_length > 0.0 { Some(summary.sample_length as f32) } else { None },
        min_bpm: summary.min_bpm,
        max_bpm: summary.max_bpm,
        normalized_bpms: summary.normalized_bpms,
        music_path,
        total_length_seconds: summary.total_length,
        charts,
    })
}
