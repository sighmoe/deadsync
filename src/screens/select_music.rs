use crate::assets::{AssetManager, FontId, SoundId, TextureId};
use crate::audio::AudioManager;
use crate::config;
use crate::graphics::renderer::{DescriptorSetId, Renderer};
use crate::graphics::vulkan_base::VulkanBase;
use crate::parsing::simfile::{SongInfo};
use crate::state::{AppState, MusicWheelEntry, NavDirection, SelectMusicState, VirtualKeyCode};
use ash::vk;
use cgmath::{Rad, Vector3};
use log::{error, info, warn};
use std::collections::HashMap;
use std::f32::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::event::{ElementState, KeyEvent};

pub const DIFFICULTY_NAMES: [&str; 5] = ["Beginner", "Easy", "Medium", "Hard", "Challenge"];
pub(crate) const SELECTION_START_PLAY_DELAY: Duration = Duration::from_millis(100);

// Helper function to check if a specific difficulty index has a playable chart for the given song
pub(crate) fn is_difficulty_playable(song: &Arc<SongInfo>, difficulty_index: usize) -> bool {
    if difficulty_index >= DIFFICULTY_NAMES.len() {
        return false;
    }
    let target_difficulty_name = DIFFICULTY_NAMES[difficulty_index];
    song.charts.iter().any(|c| {
        c.difficulty.eq_ignore_ascii_case(target_difficulty_name)
            && c.stepstype == "dance-single"
            && c.processed_data
                .as_ref()
                .map_or(false, |pd| !pd.measures.is_empty())
    })
}

fn lerp_color(color_a: [f32; 4], color_b: [f32; 4], t: f32) -> [f32; 4] {
    [
        color_a[0] * (1.0 - t) + color_b[0] * t,
        color_a[1] * (1.0 - t) + color_b[1] * t,
        color_a[2] * (1.0 - t) + color_b[2] * t,
        color_a[3] * (1.0 - t) + color_b[3] * t,
    ]
}

fn format_duration_flexible(total_seconds_f: f32) -> String {
    if total_seconds_f < 0.0 {
        return "0:00".to_string();
    }
    let total_seconds = total_seconds_f.round() as u32;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else if minutes >= 10 || minutes == 0 {
        format!("{:02}:{:02}", minutes, seconds)
    } else {
        format!("{}:{:02}", minutes, seconds)
    }
}

const DOUBLE_TAP_WINDOW: Duration = Duration::from_millis(300);

// Moved from App
fn find_pack_banner_for_wheel(pack_folder_path: &Path) -> Option<PathBuf> {
    if !pack_folder_path.is_dir() {
        return None;
    }
    let banner_name_patterns = ["banner", "ban", "bn"];

    let mut found_banner: Option<PathBuf> = None;

    for pattern_base in banner_name_patterns {
        // Ensure read_dir result is handled properly
        let entries = match fs::read_dir(pack_folder_path) {
            Ok(e) => e,
            Err(_) => return found_banner, // Or None if error implies no banner
        };

        for entry_res in entries {
            if let Ok(entry) = entry_res {
                let path = entry.path();
                if path.is_file() {
                    if let Some(filename_osstr) = path.file_name() {
                        if let Some(filename_str) = filename_osstr.to_str() {
                            let filename_lower = filename_str.to_lowercase();
                            if filename_lower.contains(pattern_base)
                                && filename_lower.ends_with(".png")
                            {
                                if filename_lower == format!("{}.png", pattern_base) {
                                    return Some(path);
                                }
                                if found_banner.is_none() {
                                    found_banner = Some(path.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
        if found_banner.is_some() && pattern_base == "banner" {
            return found_banner;
        }
    }
    found_banner
}

// Moved from App and adapted
pub(crate) fn rebuild_music_wheel_entries_logic(
    state: &mut SelectMusicState,
    song_library: &[SongInfo],
    pack_colors: &HashMap<String, [f32; 4]>,
) {
    let mut pack_total_durations: HashMap<String, f32> = HashMap::new();
    for song_info in song_library {
        let pack_name = song_info
            .folder_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown Pack")
            .to_string();

        let song_duration_for_pack_sum = song_info
            .charts
            .iter()
            .find_map(|c| c.calculated_length_sec)
            .unwrap_or(0.0);

        *pack_total_durations.entry(pack_name).or_insert(0.0) += song_duration_for_pack_sum;
    }

    let mut pack_song_counts: HashMap<String, usize> = HashMap::new();
    for song_info in song_library {
        let has_dance_single_chart = song_info
            .charts
            .iter()
            .any(|c| c.stepstype == "dance-single");

        if has_dance_single_chart {
            let pack_name = song_info
                .folder_path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown Pack")
                .to_string();
            *pack_song_counts.entry(pack_name).or_insert(0) += 1;
        }
    }

    let mut new_entries = Vec::new();
    let mut current_pack_name_in_library = String::new();

    let pack_to_focus_on: Option<String> = state.expanded_pack_name.clone().or_else(|| {
        state
            .entries
            .get(state.selected_index)
            .and_then(|entry| match entry {
                MusicWheelEntry::PackHeader { name, .. } => Some(name.clone()),
                _ => None,
            })
    });

    for song_info in song_library {
        let pack_name_for_song = song_info
            .folder_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("Unknown Pack")
            .to_string();

        if pack_name_for_song != current_pack_name_in_library {
            let color = pack_colors
                .get(&pack_name_for_song)
                .cloned()
                .unwrap_or(config::MENU_NORMAL_COLOR);

            let pack_banner_path = song_info
                .folder_path
                .parent()
                .and_then(|pack_dir| find_pack_banner_for_wheel(pack_dir)); // Use local helper

            let total_duration = pack_total_durations.get(&pack_name_for_song).copied();
            let song_count = pack_song_counts
                .get(&pack_name_for_song)
                .copied()
                .unwrap_or(0);

            new_entries.push(MusicWheelEntry::PackHeader {
                name: pack_name_for_song.clone(),
                color,
                banner_path: pack_banner_path,
                total_duration_sec: total_duration,
                song_count,
            });
            current_pack_name_in_library = pack_name_for_song.clone();
        }

        if let Some(expanded_name) = &state.expanded_pack_name {
            if *expanded_name == pack_name_for_song {
                new_entries.push(MusicWheelEntry::Song(Arc::new(song_info.clone())));
            }
        }
    }
    state.entries = new_entries;

    let mut new_selected_idx = 0;
    if let Some(focus_pack_name_str) = pack_to_focus_on {
        if let Some(idx) = state.entries.iter().position(|entry| match entry {
            MusicWheelEntry::PackHeader { name, .. } => name == &focus_pack_name_str,
            _ => false,
        }) {
            new_selected_idx = idx;
        }
    }

    if !state.entries.is_empty() {
        state.selected_index = new_selected_idx.min(state.entries.len() - 1);
    } else {
        state.selected_index = 0;
    }

    // Reset preview state - caller (App) will handle audio_manager.stop_preview()
    // if this is part of a larger state transition. For direct rebuilds, this is fine.
    state.preview_audio_path = None;
    state.preview_playback_started_at = None;
    state.is_awaiting_preview_restart = false;
    state.selection_landed_at = None;
    state.is_preview_actions_scheduled = false;
}

// Moved from App and adapted
pub(crate) fn start_preview_playback_logic(
    state: &mut SelectMusicState,
    audio_manager: &mut AudioManager,
) {
    state.preview_playback_started_at = None; // Ensure it's reset before attempting to set

    if let Some(audio_path) = &state.preview_audio_path {
        if let Some(start_sec) = state.preview_sample_start_sec {
            let duration_sec = state.preview_sample_length_sec;
            match audio_manager.play_preview(audio_path, 0.7, start_sec, duration_sec) {
                Ok(_) => {
                    state.preview_playback_started_at = Some(Instant::now());
                    info!(
                        "Preview playback started for {:?} at {:.2}s",
                        audio_path.file_name().unwrap_or_default(),
                        start_sec
                    );
                }
                Err(e) => error!("Failed to start preview playback: {}", e),
            }
        } else {
            warn!("No sample start time for song, cannot play preview.");
        }
    }
}

// Moved from App and adapted
pub(crate) fn handle_selection_change_logic(
    state: &mut SelectMusicState,
    asset_manager: &mut AssetManager,
    audio_manager: &mut AudioManager,
    renderer: &Renderer,
    vulkan_base: &VulkanBase,
) {
    let prev_preview_audio_path = state.preview_audio_path.clone();
    let prev_preview_sample_start = state.preview_sample_start_sec;
    let prev_preview_sample_length = state.preview_sample_length_sec;

    let mut new_graph_key_for_current_selection: Option<String> = None;
    let mut chart_data_for_graph_generation: Option<
        Arc<crate::parsing::simfile::ProcessedChartData>,
    > = None;

    let current_index = state.selected_index;
    if let Some(selected_entry) = state.entries.get(current_index) {
        match selected_entry {
            MusicWheelEntry::Song(selected_song_arc) => {
                asset_manager.load_song_banner(
                    vulkan_base, // Pass VulkanBase
                    renderer,
                    selected_song_arc,
                );
                state.preview_audio_path = selected_song_arc.audio_path.clone();
                state.preview_sample_start_sec = selected_song_arc.sample_start;
                state.preview_sample_length_sec = selected_song_arc.sample_length;

                let mut difficulty_index_to_use = state.selected_difficulty_index;
                if !is_difficulty_playable(selected_song_arc, difficulty_index_to_use) {
                    for i in 0..DIFFICULTY_NAMES.len() {
                        if is_difficulty_playable(selected_song_arc, i) {
                            info!(
                                "Auto-adjusting difficulty for '{}' from index {} to {} ('{}')",
                                selected_song_arc.title,
                                difficulty_index_to_use,
                                i,
                                DIFFICULTY_NAMES[i]
                            );
                            difficulty_index_to_use = i;
                            break;
                        }
                    }
                }
                state.selected_difficulty_index = difficulty_index_to_use;

                let target_difficulty_name = DIFFICULTY_NAMES[difficulty_index_to_use];
                if let Some(chart_info) = selected_song_arc.charts.iter().find(|c| {
                    c.difficulty.eq_ignore_ascii_case(target_difficulty_name)
                        && c.stepstype == "dance-single"
                        && c.processed_data.as_ref().map_or(false, |pd| {
                            !pd.measure_nps_vec.is_empty() && pd.max_nps > 0.001
                        })
                }) {
                    if let Some(pd) = &chart_info.processed_data {
                        new_graph_key_for_current_selection = Some(format!(
                            "{}//{}",
                            selected_song_arc.title, chart_info.difficulty
                        ));
                        chart_data_for_graph_generation = Some(Arc::new(pd.clone()));
                    }
                } else {
                    if let Some(chart_info_fallback) = selected_song_arc.charts.iter().find(|c| {
                        c.processed_data.as_ref().map_or(false, |pd| {
                            !pd.measure_nps_vec.is_empty() && pd.max_nps > 0.001
                        })
                    }) {
                        if let Some(pd_fallback) = &chart_info_fallback.processed_data {
                            new_graph_key_for_current_selection = Some(format!(
                                "{}//{} (fallback)",
                                selected_song_arc.title, chart_info_fallback.difficulty
                            ));
                            chart_data_for_graph_generation = Some(Arc::new(pd_fallback.clone()));
                            warn!("NPS Graph: Chart for difficulty '{}' not found or unprocessable for '{}'. Using fallback: '{}'", target_difficulty_name, selected_song_arc.title, chart_info_fallback.difficulty);
                        }
                    }
                }
            }
            MusicWheelEntry::PackHeader {
                name: _,
                color: _,
                banner_path,
                ..
            } => {
                info!(
                    "Selected a pack header ({}), attempting to load pack banner.",
                    current_index
                );
                asset_manager.load_pack_banner(
                    vulkan_base, // Pass VulkanBase
                    renderer,
                    banner_path.as_deref(),
                );
                state.preview_audio_path = None;
                state.preview_sample_start_sec = None;
                state.preview_sample_length_sec = None;
            }
        }
    } else {
        warn!(
            "Selection changed in Music Select, but index {} is out of bounds ({} entries). Loading fallback and clearing preview actions.",
            current_index,
            state.entries.len()
        );
        if let Some(fallback_res) =
            asset_manager.get_texture(crate::assets::TextureId::FallbackBanner)
        {
            renderer.update_texture_descriptor(
                &vulkan_base.device,
                crate::graphics::renderer::DescriptorSetId::DynamicBanner,
                fallback_res,
            );
        }
        state.preview_audio_path = None;
        state.preview_sample_start_sec = None;
        state.preview_sample_length_sec = None;
    }

    let current_preview_audio_path = state.preview_audio_path.clone();
    let current_preview_sample_start = state.preview_sample_start_sec;
    let current_preview_sample_length = state.preview_sample_length_sec;

    let preview_parameters_changed = prev_preview_audio_path != current_preview_audio_path
        || prev_preview_sample_start != current_preview_sample_start
        || prev_preview_sample_length != current_preview_sample_length
        || (prev_preview_audio_path.is_some() && current_preview_audio_path.is_none())
        || (prev_preview_audio_path.is_none() && current_preview_audio_path.is_some());

    if preview_parameters_changed {
        info!("Preview parameters changed. Stopping current preview and rescheduling.");
        audio_manager.stop_preview();
        state.preview_playback_started_at = None;
        state.is_awaiting_preview_restart = false;

        if state.preview_audio_path.is_some() {
            state.selection_landed_at = Some(Instant::now());
            state.is_preview_actions_scheduled = true;
            info!(
                "New preview actions (play@{}ms) scheduled for {:?}.",
                SELECTION_START_PLAY_DELAY.as_millis(),
                state
                    .preview_audio_path
                    .as_ref()
                    .and_then(|p| p.file_name())
            );
        } else {
            state.selection_landed_at = None;
            state.is_preview_actions_scheduled = false;
        }
    } else {
        info!("Preview parameters unchanged. Preview will continue or maintain its schedule. Only SFX played for difficulty change.");
        if state.preview_audio_path.is_some()
            && state.selection_landed_at.is_none()
            && state.preview_playback_started_at.is_none()
        {
            if !state.is_awaiting_preview_restart && !state.is_preview_actions_scheduled {
                state.selection_landed_at = Some(Instant::now());
                state.is_preview_actions_scheduled = true;
                info!(
                    "Re-asserting preview actions (play@{}ms) for {:?} as parameters were same but no playback/restart active.",
                    SELECTION_START_PLAY_DELAY.as_millis(),
                    state.preview_audio_path.as_ref().and_then(|p| p.file_name())
                );
            }
        }
    }

    if state.current_graph_song_chart_key != new_graph_key_for_current_selection {
        if let Some(mut old_graph_tex) = state.current_graph_texture.take() {
            info!("Destroying old NPS graph texture (key change or no graph needed).");
            old_graph_tex.destroy(&vulkan_base.device);
        }
        state.current_graph_song_chart_key = new_graph_key_for_current_selection.clone();
    }

    if let (Some(key_str), Some(pd_arc)) = (
        new_graph_key_for_current_selection,
        chart_data_for_graph_generation,
    ) {
        if state.current_graph_texture.is_none() {
            info!("Generating NPS graph for: {}", key_str);
            let nps_vec_f64: Vec<f64> = pd_arc.measure_nps_vec.iter().map(|&f| f as f64).collect();
            match crate::parsing::graph::generate_density_graph_rgba(
                &nps_vec_f64,
                pd_arc.max_nps as f64,
            ) {
                Ok(graph_image_data) => {
                    match crate::graphics::texture::create_texture_from_rgba_data(
                        vulkan_base, // Pass VulkanBase
                        graph_image_data.width,
                        graph_image_data.height,
                        &graph_image_data.data,
                        "NPS_Graph_Texture",
                    ) {
                        Ok(tex_res) => {
                            renderer.update_texture_descriptor(
                                &vulkan_base.device,
                                crate::graphics::renderer::DescriptorSetId::NpsGraph,
                                &tex_res,
                            );
                            state.current_graph_texture = Some(tex_res);
                            info!("NPS graph texture created and descriptor updated.");
                        }
                        Err(e) => error!("Failed to create NPS graph texture from data: {}", e),
                    }
                }
                Err(e) => error!("Failed to generate NPS graph image data: {}", e),
            }
        }
    } else if state.current_graph_texture.is_some() {
        if let Some(mut old_graph_tex) = state.current_graph_texture.take() {
            info!("Clearing NPS graph texture as current selection does not require one.");
            old_graph_tex.destroy(&vulkan_base.device);
        }
        renderer.update_texture_descriptor(
            &vulkan_base.device,
            crate::graphics::renderer::DescriptorSetId::NpsGraph,
            &renderer.solid_white_texture,
        );
    }
}

pub fn handle_input(
    key_event: &KeyEvent,
    state: &mut SelectMusicState,
    audio_manager: &AudioManager,
) -> (Option<AppState>, bool) {
    let mut song_or_pack_selection_changed = false;
    let mut difficulty_selection_changed = false;

    if let Some(virtual_keycode) =
        crate::state::key_to_virtual_keycode(key_event.logical_key.clone())
    {
        let num_entries = state.entries.len();
        let is_song_selected = num_entries > 0
            && matches!(
                state.entries.get(state.selected_index),
                Some(MusicWheelEntry::Song(_))
            );

        match key_event.state {
            ElementState::Pressed => {
                if virtual_keycode == VirtualKeyCode::Up || virtual_keycode == VirtualKeyCode::Down
                {
                    state.active_chord_keys.insert(virtual_keycode);
                }

                if !key_event.repeat {
                    let mut combo_action_taken = false;

                    if state.active_chord_keys.contains(&VirtualKeyCode::Up)
                        && state.active_chord_keys.contains(&VirtualKeyCode::Down)
                    {
                        if state.expanded_pack_name.is_some() {
                            info!("Up+Down combo: Collapsing pack.");
                            state.expanded_pack_name = None;
                            audio_manager.play_sfx(SoundId::MenuExpandCollapse);
                            song_or_pack_selection_changed = true;
                            state.selection_animation_timer = 0.0;
                            state.last_difficulty_nav_key = None;
                            state.last_difficulty_nav_time = None;
                            combo_action_taken = true;
                        }
                    }

                    if combo_action_taken
                        && (virtual_keycode == VirtualKeyCode::Up
                            || virtual_keycode == VirtualKeyCode::Down)
                    {
                        // Skip other actions for this key press
                    } else {
                        match virtual_keycode {
                            VirtualKeyCode::Left => {
                                if num_entries > 0 {
                                    let old_index = state.selected_index;
                                    state.selected_index = if state.selected_index == 0 {
                                        num_entries - 1
                                    } else {
                                        state.selected_index - 1
                                    };
                                    if state.selected_index != old_index {
                                        audio_manager.play_sfx(SoundId::MenuChange);
                                        song_or_pack_selection_changed = true;
                                        state.selection_animation_timer = 0.0;
                                        state.last_difficulty_nav_key = None;
                                        state.last_difficulty_nav_time = None;
                                    }
                                    state.nav_key_held_direction = Some(NavDirection::Up);
                                    state.nav_key_held_since = Some(Instant::now());
                                    state.nav_key_last_scrolled_at = Some(Instant::now());
                                }
                            }
                            VirtualKeyCode::Right => {
                                if num_entries > 0 {
                                    let old_index = state.selected_index;
                                    state.selected_index = (state.selected_index + 1) % num_entries;
                                    if state.selected_index != old_index {
                                        audio_manager.play_sfx(SoundId::MenuChange);
                                        song_or_pack_selection_changed = true;
                                        state.selection_animation_timer = 0.0;
                                        state.last_difficulty_nav_key = None;
                                        state.last_difficulty_nav_time = None;
                                    }
                                    state.nav_key_held_direction = Some(NavDirection::Down);
                                    state.nav_key_held_since = Some(Instant::now());
                                    state.nav_key_last_scrolled_at = Some(Instant::now());
                                }
                            }
                            VirtualKeyCode::Up => {
                                if is_song_selected {
                                    let now = Instant::now();
                                    if state.last_difficulty_nav_key == Some(VirtualKeyCode::Up)
                                        && state.last_difficulty_nav_time.map_or(false, |t| {
                                            now.duration_since(t) < DOUBLE_TAP_WINDOW
                                        })
                                    {
                                        if let Some(MusicWheelEntry::Song(selected_song_arc)) =
                                            state.entries.get(state.selected_index)
                                        {
                                            let original_diff_idx = state.selected_difficulty_index;
                                            let mut new_diff_idx = state.selected_difficulty_index;
                                            loop {
                                                if new_diff_idx == 0 {
                                                    break;
                                                }
                                                new_diff_idx -= 1;
                                                if is_difficulty_playable(
                                                    selected_song_arc,
                                                    new_diff_idx,
                                                ) {
                                                    state.selected_difficulty_index = new_diff_idx;
                                                    break;
                                                }
                                            }
                                            if state.selected_difficulty_index != original_diff_idx
                                            {
                                                audio_manager.play_sfx(SoundId::DifficultyEasier);
                                                difficulty_selection_changed = true;
                                            }
                                        }
                                        state.last_difficulty_nav_key = None;
                                        state.last_difficulty_nav_time = None;
                                    } else {
                                        state.last_difficulty_nav_key = Some(VirtualKeyCode::Up);
                                        state.last_difficulty_nav_time = Some(now);
                                    }
                                }
                            }
                            VirtualKeyCode::Down => {
                                if is_song_selected {
                                    let now = Instant::now();
                                    if state.last_difficulty_nav_key == Some(VirtualKeyCode::Down)
                                        && state.last_difficulty_nav_time.map_or(false, |t| {
                                            now.duration_since(t) < DOUBLE_TAP_WINDOW
                                        })
                                    {
                                        if let Some(MusicWheelEntry::Song(selected_song_arc)) =
                                            state.entries.get(state.selected_index)
                                        {
                                            let original_diff_idx = state.selected_difficulty_index;
                                            let mut new_diff_idx = state.selected_difficulty_index;
                                            loop {
                                                if new_diff_idx >= DIFFICULTY_NAMES.len() - 1 {
                                                    break;
                                                }
                                                new_diff_idx += 1;
                                                if is_difficulty_playable(
                                                    selected_song_arc,
                                                    new_diff_idx,
                                                ) {
                                                    state.selected_difficulty_index = new_diff_idx;
                                                    break;
                                                }
                                            }
                                            if state.selected_difficulty_index != original_diff_idx
                                            {
                                                audio_manager.play_sfx(SoundId::DifficultyHarder);
                                                difficulty_selection_changed = true;
                                            }
                                        }
                                        state.last_difficulty_nav_key = None;
                                        state.last_difficulty_nav_time = None;
                                    } else {
                                        state.last_difficulty_nav_key = Some(VirtualKeyCode::Down);
                                        state.last_difficulty_nav_time = Some(now);
                                    }
                                }
                            }
                            VirtualKeyCode::Enter => {
                                if num_entries > 0 {
                                    if let Some(entry_clone) =
                                        state.entries.get(state.selected_index).cloned()
                                    {
                                        match entry_clone {
                                            MusicWheelEntry::Song(selected_song_arc) => {
                                                let target_difficulty_name = DIFFICULTY_NAMES
                                                    [state.selected_difficulty_index];
                                                if selected_song_arc.charts.iter().any(|c| {
                                                    c.difficulty.eq_ignore_ascii_case(
                                                        target_difficulty_name,
                                                    ) && c.stepstype == "dance-single"
                                                        && c.processed_data.is_some()
                                                        && !c
                                                            .processed_data
                                                            .as_ref()
                                                            .unwrap()
                                                            .measures
                                                            .is_empty()
                                                }) {
                                                    audio_manager.play_sfx(SoundId::MenuStart);
                                                    state.last_difficulty_nav_key = None;
                                                    state.last_difficulty_nav_time = None;
                                                    return (
                                                        Some(AppState::Gameplay),
                                                        song_or_pack_selection_changed
                                                            || difficulty_selection_changed,
                                                    );
                                                } else {
                                                    warn!("No playable chart found for '{}' at difficulty '{}'. Cannot start.", selected_song_arc.title, target_difficulty_name);
                                                }
                                            }
                                            MusicWheelEntry::PackHeader {
                                                name: pack_name_str,
                                                ..
                                            } => {
                                                audio_manager.play_sfx(SoundId::MenuExpandCollapse);
                                                if state.expanded_pack_name.as_ref()
                                                    == Some(&pack_name_str)
                                                {
                                                    state.expanded_pack_name = None;
                                                } else {
                                                    state.expanded_pack_name =
                                                        Some(pack_name_str.clone());
                                                }
                                                song_or_pack_selection_changed = true;
                                                state.selection_animation_timer = 0.0;
                                                state.last_difficulty_nav_key = None;
                                                state.last_difficulty_nav_time = None;
                                            }
                                        }
                                    }
                                }
                            }
                            VirtualKeyCode::Escape => {
                                state.last_difficulty_nav_key = None;
                                state.last_difficulty_nav_time = None;
                                return (
                                    Some(AppState::Menu),
                                    song_or_pack_selection_changed || difficulty_selection_changed,
                                );
                            }
                            _ => {}
                        }
                    }
                }
            }
            ElementState::Released => {
                if virtual_keycode == VirtualKeyCode::Up || virtual_keycode == VirtualKeyCode::Down
                {
                    state.active_chord_keys.remove(&virtual_keycode);
                }

                match virtual_keycode {
                    VirtualKeyCode::Left | VirtualKeyCode::Right => {
                        state.nav_key_held_direction = None;
                        state.nav_key_held_since = None;
                        state.nav_key_last_scrolled_at = None;
                    }
                    _ => {}
                }
            }
        }
    }
    (
        None,
        song_or_pack_selection_changed || difficulty_selection_changed,
    )
}

pub fn update(state: &mut SelectMusicState, dt: f32, audio_manager: &AudioManager) -> bool {
    let mut selection_changed_by_update = false;
    const ANIMATION_CYCLE_DURATION: f32 = 1.0;
    state.selection_animation_timer += dt;
    if state.selection_animation_timer > ANIMATION_CYCLE_DURATION {
        state.selection_animation_timer -= ANIMATION_CYCLE_DURATION;
    }

    state.meter_arrow_animation_timer += dt;
    if state.meter_arrow_animation_timer > config::METER_ARROW_ANIM_DURATION_SEC {
        state.meter_arrow_animation_timer -= config::METER_ARROW_ANIM_DURATION_SEC;
    }

    let initial_hold_delay = Duration::from_millis(config::MUSIC_WHEEL_NAV_INITIAL_HOLD_DELAY_MS);
    let repeat_scroll_interval =
        Duration::from_millis(config::MUSIC_WHEEL_NAV_REPEAT_SCROLL_INTERVAL_MS);

    if let (Some(direction), Some(held_since), Some(last_scrolled_at)) = (
        state.nav_key_held_direction,
        state.nav_key_held_since,
        state.nav_key_last_scrolled_at,
    ) {
        let now = Instant::now();
        if now.duration_since(held_since) > initial_hold_delay {
            if now.duration_since(last_scrolled_at) >= repeat_scroll_interval {
                let num_entries = state.entries.len();
                if num_entries > 0 {
                    let old_index = state.selected_index;
                    match direction {
                        NavDirection::Up => {
                            state.selected_index = if state.selected_index == 0 {
                                num_entries - 1
                            } else {
                                state.selected_index - 1
                            };
                        }
                        NavDirection::Down => {
                            state.selected_index = (state.selected_index + 1) % num_entries;
                        }
                    }
                    if state.selected_index != old_index {
                        audio_manager.play_sfx(SoundId::MenuChange);
                        selection_changed_by_update = true;
                        state.selection_animation_timer = 0.0;
                        state.last_difficulty_nav_key = None;
                        state.last_difficulty_nav_time = None;
                    }
                    state.nav_key_last_scrolled_at = Some(now);
                }
            }
        }
    }
    selection_changed_by_update
}

pub fn draw(
    renderer: &Renderer,
    state: &SelectMusicState,
    assets: &AssetManager,
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
) {
    let header_footer_font = assets.get_font(FontId::Wendy).expect("Wendy font missing");
    let list_font = assets.get_font(FontId::Miso).expect("Miso font missing");
    let difficulty_font = assets
        .get_font(FontId::Wendy)
        .expect("Wendy font for difficulty missing");
    let miso_font_for_counts = assets
        .get_font(FontId::Miso)
        .expect("Miso font missing for counts");
    let meter_arrow_texture_opt = assets.get_texture(TextureId::MeterArrow);

    let (window_width, window_height) = renderer.window_size();
    let center_x = window_width / 2.0;

    const TARGET_BAR_TEXT_VISUAL_PX_HEIGHT_AT_REF_RES: f32 = 34.0;
    const OBSERVED_PX_HEIGHT_AT_REF_FOR_30PX_TARGET_OLD_METHOD: f32 = 19.0;
    const ASCENDER_POSITIONING_ADJUSTMENT_FACTOR: f32 = 0.65;
    const HEADER_FOOTER_LETTER_SPACING_FACTOR: f32 = 0.90;

    const PINK_BOX_REF_WIDTH: f32 = 625.0;
    const PINK_BOX_REF_HEIGHT: f32 = 90.0;
    const SMALL_UPPER_RIGHT_BOX_REF_WIDTH: f32 = 48.0;
    const SMALL_UPPER_RIGHT_BOX_REF_HEIGHT: f32 = 228.0;
    const LEFT_BOXES_REF_WIDTH: f32 = 429.0;
    const LEFT_BOX_REF_HEIGHT: f32 = 96.0;
    const STEPARTIST_INFO_BOX_REF_WIDTH: f32 = 263.0;
    const STEPARTIST_INFO_BOX_REF_HEIGHT: f32 = 26.0;
    const ARTIST_BPM_BOX_REF_WIDTH: f32 = 480.0;
    const ARTIST_BPM_BOX_REF_HEIGHT: f32 = 75.0;
    const FALLBACK_BANNER_REF_WIDTH: f32 = 480.0;
    const FALLBACK_BANNER_REF_HEIGHT: f32 = 188.0;

    const MUSIC_WHEEL_BOX_REF_WIDTH: f32 = 591.0;
    const MUSIC_WHEEL_BOX_REF_HEIGHT: f32 = 46.0;
    const NUM_MUSIC_WHEEL_BOXES: usize = 15;
    const CENTER_MUSIC_WHEEL_SLOT_INDEX: usize = 7;

    const VERTICAL_GAP_PINK_TO_UPPER_REF: f32 = 7.0;
    const HORIZONTAL_GAP_LEFT_TO_RIGHT_REF: f32 = 3.0;
    const VERTICAL_GAP_BETWEEN_LEFT_BOXES_REF: f32 = 36.0;
    const VERTICAL_GAP_GRAPH_TO_STEPARTIST_BOX_REF: f32 = 1.0;
    const VERTICAL_GAP_ARTIST_TO_BANNER_REF: f32 = 2.0;
    const MUSIC_WHEEL_VERTICAL_GAP_REF: f32 = 2.0;
    const METER_ARROW_PADDING_LEFT_REF: f32 = 0.0;

    const STEPARTIST_INFO_TEXT_TARGET_PX_HEIGHT_AT_REF_RES: f32 = 22.0;
    const STEPARTIST_BOX_HEADER_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
    const STEPARTIST_BOX_VALUE_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
    const STEPARTIST_BOX_TEXT_LEFT_PADDING_REF: f32 = 7.0;
    const STEPARTIST_BOX_HEADER_TO_VALUE_GAP_REF: f32 = 21.0;
    const STEPARTIST_BOX_TEXT_VERTICAL_NUDGE_REF: f32 = 2.0;

    let width_scale_factor = window_width / config::LAYOUT_BOXES_REF_RES_WIDTH;
    let height_scale_factor = window_height / config::LAYOUT_BOXES_REF_RES_HEIGHT;

    let bar_text_vertical_nudge_current =
        config::BAR_TEXT_VERTICAL_NUDGE_PX_AT_REF_RES * height_scale_factor;
    let music_wheel_text_vertical_nudge_current =
        config::MUSIC_WHEEL_TEXT_VERTICAL_NUDGE_PX_AT_REF_RES * height_scale_factor;

    let music_wheel_box_current_width = MUSIC_WHEEL_BOX_REF_WIDTH * width_scale_factor;
    let music_wheel_box_current_height = MUSIC_WHEEL_BOX_REF_HEIGHT * height_scale_factor;
    let music_wheel_vertical_gap_current = MUSIC_WHEEL_VERTICAL_GAP_REF * height_scale_factor;

    let bar_height =
        (window_height / config::UI_REFERENCE_HEIGHT) * config::UI_BAR_REFERENCE_HEIGHT;
    let footer_y_top_edge = window_height - bar_height;

    let pink_box_current_width = PINK_BOX_REF_WIDTH * width_scale_factor;
    let pink_box_current_height = PINK_BOX_REF_HEIGHT * height_scale_factor;
    let small_upper_right_box_current_width = SMALL_UPPER_RIGHT_BOX_REF_WIDTH * width_scale_factor;
    let small_upper_right_box_current_height =
        SMALL_UPPER_RIGHT_BOX_REF_HEIGHT * height_scale_factor;
    let left_boxes_current_width = LEFT_BOXES_REF_WIDTH * width_scale_factor;
    let left_box_current_height = LEFT_BOX_REF_HEIGHT * height_scale_factor;
    let stepartist_info_box_current_width = STEPARTIST_INFO_BOX_REF_WIDTH * width_scale_factor;
    let stepartist_info_box_current_height = STEPARTIST_INFO_BOX_REF_HEIGHT * height_scale_factor;
    let artist_bpm_box_current_width = ARTIST_BPM_BOX_REF_WIDTH * width_scale_factor;
    let artist_bpm_box_current_height = ARTIST_BPM_BOX_REF_HEIGHT * height_scale_factor;
    let fallback_banner_current_width = FALLBACK_BANNER_REF_WIDTH * width_scale_factor;
    let fallback_banner_current_height = FALLBACK_BANNER_REF_HEIGHT * height_scale_factor;

    let vertical_gap_pink_to_upper_current = VERTICAL_GAP_PINK_TO_UPPER_REF * height_scale_factor;
    let _horizontal_gap_left_to_right_current =
        HORIZONTAL_GAP_LEFT_TO_RIGHT_REF * width_scale_factor;
    let vertical_gap_between_left_boxes_current =
        VERTICAL_GAP_BETWEEN_LEFT_BOXES_REF * height_scale_factor;
    let vertical_gap_graph_to_stepartist_box_current =
        VERTICAL_GAP_GRAPH_TO_STEPARTIST_BOX_REF * height_scale_factor;
    let vertical_gap_artist_to_banner_current =
        VERTICAL_GAP_ARTIST_TO_BANNER_REF * height_scale_factor;
    let song_text_left_padding_current =
        config::MUSIC_WHEEL_SONG_TEXT_LEFT_PADDING_REF * width_scale_factor;
    let vertical_gap_stepartist_to_artist_box_current =
        config::VERTICAL_GAP_TOPMOST_TO_ARTIST_BOX_REF * height_scale_factor;
    let scaled_meter_arrow_padding_left = METER_ARROW_PADDING_LEFT_REF * width_scale_factor;

    let detail_header_text_target_current_px_height =
        config::DETAIL_HEADER_TEXT_TARGET_PX_HEIGHT_AT_REF_RES * height_scale_factor;
    let detail_value_text_target_current_px_height =
        config::DETAIL_VALUE_TEXT_TARGET_PX_HEIGHT_AT_REF_RES * height_scale_factor;
    let artist_header_left_padding_current =
        config::ARTIST_HEADER_LEFT_PADDING_REF * width_scale_factor;
    let artist_header_top_padding_current =
        config::ARTIST_HEADER_TOP_PADDING_REF * height_scale_factor;
    let bpm_header_left_padding_current = config::BPM_HEADER_LEFT_PADDING_REF * width_scale_factor;
    let header_to_value_horizontal_gap_current =
        config::HEADER_TO_VALUE_HORIZONTAL_GAP_REF * width_scale_factor;
    let bpm_to_length_horizontal_gap_current =
        config::BPM_TO_LENGTH_HORIZONTAL_GAP_REF * width_scale_factor;
    let artist_to_bpm_vertical_gap_current =
        config::ARTIST_TO_BPM_VERTICAL_GAP_REF * height_scale_factor;

    let total_music_boxes_height = NUM_MUSIC_WHEEL_BOXES as f32 * music_wheel_box_current_height;
    let total_music_gaps_height =
        (NUM_MUSIC_WHEEL_BOXES.saturating_sub(1)) as f32 * music_wheel_vertical_gap_current;
    let full_music_wheel_stack_height = total_music_boxes_height + total_music_gaps_height;
    let music_wheel_stack_top_y = (window_height - full_music_wheel_stack_height) / 2.0;

    let music_box_right_x = window_width;
    let music_box_left_x = music_box_right_x - music_wheel_box_current_width;
    let music_box_center_x = music_box_left_x + music_wheel_box_current_width / 2.0;

    let wheel_text_current_target_visual_height =
        config::MUSIC_WHEEL_TEXT_TARGET_PX_HEIGHT_AT_REF_RES * height_scale_factor;
    let wheel_font_typographic_height_normalized =
        (list_font.metrics.ascender - list_font.metrics.descender).max(1e-5);
    let wheel_text_effective_scale =
        wheel_text_current_target_visual_height / wheel_font_typographic_height_normalized;

    const ANIMATION_CYCLE_DURATION_SELECT: f32 = 1.0;
    let anim_t_unscaled =
        (state.selection_animation_timer / ANIMATION_CYCLE_DURATION_SELECT) * PI * 2.0;
    let anim_t = (anim_t_unscaled.sin() + 1.0) / 2.0;

    for i in 0..NUM_MUSIC_WHEEL_BOXES {
        let current_box_top_y = music_wheel_stack_top_y
            + (i as f32 * (music_wheel_box_current_height + music_wheel_vertical_gap_current));
        let current_box_center_y = current_box_top_y + music_wheel_box_current_height / 2.0;
        let mut display_text = "".to_string();
        let current_box_color;
        let mut current_text_color = config::SONG_TEXT_COLOR;
        let mut text_x_pos = music_box_left_x;
        let num_entries = state.entries.len();
        let is_selected_slot = i == CENTER_MUSIC_WHEEL_SLOT_INDEX;

        let mut current_entry_song_count: Option<usize> = None;

        if num_entries > 0 {
            let list_index_isize = (state.selected_index as isize + i as isize
                - CENTER_MUSIC_WHEEL_SLOT_INDEX as isize
                + num_entries as isize)
                % num_entries as isize;
            let list_index = if list_index_isize < 0 {
                (list_index_isize + num_entries as isize) as usize
            } else {
                list_index_isize as usize
            };

            if let Some(entry) = state.entries.get(list_index) {
                match entry {
                    MusicWheelEntry::Song(song_info_arc) => {
                        display_text = song_info_arc.title.clone();
                        current_box_color = if is_selected_slot {
                            lerp_color(
                                config::MUSIC_WHEEL_BOX_COLOR,
                                config::SELECTED_SONG_BOX_COLOR,
                                anim_t,
                            )
                        } else {
                            config::MUSIC_WHEEL_BOX_COLOR
                        };
                        current_text_color = config::SONG_TEXT_COLOR;
                        text_x_pos = music_box_left_x + song_text_left_padding_current;
                    }
                    MusicWheelEntry::PackHeader {
                        name: pack_name,
                        color: pack_text_color_val,
                        song_count,
                        ..
                    } => {
                        display_text = pack_name.clone();
                        current_box_color = if is_selected_slot {
                            lerp_color(
                                config::PACK_HEADER_BOX_COLOR,
                                config::SELECTED_PACK_HEADER_BOX_COLOR,
                                anim_t,
                            )
                        } else {
                            config::PACK_HEADER_BOX_COLOR
                        };
                        current_text_color = *pack_text_color_val;
                        let text_width_pixels = list_font.measure_text_normalized(&display_text)
                            * wheel_text_effective_scale;
                        text_x_pos = music_box_left_x
                            + (music_wheel_box_current_width - text_width_pixels) / 2.0;
                        current_entry_song_count = Some(*song_count);
                    }
                }
            } else {
                current_box_color = config::MUSIC_WHEEL_BOX_COLOR;
            }
        } else {
            current_box_color = config::MUSIC_WHEEL_BOX_COLOR;
        }

        renderer.draw_quad(
            device,
            cmd_buf,
            DescriptorSetId::SolidColor,
            Vector3::new(music_box_center_x, current_box_center_y, 0.0),
            (
                music_wheel_box_current_width,
                music_wheel_box_current_height,
            ),
            Rad(0.0),
            current_box_color,
            [0.0, 0.0],
            [1.0, 1.0],
        );
        if !display_text.is_empty() {
            let current_box_center_y_for_text =
                current_box_top_y + music_wheel_box_current_height / 2.0;
            let scaled_ascender = list_font.metrics.ascender * wheel_text_effective_scale;
            let scaled_descender = list_font.metrics.descender * wheel_text_effective_scale;
            let mut text_baseline_y =
                current_box_center_y_for_text + (scaled_ascender + scaled_descender) / 2.0;
            text_baseline_y += music_wheel_text_vertical_nudge_current;
            renderer.draw_text(
                device,
                cmd_buf,
                list_font,
                &display_text,
                text_x_pos,
                text_baseline_y,
                current_text_color,
                wheel_text_effective_scale,
                None,
            );

            if let Some(song_count_val) = current_entry_song_count {
                if song_count_val > 0 {
                    let count_str = format!("{}", song_count_val);

                    let target_count_text_visual_height_px = 21.0 * height_scale_factor;
                    let count_font_typographic_height_norm =
                        (miso_font_for_counts.metrics.ascender
                            - miso_font_for_counts.metrics.descender)
                            .max(1e-5);
                    let count_text_scale =
                        target_count_text_visual_height_px / count_font_typographic_height_norm;

                    let count_text_width_pixels =
                        miso_font_for_counts.measure_text_normalized(&count_str) * count_text_scale;
                    let padding_right_px = 15.0 * width_scale_factor;
                    let count_text_x_pos =
                        music_box_right_x - padding_right_px - count_text_width_pixels;

                    let count_scaled_ascender =
                        miso_font_for_counts.metrics.ascender * count_text_scale;
                    let count_scaled_descender =
                        miso_font_for_counts.metrics.descender * count_text_scale;
                    let mut count_text_baseline_y = current_box_center_y_for_text
                        + (count_scaled_ascender + count_scaled_descender) / 2.0;
                    count_text_baseline_y += music_wheel_text_vertical_nudge_current;

                    renderer.draw_text(
                        device,
                        cmd_buf,
                        miso_font_for_counts,
                        &count_str,
                        count_text_x_pos,
                        count_text_baseline_y,
                        [1.0, 1.0, 1.0, 1.0],
                        count_text_scale,
                        None,
                    );
                }
            }
        }
    }

    renderer.draw_quad(
        device,
        cmd_buf,
        DescriptorSetId::SolidColor,
        Vector3::new(center_x, bar_height / 2.0, 0.0),
        (window_width, bar_height),
        Rad(0.0),
        config::UI_BAR_COLOR,
        [0.0, 0.0],
        [1.0, 1.0],
    );
    renderer.draw_quad(
        device,
        cmd_buf,
        DescriptorSetId::SolidColor,
        Vector3::new(center_x, footer_y_top_edge + bar_height / 2.0, 0.0),
        (window_width, bar_height),
        Rad(0.0),
        config::UI_BAR_COLOR,
        [0.0, 0.0],
        [1.0, 1.0],
    );

    let large_pink_box_left_x = 0.0;
    let large_pink_box_top_y = footer_y_top_edge - pink_box_current_height;
    let large_pink_box_center_x = large_pink_box_left_x + pink_box_current_width / 2.0;
    let large_pink_box_center_y = large_pink_box_top_y + pink_box_current_height / 2.0;
    renderer.draw_quad(
        device,
        cmd_buf,
        DescriptorSetId::SolidColor,
        Vector3::new(large_pink_box_center_x, large_pink_box_center_y, 0.0),
        (pink_box_current_width, pink_box_current_height),
        Rad(0.0),
        config::PINK_BOX_COLOR,
        [0.0, 0.0],
        [1.0, 1.0],
    );

    let small_upper_right_box_top_y = large_pink_box_top_y
        - vertical_gap_pink_to_upper_current
        - small_upper_right_box_current_height;
    let small_upper_right_box_left_x =
        large_pink_box_left_x + pink_box_current_width - small_upper_right_box_current_width;
    let small_upper_right_box_center_x =
        small_upper_right_box_left_x + small_upper_right_box_current_width / 2.0;
    let small_upper_right_box_center_y =
        small_upper_right_box_top_y + small_upper_right_box_current_height / 2.0;
    renderer.draw_quad(
        device,
        cmd_buf,
        DescriptorSetId::SolidColor,
        Vector3::new(
            small_upper_right_box_center_x,
            small_upper_right_box_center_y,
            0.0,
        ),
        (
            small_upper_right_box_current_width,
            small_upper_right_box_current_height,
        ),
        Rad(0.0),
        config::UI_BOX_DARK_COLOR,
        [0.0, 0.0],
        [1.0, 1.0],
    );

    let scaled_inner_box_dim_w = config::DIFFICULTY_DISPLAY_INNER_BOX_REF_SIZE * width_scale_factor;
    let scaled_inner_box_dim_h =
        config::DIFFICULTY_DISPLAY_INNER_BOX_REF_SIZE * height_scale_factor;
    let scaled_padding_border_x =
        config::DIFFICULTY_DISPLAY_INNER_BOX_BORDER_AND_SPACING_REF * width_scale_factor;
    let scaled_padding_border_y =
        config::DIFFICULTY_DISPLAY_INNER_BOX_BORDER_AND_SPACING_REF * height_scale_factor;
    let inner_boxes_start_x = small_upper_right_box_left_x + scaled_padding_border_x;
    let inner_boxes_start_y = small_upper_right_box_top_y + scaled_padding_border_y;

    let difficulty_levels_ordered_colors = [
        config::DIFFICULTY_TEXT_COLOR_BEGINNER,
        config::DIFFICULTY_TEXT_COLOR_EASY,
        config::DIFFICULTY_TEXT_COLOR_MEDIUM,
        config::DIFFICULTY_TEXT_COLOR_HARD,
        config::DIFFICULTY_TEXT_COLOR_CHALLENGE,
    ];

    let selected_song_arc_opt = state.entries.get(state.selected_index).and_then(|entry| {
        if let MusicWheelEntry::Song(song_arc) = entry {
            Some(song_arc.clone())
        } else {
            None
        }
    });

    let meter_arrow_target_y = inner_boxes_start_y
        + state.selected_difficulty_index as f32
            * (scaled_inner_box_dim_h + scaled_padding_border_y)
        + scaled_inner_box_dim_h / 2.0;

    for (i, diff_color) in difficulty_levels_ordered_colors.iter().enumerate() {
        let current_inner_box_top_y =
            inner_boxes_start_y + i as f32 * (scaled_inner_box_dim_h + scaled_padding_border_y);
        let inner_box_center_x = inner_boxes_start_x + scaled_inner_box_dim_w / 2.0;
        let inner_box_center_y = current_inner_box_top_y + scaled_inner_box_dim_h / 2.0;

        renderer.draw_quad(
            device,
            cmd_buf,
            DescriptorSetId::SolidColor,
            Vector3::new(inner_box_center_x, inner_box_center_y, 0.0),
            (scaled_inner_box_dim_w, scaled_inner_box_dim_h),
            Rad(0.0),
            config::DIFFICULTY_DISPLAY_INNER_BOX_COLOR,
            [0.0, 0.0],
            [1.0, 1.0],
        );
        if let Some(selected_song_arc) = &selected_song_arc_opt {
            if let Some(chart_info) = selected_song_arc.charts.iter().find(|c| {
                c.difficulty.eq_ignore_ascii_case(DIFFICULTY_NAMES[i])
                    && c.stepstype == "dance-single"
            }) {
                if is_difficulty_playable(selected_song_arc, i) {
                    if !chart_info.meter.is_empty()
                        && chart_info.meter.chars().all(char::is_numeric)
                    {
                        let meter_str = &chart_info.meter;
                        let target_text_visual_height =
                            config::DIFFICULTY_METER_TEXT_VISUAL_HEIGHT_REF * height_scale_factor;
                        let font_typographic_height_norm = (difficulty_font.metrics.ascender
                            - difficulty_font.metrics.descender)
                            .max(1e-5);
                        let text_scale = target_text_visual_height / font_typographic_height_norm;
                        let text_width_pixels =
                            difficulty_font.measure_text_normalized(meter_str) * text_scale;
                        let text_draw_x = inner_boxes_start_x
                            + (scaled_inner_box_dim_w - text_width_pixels) / 2.0;
                        let text_visual_center_y =
                            current_inner_box_top_y + scaled_inner_box_dim_h / 2.0;
                        let scaled_vertical_nudge =
                            config::DIFFICULTY_METER_TEXT_VERTICAL_NUDGE_REF * height_scale_factor;
                        let text_baseline_y = text_visual_center_y
                            + (difficulty_font.metrics.ascender
                                + difficulty_font.metrics.descender)
                                / 2.0
                                * text_scale
                            + scaled_vertical_nudge;

                        renderer.draw_text(
                            device,
                            cmd_buf,
                            difficulty_font,
                            meter_str,
                            text_draw_x,
                            text_baseline_y,
                            *diff_color,
                            text_scale,
                            None,
                        );
                    }
                }
            }
        }
    }

    let bottom_left_box_top_y =
        large_pink_box_top_y - vertical_gap_pink_to_upper_current - left_box_current_height;
    let bottom_left_box_left_x = small_upper_right_box_left_x
        - HORIZONTAL_GAP_LEFT_TO_RIGHT_REF * width_scale_factor
        - left_boxes_current_width;
    let bottom_left_box_center_x = bottom_left_box_left_x + left_boxes_current_width / 2.0;
    let bottom_left_box_center_y = bottom_left_box_top_y + left_box_current_height / 2.0;
    renderer.draw_quad(
        device,
        cmd_buf,
        DescriptorSetId::SolidColor,
        Vector3::new(bottom_left_box_center_x, bottom_left_box_center_y, 0.0),
        (left_boxes_current_width, left_box_current_height),
        Rad(0.0),
        config::UI_BOX_DARK_COLOR,
        [0.0, 0.0],
        [1.0, 1.0],
    );

    let graph_area_left_x = bottom_left_box_left_x;
    let graph_area_width = left_boxes_current_width;
    let graph_area_height = left_box_current_height;
    let graph_area_top_y =
        bottom_left_box_top_y - vertical_gap_between_left_boxes_current - graph_area_height;
    let graph_area_center_x = graph_area_left_x + graph_area_width / 2.0;
    let graph_area_center_y = graph_area_top_y + graph_area_height / 2.0;
    renderer.draw_quad(
        device,
        cmd_buf,
        DescriptorSetId::SolidColor,
        Vector3::new(graph_area_center_x, graph_area_center_y, 0.0),
        (graph_area_width, graph_area_height),
        Rad(0.0),
        config::UI_BOX_DARK_COLOR,
        [0.0, 0.0],
        [1.0, 1.0],
    );
    if state.current_graph_texture.is_some() {
        renderer.draw_quad(
            device,
            cmd_buf,
            DescriptorSetId::NpsGraph,
            Vector3::new(graph_area_center_x, graph_area_center_y, 0.0),
            (graph_area_width, graph_area_height),
            Rad(0.0),
            [1.0, 1.0, 1.0, 1.0],
            [0.0, 0.0],
            [1.0, 1.0],
        );
    }

    if meter_arrow_target_y > 0.0 {
        if let Some(meter_arrow_texture) = meter_arrow_texture_opt {
            let arrow_texture_aspect =
                meter_arrow_texture.width as f32 / meter_arrow_texture.height.max(1) as f32;
            let base_arrow_draw_height = scaled_inner_box_dim_h * 0.7;
            let arrow_draw_height = base_arrow_draw_height * config::METER_ARROW_SIZE_SCALE_FACTOR;
            let arrow_draw_width = arrow_draw_height * arrow_texture_aspect;
            let base_arrow_center_x = small_upper_right_box_left_x
                - scaled_meter_arrow_padding_left
                - arrow_draw_width / 2.0;
            let anim_cycle_progress =
                state.meter_arrow_animation_timer / config::METER_ARROW_ANIM_DURATION_SEC;
            let horizontal_offset_rad = anim_cycle_progress * PI * 2.0;
            let horizontal_offset_factor = horizontal_offset_rad.sin();
            let scaled_max_horizontal_travel =
                config::METER_ARROW_ANIM_HORIZONTAL_TRAVEL_REF * width_scale_factor;
            let current_horizontal_offset = horizontal_offset_factor * scaled_max_horizontal_travel;
            let animated_arrow_center_x = base_arrow_center_x + current_horizontal_offset;
            renderer.draw_quad(
                device,
                cmd_buf,
                DescriptorSetId::MeterArrow,
                Vector3::new(animated_arrow_center_x, meter_arrow_target_y, 0.0),
                (arrow_draw_width, arrow_draw_height),
                Rad(0.0),
                [1.0, 1.0, 1.0, 1.0],
                [0.0, 0.0],
                [1.0, 1.0],
            );
        }
    }

    let stepartist_info_box_top_y = graph_area_top_y
        - vertical_gap_graph_to_stepartist_box_current
        - stepartist_info_box_current_height;
    let stepartist_info_box_left_x = graph_area_left_x;
    let stepartist_info_box_center_x =
        stepartist_info_box_left_x + stepartist_info_box_current_width / 2.0;
    let stepartist_info_box_center_y =
        stepartist_info_box_top_y + stepartist_info_box_current_height / 2.0;
    renderer.draw_quad(
        device,
        cmd_buf,
        DescriptorSetId::SolidColor,
        Vector3::new(
            stepartist_info_box_center_x,
            stepartist_info_box_center_y,
            0.0,
        ),
        (
            stepartist_info_box_current_width,
            stepartist_info_box_current_height,
        ),
        Rad(0.0),
        config::PINK_BOX_COLOR,
        [0.0, 0.0],
        [1.0, 1.0],
    );

    let miso_font = assets
        .get_font(FontId::Miso)
        .expect("Miso font missing for stepartist info box text");
    let stepartist_box_text_target_visual_height =
        STEPARTIST_INFO_TEXT_TARGET_PX_HEIGHT_AT_REF_RES * height_scale_factor;
    let stepartist_font_typographic_h_norm =
        (miso_font.metrics.ascender - miso_font.metrics.descender).max(1e-5);
    let stepartist_text_effective_scale =
        stepartist_box_text_target_visual_height / stepartist_font_typographic_h_norm;

    let scaled_miso_asc_stepartist = miso_font.metrics.ascender * stepartist_text_effective_scale;
    let current_stepartist_text_vertical_nudge =
        STEPARTIST_BOX_TEXT_VERTICAL_NUDGE_REF * height_scale_factor;
    let stepartist_visual_text_height = scaled_miso_asc_stepartist
        - (miso_font.metrics.descender * stepartist_text_effective_scale);
    let stepartist_text_padding_top_visual =
        (stepartist_info_box_current_height - stepartist_visual_text_height) / 2.0;
    let mut stepartist_baseline_y =
        stepartist_info_box_top_y + stepartist_text_padding_top_visual + scaled_miso_asc_stepartist;
    stepartist_baseline_y += current_stepartist_text_vertical_nudge;

    let stepartist_text_padding_left_current =
        STEPARTIST_BOX_TEXT_LEFT_PADDING_REF * width_scale_factor;
    let header_steps_str = "STEPS";
    let mut current_pen_x = stepartist_info_box_left_x + stepartist_text_padding_left_current;

    renderer.draw_text(
        device,
        cmd_buf,
        miso_font,
        header_steps_str,
        current_pen_x,
        stepartist_baseline_y,
        STEPARTIST_BOX_HEADER_COLOR,
        stepartist_text_effective_scale,
        None,
    );
    current_pen_x +=
        miso_font.measure_text_normalized(header_steps_str) * stepartist_text_effective_scale;
    current_pen_x += STEPARTIST_BOX_HEADER_TO_VALUE_GAP_REF * width_scale_factor;

    if let Some(selected_entry) = state.entries.get(state.selected_index) {
        if let MusicWheelEntry::Song(selected_song_arc) = selected_entry {
            let mut stepartist_display_name_for_ui = String::new();

            if state.selected_difficulty_index < DIFFICULTY_NAMES.len() {
                let target_difficulty_name = DIFFICULTY_NAMES[state.selected_difficulty_index];
                if let Some(chart) = selected_song_arc.charts.iter().find(|c| {
                    c.difficulty.eq_ignore_ascii_case(target_difficulty_name)
                        && c.stepstype == "dance-single"
                }) {
                    if !chart.stepartist_display_name.trim().is_empty() {
                        stepartist_display_name_for_ui =
                            chart.stepartist_display_name.trim().to_string();
                    }
                }
                if stepartist_display_name_for_ui.is_empty() {
                    if let Some(chart) = selected_song_arc.charts.iter().find(|c| {
                        c.stepstype == "dance-single"
                            && !c.stepartist_display_name.trim().is_empty()
                    }) {
                        stepartist_display_name_for_ui =
                            chart.stepartist_display_name.trim().to_string();
                    }
                }
            }

            if !stepartist_display_name_for_ui.is_empty() {
                renderer.draw_text(
                    device,
                    cmd_buf,
                    miso_font,
                    &stepartist_display_name_for_ui,
                    current_pen_x,
                    stepartist_baseline_y,
                    STEPARTIST_BOX_VALUE_COLOR,
                    stepartist_text_effective_scale,
                    None,
                );
            }
        }
    }

    let artist_bpm_box_left_x = stepartist_info_box_left_x;
    let artist_bpm_box_actual_top_y = stepartist_info_box_top_y
        - vertical_gap_stepartist_to_artist_box_current
        - artist_bpm_box_current_height;
    let artist_bpm_box_center_x = artist_bpm_box_left_x + artist_bpm_box_current_width / 2.0;
    let artist_bpm_box_center_y = artist_bpm_box_actual_top_y + artist_bpm_box_current_height / 2.0;
    renderer.draw_quad(
        device,
        cmd_buf,
        DescriptorSetId::SolidColor,
        Vector3::new(artist_bpm_box_center_x, artist_bpm_box_center_y, 0.0),
        (artist_bpm_box_current_width, artist_bpm_box_current_height),
        Rad(0.0),
        config::UI_BOX_DARK_COLOR,
        [0.0, 0.0],
        [1.0, 1.0],
    );

    let fallback_banner_left_x = artist_bpm_box_left_x;
    let fallback_banner_width_to_draw = fallback_banner_current_width;
    let fallback_banner_actual_top_y = artist_bpm_box_actual_top_y
        - vertical_gap_artist_to_banner_current
        - fallback_banner_current_height;
    let fallback_banner_center_x = fallback_banner_left_x + fallback_banner_width_to_draw / 2.0;
    let fallback_banner_center_y =
        fallback_banner_actual_top_y + fallback_banner_current_height / 2.0;
    renderer.draw_quad(
        device,
        cmd_buf,
        DescriptorSetId::DynamicBanner,
        Vector3::new(fallback_banner_center_x, fallback_banner_center_y, 0.0),
        (
            fallback_banner_width_to_draw,
            fallback_banner_current_height,
        ),
        Rad(0.0),
        [1.0, 1.0, 1.0, 1.0],
        [0.0, 0.0],
        [1.0, 1.0],
    );

    let hf_target_visual_current_px_height =
        TARGET_BAR_TEXT_VISUAL_PX_HEIGHT_AT_REF_RES * height_scale_factor;
    let hf_font_typographic_height_normalized =
        (header_footer_font.metrics.ascender - header_footer_font.metrics.descender).max(1e-5);
    let base_scale_for_typographic_height =
        hf_target_visual_current_px_height / hf_font_typographic_height_normalized;
    let height_adjustment_factor = if OBSERVED_PX_HEIGHT_AT_REF_FOR_30PX_TARGET_OLD_METHOD > 1e-5 {
        TARGET_BAR_TEXT_VISUAL_PX_HEIGHT_AT_REF_RES
            / OBSERVED_PX_HEIGHT_AT_REF_FOR_30PX_TARGET_OLD_METHOD
    } else {
        1.0
    };
    let hf_effective_scale = base_scale_for_typographic_height * height_adjustment_factor;
    let hf_scaled_ascender_metric = header_footer_font.metrics.ascender * hf_effective_scale;
    let hf_scaled_ascender_for_positioning =
        hf_scaled_ascender_metric * ASCENDER_POSITIONING_ADJUSTMENT_FACTOR;
    let hf_empty_vertical_space = (bar_height - hf_target_visual_current_px_height).max(0.0);
    let hf_padding_from_bar_top_to_text_visual_top = hf_empty_vertical_space / 2.0;
    let mut header_baseline_y =
        hf_padding_from_bar_top_to_text_visual_top + hf_scaled_ascender_for_positioning;
    let mut footer_baseline_y = footer_y_top_edge
        + hf_padding_from_bar_top_to_text_visual_top
        + hf_scaled_ascender_for_positioning;
    header_baseline_y += bar_text_vertical_nudge_current;
    footer_baseline_y += bar_text_vertical_nudge_current;
    let header_text_left_padding_px = 14.0 * width_scale_factor;
    let header_text_str = "SELECT MUSIC";
    renderer.draw_text(
        device,
        cmd_buf,
        header_footer_font,
        header_text_str,
        header_text_left_padding_px,
        header_baseline_y,
        config::UI_BAR_TEXT_COLOR,
        hf_effective_scale,
        Some(HEADER_FOOTER_LETTER_SPACING_FACTOR),
    );
    let footer_text_str = "EVENT MODE";
    let footer_text_glyph_width =
        header_footer_font.measure_text_normalized(footer_text_str) * hf_effective_scale;
    let num_chars = footer_text_str.chars().count();
    let footer_text_visual_width = if num_chars > 1 {
        footer_text_glyph_width
            * (1.0
                + (HEADER_FOOTER_LETTER_SPACING_FACTOR - 1.0)
                    * ((num_chars - 1) as f32 / num_chars as f32))
    } else {
        footer_text_glyph_width
    };
    renderer.draw_text(
        device,
        cmd_buf,
        header_footer_font,
        footer_text_str,
        center_x - footer_text_visual_width / 2.0,
        footer_baseline_y,
        config::UI_BAR_TEXT_COLOR,
        hf_effective_scale,
        Some(HEADER_FOOTER_LETTER_SPACING_FACTOR),
    );

    let detail_header_font_typographic_h_norm =
        (list_font.metrics.ascender - list_font.metrics.descender).max(1e-5);
    let detail_header_effective_scale =
        detail_header_text_target_current_px_height / detail_header_font_typographic_h_norm;
    let detail_value_effective_scale =
        detail_value_text_target_current_px_height / detail_header_font_typographic_h_norm;
    let first_row_visual_top_y_from_box_top = artist_header_top_padding_current;
    let advance_for_next_line = detail_value_text_target_current_px_height;

    if let Some(selected_entry) = state.entries.get(state.selected_index) {
        match selected_entry {
            MusicWheelEntry::Song(selected_song_arc) => {
                let artist_header_str = "ARTIST";
                let artist_header_width = list_font.measure_text_normalized(artist_header_str)
                    * detail_header_effective_scale;
                let artist_header_x = artist_bpm_box_left_x + artist_header_left_padding_current;
                let artist_header_baseline_y = artist_bpm_box_actual_top_y
                    + first_row_visual_top_y_from_box_top
                    + (list_font.metrics.ascender * detail_header_effective_scale)
                    + music_wheel_text_vertical_nudge_current;
                renderer.draw_text(
                    device,
                    cmd_buf,
                    list_font,
                    artist_header_str,
                    artist_header_x,
                    artist_header_baseline_y,
                    config::DETAIL_HEADER_TEXT_COLOR,
                    detail_header_effective_scale,
                    None,
                );
                let artist_value_str = &selected_song_arc.artist;
                let artist_value_x =
                    artist_header_x + artist_header_width + header_to_value_horizontal_gap_current;
                let artist_value_baseline_y = artist_bpm_box_actual_top_y
                    + first_row_visual_top_y_from_box_top
                    + (list_font.metrics.ascender * detail_value_effective_scale)
                    + music_wheel_text_vertical_nudge_current;
                renderer.draw_text(
                    device,
                    cmd_buf,
                    list_font,
                    artist_value_str,
                    artist_value_x,
                    artist_value_baseline_y,
                    config::SONG_TEXT_COLOR,
                    detail_value_effective_scale,
                    None,
                );
                let bpm_header_str = "BPM";
                let bpm_header_width = list_font.measure_text_normalized(bpm_header_str)
                    * detail_header_effective_scale;
                let bpm_header_x = artist_bpm_box_left_x + bpm_header_left_padding_current;
                let bpm_header_baseline_y = artist_value_baseline_y
                    + advance_for_next_line
                    + artist_to_bpm_vertical_gap_current;
                renderer.draw_text(
                    device,
                    cmd_buf,
                    list_font,
                    bpm_header_str,
                    bpm_header_x,
                    bpm_header_baseline_y,
                    config::DETAIL_HEADER_TEXT_COLOR,
                    detail_header_effective_scale,
                    None,
                );
                let bpm_value_str = if selected_song_arc.bpms_header.len() == 1 {
                    format!("{:.0}", selected_song_arc.bpms_header[0].1)
                } else if !selected_song_arc.bpms_header.is_empty() {
                    let min_bpm = selected_song_arc
                        .bpms_header
                        .iter()
                        .map(|&(_, bpm)| bpm)
                        .fold(f32::INFINITY, f32::min);
                    let max_bpm = selected_song_arc
                        .bpms_header
                        .iter()
                        .map(|&(_, bpm)| bpm)
                        .fold(f32::NEG_INFINITY, f32::max);
                    if (min_bpm - max_bpm).abs() < 0.1 {
                        format!("{:.0}", min_bpm)
                    } else {
                        format!("{:.0} - {:.0}", min_bpm, max_bpm)
                    }
                } else {
                    "???".to_string()
                };
                let bpm_value_x =
                    bpm_header_x + bpm_header_width + header_to_value_horizontal_gap_current;
                let bpm_value_baseline_y = bpm_header_baseline_y;
                renderer.draw_text(
                    device,
                    cmd_buf,
                    list_font,
                    &bpm_value_str,
                    bpm_value_x,
                    bpm_value_baseline_y,
                    config::SONG_TEXT_COLOR,
                    detail_value_effective_scale,
                    None,
                );
                let length_header_str = "LENGTH";
                let length_value_str = selected_song_arc
                    .charts
                    .iter()
                    .find_map(|c| c.calculated_length_sec)
                    .map_or_else(
                        || "??:??".to_string(),
                        |secs| format_duration_flexible(secs),
                    );
                let length_header_width = list_font.measure_text_normalized(length_header_str)
                    * detail_header_effective_scale;
                let length_header_x =
                    bpm_header_x + bpm_header_width + bpm_to_length_horizontal_gap_current;
                let length_value_x =
                    length_header_x + length_header_width + header_to_value_horizontal_gap_current;
                let length_header_baseline_y = bpm_header_baseline_y;
                let length_value_baseline_y = bpm_value_baseline_y;
                renderer.draw_text(
                    device,
                    cmd_buf,
                    list_font,
                    length_header_str,
                    length_header_x,
                    length_header_baseline_y,
                    config::DETAIL_HEADER_TEXT_COLOR,
                    detail_header_effective_scale,
                    None,
                );
                renderer.draw_text(
                    device,
                    cmd_buf,
                    list_font,
                    &length_value_str,
                    length_value_x,
                    length_value_baseline_y,
                    config::SONG_TEXT_COLOR,
                    detail_value_effective_scale,
                    None,
                );
            }
            MusicWheelEntry::PackHeader {
                total_duration_sec, ..
            } => {
                let artist_header_str = "ARTIST";
                let artist_header_x = artist_bpm_box_left_x + artist_header_left_padding_current;
                let artist_header_baseline_y = artist_bpm_box_actual_top_y
                    + first_row_visual_top_y_from_box_top
                    + (list_font.metrics.ascender * detail_header_effective_scale)
                    + music_wheel_text_vertical_nudge_current;
                renderer.draw_text(
                    device,
                    cmd_buf,
                    list_font,
                    artist_header_str,
                    artist_header_x,
                    artist_header_baseline_y,
                    config::DETAIL_HEADER_TEXT_COLOR,
                    detail_header_effective_scale,
                    None,
                );
                let bpm_header_str = "BPM";
                let bpm_header_width = list_font.measure_text_normalized(bpm_header_str)
                    * detail_header_effective_scale;
                let bpm_header_x = artist_bpm_box_left_x + bpm_header_left_padding_current;
                let bpm_header_baseline_y = artist_header_baseline_y
                    + advance_for_next_line
                    + artist_to_bpm_vertical_gap_current;
                renderer.draw_text(
                    device,
                    cmd_buf,
                    list_font,
                    bpm_header_str,
                    bpm_header_x,
                    bpm_header_baseline_y,
                    config::DETAIL_HEADER_TEXT_COLOR,
                    detail_header_effective_scale,
                    None,
                );
                let length_header_str = "LENGTH";
                let length_value_str = total_duration_sec.map_or_else(
                    || "??:??".to_string(),
                    |secs| format_duration_flexible(secs),
                );
                let length_header_width = list_font.measure_text_normalized(length_header_str)
                    * detail_header_effective_scale;
                let length_header_x =
                    bpm_header_x + bpm_header_width + bpm_to_length_horizontal_gap_current;
                let length_value_x =
                    length_header_x + length_header_width + header_to_value_horizontal_gap_current;
                let length_header_baseline_y = bpm_header_baseline_y;
                let length_value_baseline_y = bpm_header_baseline_y;
                renderer.draw_text(
                    device,
                    cmd_buf,
                    list_font,
                    length_header_str,
                    length_header_x,
                    length_header_baseline_y,
                    config::DETAIL_HEADER_TEXT_COLOR,
                    detail_header_effective_scale,
                    None,
                );
                renderer.draw_text(
                    device,
                    cmd_buf,
                    list_font,
                    &length_value_str,
                    length_value_x,
                    length_value_baseline_y,
                    config::SONG_TEXT_COLOR,
                    detail_value_effective_scale,
                    None,
                );
            }
        }
    }
}
