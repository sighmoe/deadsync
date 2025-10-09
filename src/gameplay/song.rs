use crate::gameplay::chart::ChartData;
use once_cell::sync::Lazy;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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
    pub min_bpm: f64,
    pub max_bpm: f64,
    pub normalized_bpms: String,
    pub total_length_seconds: i32,
    pub charts: Vec<ChartData>,
}

#[derive(Clone, Debug)]
pub struct SongPack {
    pub name: String,
    pub songs: Vec<Arc<SongData>>,
}

static SONG_CACHE: Lazy<Mutex<Vec<SongPack>>> = Lazy::new(|| Mutex::new(Vec::new()));

/// Provides safe, read-only access to the global song cache.
pub fn get_song_cache() -> std::sync::MutexGuard<'static, Vec<SongPack>> {
    SONG_CACHE.lock().unwrap()
}

/// A public function to allow the parser to populate the cache.
pub(super) fn set_song_cache(packs: Vec<SongPack>) {
    *SONG_CACHE.lock().unwrap() = packs;
}