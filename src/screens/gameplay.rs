use crate::assets::{AssetManager, TextureId};
use crate::config;
use crate::graphics::renderer::{DescriptorSetId, Renderer};
use crate::state::{
    AppState, Arrow, ArrowDirection, FlashState, GameState, Judgment, NoteType, TargetInfo,
    VirtualKeyCode, ALL_ARROW_DIRECTIONS,
};
use cgmath::{Rad, Vector3};
use ash::vk;
use log::{debug, info, trace, warn};
use rand::{distr::Bernoulli, prelude::{Distribution, Rng}};
use rand::prelude::IndexedRandom;
use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;
use std::time::{Instant};
use winit::event::{ElementState, KeyEvent};

// --- Initialization ---
pub fn initialize_game_state(
    win_w: f32,
    win_h: f32,
    audio_start_time: Instant, 
) -> GameState {
    info!(
        "Initializing game state for window size: {}x{} (Y-UP projection)", // Clarify projection
        win_w, win_h
    );
    let center_x = win_w / 2.0;
    let target_spacing = config::TARGET_SIZE * 1.2; 
    let total_targets_width = (ALL_ARROW_DIRECTIONS.len() as f32 - 1.0) * target_spacing;
    let start_x_targets = center_x - total_targets_width / 2.0;

    // TARGET_Y_POS is from the bottom of the screen in Y-UP
    let targets = ALL_ARROW_DIRECTIONS
        .iter()
        .enumerate()
        .map(|(i, &dir)| TargetInfo {
            x: start_x_targets + i as f32 * target_spacing,
            y: config::TARGET_Y_POS, 
            direction: dir,
        })
        .collect();

    let mut arrows = HashMap::new();
    for dir in ALL_ARROW_DIRECTIONS.iter() {
        arrows.insert(*dir, Vec::new());
    }

    let seconds_per_beat = 60.0 / config::SONG_BPM;
    let beat_offset = (config::AUDIO_SYNC_OFFSET_MS as f32 / 1000.0) / seconds_per_beat;
    let initial_beat = -beat_offset; 
    info!(
        "Audio Sync Offset: {} ms -> Beat Offset: {:.4} -> Initial Beat: {:.4}",
        config::AUDIO_SYNC_OFFSET_MS, beat_offset, initial_beat
    );

    let first_potential_spawn_beat = initial_beat + config::SPAWN_LOOKAHEAD_BEATS;
    let initial_last_spawned_16th_index = (first_potential_spawn_beat * 4.0 - 1.0).floor() as i32;

    info!(
        "Initial last spawned 16th index: {}",
        initial_last_spawned_16th_index
    );

    GameState {
        targets,
        arrows,
        pressed_keys: HashSet::new(),
        last_spawned_16th_index: initial_last_spawned_16th_index,
        last_spawned_direction: None, 
        current_beat: initial_beat,
        window_size: (win_w, win_h),
        flash_states: HashMap::new(), 
        audio_start_time: Some(audio_start_time), 
    }
}

// --- Input Handling --- (No changes from your current version)
pub fn handle_input(
    key_event: &KeyEvent,
    game_state: &mut GameState,
) -> Option<AppState> {
    if key_event.state == ElementState::Pressed && !key_event.repeat {
        if let Some(VirtualKeyCode::Escape) = crate::state::key_to_virtual_keycode(key_event.logical_key.clone()) {
           info!("Escape pressed in gameplay, returning to Select Music.");
           return Some(AppState::SelectMusic); 
       }
   }

     if let Some(virtual_keycode) = crate::state::key_to_virtual_keycode(key_event.logical_key.clone()) {
        match virtual_keycode {
            VirtualKeyCode::Left | VirtualKeyCode::Down | VirtualKeyCode::Up | VirtualKeyCode::Right => {
                 match key_event.state {
                    ElementState::Pressed => {
                        if game_state.pressed_keys.insert(virtual_keycode) && !key_event.repeat {
                            trace!("Gameplay Key Pressed: {:?}", virtual_keycode);
                            check_hits_on_press(game_state, virtual_keycode);
                        }
                    }
                    ElementState::Released => {
                        if game_state.pressed_keys.remove(&virtual_keycode) {
                            trace!("Gameplay Key Released: {:?}", virtual_keycode);
                        }
                    }
                }
            },
            _ => {} 
        }
     }
    None 
}


// --- Update Logic ---
pub fn update(game_state: &mut GameState, dt: f32, rng: &mut impl Rng) {
    if let Some(start_time) = game_state.audio_start_time {
        let elapsed_since_audio_start = Instant::now().duration_since(start_time).as_secs_f32();
        let seconds_per_beat = 60.0 / config::SONG_BPM;
        let beat_offset = (config::AUDIO_SYNC_OFFSET_MS as f32 / 1000.0) / seconds_per_beat;
        game_state.current_beat = elapsed_since_audio_start / seconds_per_beat - beat_offset;
         trace!("Current Beat: {:.4}", game_state.current_beat);
    } else {
        warn!("Audio start time not set, cannot update beat accurately!");
    }

    spawn_arrows(game_state, rng);

    let arrow_delta_y = config::ARROW_SPEED * dt;
    for column_arrows in game_state.arrows.values_mut() {
        for arrow in column_arrows.iter_mut() {
            // CORRECTED FOR Y-UP: Arrows move DOWN by DECREASING Y
            arrow.y -= arrow_delta_y; 
        }
    }

    check_misses(game_state);

    let now = Instant::now();
    game_state.flash_states.retain(|_dir, flash| now < flash.end_time);
}

// --- Arrow Spawning Logic ---
fn spawn_arrows(state: &mut GameState, rng: &mut impl Rng) {
    let seconds_per_beat = 60.0 / config::SONG_BPM;
    let lookahead_target_beat = state.current_beat + config::SPAWN_LOOKAHEAD_BEATS;
    let target_16th_index = (lookahead_target_beat * 4.0).floor() as i32;

    if target_16th_index > state.last_spawned_16th_index {
        let bernoulli_half = Bernoulli::new(0.5).unwrap(); 

        for i in (state.last_spawned_16th_index + 1)..=target_16th_index {
            let target_beat = i as f32 / 4.0; 

            let note_type = match i % 4 {
                0 => NoteType::Quarter,   
                2 => NoteType::Eighth,    
                1 | 3 => NoteType::Sixteenth, 
                _ => unreachable!(),      
            };

            let should_spawn = match config::DIFFICULTY {
                0 => note_type == NoteType::Quarter, 
                1 => note_type == NoteType::Quarter || (note_type == NoteType::Eighth && bernoulli_half.sample(rng)),
                2 => note_type == NoteType::Quarter || note_type == NoteType::Eighth, 
                3 | 4 => true, 
                _ => true, 
            };

            if !should_spawn {
                trace!("Skipping spawn for index {} (type {:?}) due to difficulty {}", i, note_type, config::DIFFICULTY);
                continue; 
            }

            let beats_until_target = target_beat - state.current_beat;
            if beats_until_target <= 0.0 {
                trace!("Skipping spawn for past beat {:.2} (current: {:.2})", target_beat, state.current_beat);
                continue;
            }

            let time_to_target_s = beats_until_target * seconds_per_beat;
            let distance_to_travel = config::ARROW_SPEED * time_to_target_s;
            
            // CORRECTED FOR Y-UP: Spawn ABOVE target (higher Y value)
            // TARGET_Y_POS is distance from bottom. distance_to_travel is how far above that.
            let spawn_y = config::TARGET_Y_POS + distance_to_travel; 

            // Culling conditions for Y-UP:
            // spawn_y <= TARGET_Y_POS + (ARROW_SIZE * 0.1) means too close to target (or below it if distance_to_travel is small)
            if spawn_y <= config::TARGET_Y_POS + (config::ARROW_SIZE * 0.1) { 
                trace!("Skipping spawn for arrow too close to target (spawn_y: {:.1}, target_y: {:.1}) for beat {:.2}", spawn_y, config::TARGET_Y_POS, target_beat);
                continue;
            }
            // spawn_y > window_height + ARROW_SIZE * 2.0 means too far above screen top
            if spawn_y > state.window_size.1 + config::ARROW_SIZE * 2.0 { 
               trace!("Skipping spawn for arrow too far off-screen (spawn_y: {:.1}, window_height: {:.1}) for beat {:.2}", spawn_y, state.window_size.1, target_beat);
               continue;
            }

            let dir: ArrowDirection = if config::DIFFICULTY >= 4 && state.last_spawned_direction.is_some() {
                let mut available_dirs: Vec<ArrowDirection> = ALL_ARROW_DIRECTIONS
                    .iter()
                    .copied()
                    .filter(|&d| Some(d) != state.last_spawned_direction)
                    .collect();
                if available_dirs.is_empty() {
                    available_dirs = ALL_ARROW_DIRECTIONS.to_vec();
                }
                *available_dirs.as_slice().choose(rng).unwrap_or(&ALL_ARROW_DIRECTIONS[0])
            } else {
                *ALL_ARROW_DIRECTIONS.choose(rng).unwrap()
            };

            let target_x = state
                .targets
                .iter()
                .find(|t| t.direction == dir)
                .map(|t| t.x)
                .unwrap_or(state.window_size.0 / 2.0); 

            if let Some(column_arrows) = state.arrows.get_mut(&dir) {
                column_arrows.push(Arrow {
                    x: target_x, 
                    y: spawn_y,
                    direction: dir,
                    note_type,
                    target_beat,
                });
                 trace!(
                    "Spawned {:?} {:?} at y={:.1} (target_y={:.1}), target_beat={:.2} (current_beat={:.2})",
                    dir, note_type, spawn_y, config::TARGET_Y_POS, target_beat, state.current_beat
                );
                if config::DIFFICULTY >= 4 {
                    state.last_spawned_direction = Some(dir); 
                }
            }
        }
        state.last_spawned_16th_index = target_16th_index;
    }
}

// --- Hit Checking Logic --- (No changes from your current version)
fn check_hits_on_press(state: &mut GameState, keycode: VirtualKeyCode) {
    let direction = match keycode {
        VirtualKeyCode::Left => Some(ArrowDirection::Left),
        VirtualKeyCode::Down => Some(ArrowDirection::Down),
        VirtualKeyCode::Up => Some(ArrowDirection::Up),
        VirtualKeyCode::Right => Some(ArrowDirection::Right),
        _ => None, 
    };

    if let Some(dir) = direction {
        if let Some(column_arrows) = state.arrows.get_mut(&dir) {
            let current_beat = state.current_beat;
            let seconds_per_beat = 60.0 / config::SONG_BPM;

            let mut best_hit_idx: Option<usize> = None;
            let mut min_abs_time_diff_ms = config::MAX_HIT_WINDOW_MS + 1.0; 

            for (idx, arrow) in column_arrows.iter().enumerate() {
                let beat_diff = current_beat - arrow.target_beat;
                let time_diff_ms = beat_diff * seconds_per_beat * 1000.0;
                let abs_time_diff_ms = time_diff_ms.abs();

                if abs_time_diff_ms <= config::MAX_HIT_WINDOW_MS && abs_time_diff_ms < min_abs_time_diff_ms {
                    min_abs_time_diff_ms = abs_time_diff_ms;
                    best_hit_idx = Some(idx);
                }
            }

            if let Some(idx) = best_hit_idx {
                let hit_arrow = &column_arrows[idx]; 
                 let time_diff_for_log =
                     (current_beat - hit_arrow.target_beat) * seconds_per_beat * 1000.0;
                 let note_type_for_log = hit_arrow.note_type; 

                let judgment = if min_abs_time_diff_ms <= config::W1_WINDOW_MS { Judgment::W1 }
                    else if min_abs_time_diff_ms <= config::W2_WINDOW_MS { Judgment::W2 }
                    else if min_abs_time_diff_ms <= config::W3_WINDOW_MS { Judgment::W3 }
                    else if min_abs_time_diff_ms <= config::W4_WINDOW_MS { Judgment::W4 }
                    else { Judgment::W4 }; 

                info!(
                    "HIT! {:?} {:?} ({:.1}ms) -> {:?}",
                    dir, note_type_for_log, time_diff_for_log, judgment
                );

                let flash_color = match judgment {
                    Judgment::W1 => config::FLASH_COLOR_W1,
                    Judgment::W2 => config::FLASH_COLOR_W2,
                    Judgment::W3 => config::FLASH_COLOR_W3,
                    Judgment::W4 => config::FLASH_COLOR_W4,
                    Judgment::Miss => unreachable!(), 
                };
                let flash_end_time = Instant::now() + config::FLASH_DURATION;
                state.flash_states.insert(
                    dir,
                    FlashState { color: flash_color, end_time: flash_end_time },
                );
                column_arrows.remove(idx);
            } else {
                 debug!(
                    "Input {:?} registered, but no arrow within {:.1}ms hit window (Beat: {:.2}).",
                    keycode, config::MAX_HIT_WINDOW_MS, current_beat
                );
            }
        }
    }
}

// --- Miss Checking Logic --- (No changes from your current version)
fn check_misses(state: &mut GameState) {
    let seconds_per_beat = 60.0 / config::SONG_BPM;
    let miss_window_beats = (config::MISS_WINDOW_MS / 1000.0) / seconds_per_beat;
    let current_beat = state.current_beat;
    let mut missed_count = 0;

    for (_dir, column_arrows) in state.arrows.iter_mut() {
        column_arrows.retain(|arrow| {
            let beat_diff = current_beat - arrow.target_beat;
            if beat_diff > miss_window_beats {
                 info!(
                    "MISSED! {:?} {:?} (Tgt: {:.2}, Now: {:.2}, Diff: {:.1}ms > {:.1}ms)",
                    arrow.direction, arrow.note_type, arrow.target_beat, current_beat,
                    beat_diff * seconds_per_beat * 1000.0, config::MISS_WINDOW_MS
                );
                 missed_count += 1;
                false 
            } else {
                true 
            }
        });
    }
     if missed_count > 0 {
          trace!("Removed {} missed arrows.", missed_count);
     }
}

// --- Drawing Logic ---
pub fn draw(
    renderer: &Renderer,
    game_state: &GameState,
    assets: &AssetManager,
    device: &ash::Device,
    cmd_buf: vk::CommandBuffer,
) {
    let _arrow_texture = assets.get_texture(TextureId::Arrows).expect("Arrow texture missing");
    let now = Instant::now(); 

    let frame_index = ((game_state.current_beat * 2.0).floor() as usize) % 4;
    let uv_width = 1.0 / 4.0; 
    let uv_x_start = frame_index as f32 * uv_width;
    let base_uv_offset = [uv_x_start, 0.0]; 
    let base_uv_scale = [uv_width, 1.0];    

    // --- Draw Targets ---
    for target in &game_state.targets {
        let current_tint = game_state.flash_states
            .get(&target.direction)
            .filter(|flash| now < flash.end_time) 
            .map_or(config::TARGET_TINT, |flash| flash.color); 

        let rotation_angle = match target.direction {
            ArrowDirection::Left => Rad(PI / 2.0),
            ArrowDirection::Down => Rad(0.0),
            ArrowDirection::Up => Rad(PI),
            ArrowDirection::Right => Rad(-PI / 2.0), 
        };

        renderer.draw_quad(
            device, cmd_buf, DescriptorSetId::Gameplay, 
            Vector3::new(target.x, target.y, 0.0),
            (config::TARGET_SIZE, config::TARGET_SIZE),
            rotation_angle,
            current_tint,
            base_uv_offset, 
            base_uv_scale,
        );
    }

    // --- Draw Arrows ---
    for column_arrows in game_state.arrows.values() {
        for arrow in column_arrows {
            // Culling for Y-UP:
            // arrow.y < -config::ARROW_SIZE (too far below screen bottom, Y=0)
            // arrow.y > game_state.window_size.1 + config::ARROW_SIZE (too far above screen top, Y=window_height)
            if arrow.y < (0.0 - config::ARROW_SIZE) || arrow.y > (game_state.window_size.1 + config::ARROW_SIZE) {
                continue;
            }

            let arrow_tint = match arrow.note_type {
                NoteType::Quarter => config::ARROW_TINT_QUARTER,
                NoteType::Eighth => config::ARROW_TINT_EIGHTH,
                NoteType::Sixteenth => config::ARROW_TINT_SIXTEENTH,
            };

            let rotation_angle = match arrow.direction {
                ArrowDirection::Left => Rad(PI / 2.0),
                ArrowDirection::Down => Rad(0.0),
                ArrowDirection::Up => Rad(PI),
                ArrowDirection::Right => Rad(-PI / 2.0),
            };

            renderer.draw_quad(
                device, cmd_buf, DescriptorSetId::Gameplay, 
                Vector3::new(arrow.x, arrow.y, 0.0),
                (config::ARROW_SIZE, config::ARROW_SIZE),
                rotation_angle,
                arrow_tint,
                base_uv_offset, 
                base_uv_scale,
            );
        }
    }
}