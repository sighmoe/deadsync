use crate::assets::{AssetManager, TextureId}; // Assuming sound IDs for hits exist
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
    audio_start_time: Instant, // Require audio start time for beat sync
) -> GameState {
    info!(
        "Initializing game state for window size: {}x{}",
        win_w, win_h
    );
    let center_x = win_w / 2.0;
    // Calculate horizontal positions for targets
    let target_spacing = config::TARGET_SIZE * 1.2; // Spacing between center of targets
    let total_targets_width = (ALL_ARROW_DIRECTIONS.len() as f32 - 1.0) * target_spacing;
    let start_x_targets = center_x - total_targets_width / 2.0;

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
        arrows.insert(*dir, Vec::new()); // Initialize empty vector for each direction
    }

    // Calculate initial beat based on audio sync offset
    let seconds_per_beat = 60.0 / config::SONG_BPM;
    let beat_offset = (config::AUDIO_SYNC_OFFSET_MS as f32 / 1000.0) / seconds_per_beat;
    let initial_beat = -beat_offset; // Start slightly before beat 0 based on offset
    info!(
        "Audio Sync Offset: {} ms -> Beat Offset: {:.4} -> Initial Beat: {:.4}",
        config::AUDIO_SYNC_OFFSET_MS, beat_offset, initial_beat
    );

    // Calculate the index of the 16th note *before* the initial beat's lookahead starts
    // So if initial beat is -0.1 and lookahead is 4, we want spawns starting around beat 3.9
    // last_spawned should be the index *right before* the first one we might spawn.
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
        last_spawned_direction: None, // For anti-repeat logic on higher difficulties
        current_beat: initial_beat,
        window_size: (win_w, win_h),
        flash_states: HashMap::new(), // No flashes initially
        audio_start_time: Some(audio_start_time), // Store the start time
    }
}

// --- Input Handling ---
pub fn handle_input(
    key_event: &KeyEvent,
    game_state: &mut GameState,
) -> Option<AppState> {
    if key_event.state == ElementState::Pressed && !key_event.repeat {
        if let Some(VirtualKeyCode::Escape) = crate::state::key_to_virtual_keycode(key_event.logical_key.clone()) {
            info!("Escape pressed in gameplay, returning to menu.");
            return Some(AppState::Menu); // Request state transition
        }
    }

    // Handle arrow key presses/releases for gameplay
     if let Some(virtual_keycode) = crate::state::key_to_virtual_keycode(key_event.logical_key.clone()) {
        match virtual_keycode {
            VirtualKeyCode::Left | VirtualKeyCode::Down | VirtualKeyCode::Up | VirtualKeyCode::Right => {
                 match key_event.state {
                    ElementState::Pressed => {
                        // Only process the first press, not repeats handled by OS
                        if game_state.pressed_keys.insert(virtual_keycode) && !key_event.repeat {
                            trace!("Gameplay Key Pressed: {:?}", virtual_keycode);
                            check_hits_on_press(game_state, virtual_keycode);
                        }
                    }
                    ElementState::Released => {
                        if game_state.pressed_keys.remove(&virtual_keycode) {
                            trace!("Gameplay Key Released: {:?}", virtual_keycode);
                            // Optionally handle release events if needed (e.g., for holds)
                        }
                    }
                }
            },
            _ => {} // Ignore Enter, Escape (handled above), etc. for hit checking
        }
     }


    None // No state change requested by default
}

// --- Update Logic ---
pub fn update(game_state: &mut GameState, dt: f32, rng: &mut impl Rng) {
    // 1. Update Current Beat based on precise audio time
    if let Some(start_time) = game_state.audio_start_time {
        let elapsed_since_audio_start = Instant::now().duration_since(start_time).as_secs_f32();
        let seconds_per_beat = 60.0 / config::SONG_BPM;
        let beat_offset = (config::AUDIO_SYNC_OFFSET_MS as f32 / 1000.0) / seconds_per_beat;
        game_state.current_beat = elapsed_since_audio_start / seconds_per_beat - beat_offset;
         trace!("Current Beat: {:.4}", game_state.current_beat);
    } else {
        // Fallback if audio isn't running (e.g., during init error)
        warn!("Audio start time not set, cannot update beat accurately!");
        // Avoid updating beat based on dt as it will drift significantly
        // game_state.current_beat += dt * (config::SONG_BPM / 60.0);
    }

    // 2. Spawn New Arrows
    spawn_arrows(game_state, rng);

    // 3. Update Arrow Positions
    let arrow_delta_y = config::ARROW_SPEED * dt;
    for column_arrows in game_state.arrows.values_mut() {
        for arrow in column_arrows.iter_mut() {
            arrow.y -= arrow_delta_y; // Move arrows down (negative Y direction)
        }
    }

    // 4. Check for Missed Arrows
    check_misses(game_state);

    // 5. Update Flash States (remove expired ones)
    let now = Instant::now();
    game_state.flash_states.retain(|_dir, flash| now < flash.end_time);
}

// --- Arrow Spawning Logic ---
fn spawn_arrows(state: &mut GameState, rng: &mut impl Rng) {
    let seconds_per_beat = 60.0 / config::SONG_BPM;
    // Determine the latest 16th note index we should have *considered* spawning by now,
    // based on the lookahead window.
    let lookahead_target_beat = state.current_beat + config::SPAWN_LOOKAHEAD_BEATS;
    let target_16th_index = (lookahead_target_beat * 4.0).floor() as i32;

    // If the target index is ahead of the last one we processed, spawn intervening notes.
    if target_16th_index > state.last_spawned_16th_index {
        // Probability distributions for random difficulties
        let bernoulli_half = Bernoulli::new(0.5).unwrap(); // 50% chance

        // Iterate through each 16th note index from the last spawned up to the target
        for i in (state.last_spawned_16th_index + 1)..=target_16th_index {
            let target_beat = i as f32 / 4.0; // Beat time for this 16th note

            // Determine the type of note based on the 16th index
            let note_type = match i % 4 {
                0 => NoteType::Quarter,   // On the beat
                2 => NoteType::Eighth,    // Off the beat (8th note)
                1 | 3 => NoteType::Sixteenth, // Off the beat (16th notes)
                _ => unreachable!(),      // Should not happen
            };

            // Decide whether to spawn based on difficulty level
            let should_spawn = match config::DIFFICULTY {
                0 => note_type == NoteType::Quarter, // Easy: Quarters only
                1 => { // Medium: Quarters + 50% chance of Eighths
                    note_type == NoteType::Quarter
                        || (note_type == NoteType::Eighth && bernoulli_half.sample(rng))
                }
                2 => note_type == NoteType::Quarter || note_type == NoteType::Eighth, // Hard: Quarters and Eighths
                3 | 4 => true, // Expert/Challenge: All notes (Q, E, S)
                _ => true, // Default to all notes for undefined difficulties
            };

            if !should_spawn {
                 trace!("Skipping spawn for index {} (type {:?}) due to difficulty {}", i, note_type, config::DIFFICULTY);
                continue; // Skip spawning this note
            }

            // Calculate spawn position
            let beats_until_target = target_beat - state.current_beat;
            // Skip if the note is already past (shouldn't happen with lookahead, but safety check)
            if beats_until_target <= 0.0 {
                 trace!("Skipping spawn for past beat {:.2} (current: {:.2})", target_beat, state.current_beat);
                continue;
            }

            let time_to_target_s = beats_until_target * seconds_per_beat;
            let distance_to_travel = config::ARROW_SPEED * time_to_target_s;
            let spawn_y = config::TARGET_Y_POS + distance_to_travel; // Spawn above target

            // Optional: Skip spawning if it's too close (e.g., if lookahead is very small or FPS drops)
             if spawn_y <= config::TARGET_Y_POS + (config::ARROW_SIZE * 0.1) {
                  trace!("Skipping spawn for arrow too close to target (y: {:.1}) for beat {:.2}", spawn_y, target_beat);
                  continue;
             }
              // Optional: Skip spawning if it's way off-screen (performance)
              if spawn_y > state.window_size.1 + config::ARROW_SIZE * 2.0 {
                 trace!("Skipping spawn for arrow too far off-screen (y: {:.1}) for beat {:.2}", spawn_y, target_beat);
                 continue;
              }


            // Choose arrow direction
            let dir: ArrowDirection = if config::DIFFICULTY >= 4 && state.last_spawned_direction.is_some() {
                // Difficulty 4+: Try not to repeat the last direction
                let mut available_dirs: Vec<ArrowDirection> = ALL_ARROW_DIRECTIONS
                    .iter()
                    .copied()
                    .filter(|&d| Some(d) != state.last_spawned_direction)
                    .collect();
                // If only one direction was left, allow repeating it
                if available_dirs.is_empty() {
                    available_dirs = ALL_ARROW_DIRECTIONS.to_vec();
                }
                *available_dirs.as_slice().choose(rng).unwrap_or(&ALL_ARROW_DIRECTIONS[0]) // Choose randomly from available slice
            } else {
                // Random direction for lower difficulties or if no previous arrow
                *ALL_ARROW_DIRECTIONS.choose(rng).unwrap() // choose works directly on slices/arrays
            };

            // Get the target X position for the chosen direction
            let target_x = state
                .targets
                .iter()
                .find(|t| t.direction == dir)
                .map(|t| t.x)
                .unwrap_or(state.window_size.0 / 2.0); // Fallback to center X if target not found

            // Add the new arrow to the corresponding column
            if let Some(column_arrows) = state.arrows.get_mut(&dir) {
                column_arrows.push(Arrow {
                    x: target_x, // Spawn at the target's X
                    y: spawn_y,
                    direction: dir,
                    note_type,
                    target_beat,
                });
                 trace!(
                    "Spawned {:?} {:?} at y={:.1}, target_beat={:.2} (current_beat={:.2})",
                    dir, note_type, spawn_y, target_beat, state.current_beat
                );
                if config::DIFFICULTY >= 4 {
                    state.last_spawned_direction = Some(dir); // Update last spawned for anti-repeat
                }
            }
        }
        // Update the last processed index
        state.last_spawned_16th_index = target_16th_index;
    }
}

// --- Hit Checking Logic ---
fn check_hits_on_press(state: &mut GameState, keycode: VirtualKeyCode) {
    let direction = match keycode {
        VirtualKeyCode::Left => Some(ArrowDirection::Left),
        VirtualKeyCode::Down => Some(ArrowDirection::Down),
        VirtualKeyCode::Up => Some(ArrowDirection::Up),
        VirtualKeyCode::Right => Some(ArrowDirection::Right),
        _ => None, // Ignore other keys
    };

    if let Some(dir) = direction {
        if let Some(column_arrows) = state.arrows.get_mut(&dir) {
            let current_beat = state.current_beat;
            let seconds_per_beat = 60.0 / config::SONG_BPM;

            let mut best_hit_idx: Option<usize> = None;
            let mut min_abs_time_diff_ms = config::MAX_HIT_WINDOW_MS + 1.0; // Start outside window

            // Find the closest arrow within the maximum hit window
            for (idx, arrow) in column_arrows.iter().enumerate() {
                let beat_diff = current_beat - arrow.target_beat;
                let time_diff_ms = beat_diff * seconds_per_beat * 1000.0;
                let abs_time_diff_ms = time_diff_ms.abs();

                // Check if this arrow is within the widest possible hit window
                // AND is closer than the current best hit found so far
                if abs_time_diff_ms <= config::MAX_HIT_WINDOW_MS && abs_time_diff_ms < min_abs_time_diff_ms {
                    min_abs_time_diff_ms = abs_time_diff_ms;
                    best_hit_idx = Some(idx);
                }
            }

            // If an arrow within the window was found
            if let Some(idx) = best_hit_idx {
                let hit_arrow = &column_arrows[idx]; // Get immutable borrow first
                 let time_diff_for_log =
                     (current_beat - hit_arrow.target_beat) * seconds_per_beat * 1000.0;
                 let note_type_for_log = hit_arrow.note_type; // Copy data needed after remove

                // Determine judgment based on timing window
                let judgment = if min_abs_time_diff_ms <= config::W1_WINDOW_MS { Judgment::W1 }
                    else if min_abs_time_diff_ms <= config::W2_WINDOW_MS { Judgment::W2 }
                    else if min_abs_time_diff_ms <= config::W3_WINDOW_MS { Judgment::W3 }
                    else if min_abs_time_diff_ms <= config::W4_WINDOW_MS { Judgment::W4 }
                    else { Judgment::W4 }; // Must be W4 if within MAX_HIT_WINDOW_MS

                info!(
                    "HIT! {:?} {:?} ({:.1}ms) -> {:?}",
                    dir, note_type_for_log, time_diff_for_log, judgment
                );

                // Trigger target flash effect
                let flash_color = match judgment {
                    Judgment::W1 => config::FLASH_COLOR_W1,
                    Judgment::W2 => config::FLASH_COLOR_W2,
                    Judgment::W3 => config::FLASH_COLOR_W3,
                    Judgment::W4 => config::FLASH_COLOR_W4,
                    Judgment::Miss => unreachable!(), // Should not happen on a hit
                };
                let flash_end_time = Instant::now() + config::FLASH_DURATION;
                state.flash_states.insert(
                    dir,
                    FlashState { color: flash_color, end_time: flash_end_time },
                );

                // Remove the hit arrow
                column_arrows.remove(idx);

            } else {
                 // No arrow was found within the MAX_HIT_WINDOW_MS for this key press
                 debug!(
                    "Input {:?} registered, but no arrow within {:.1}ms hit window (Beat: {:.2}).",
                    keycode, config::MAX_HIT_WINDOW_MS, current_beat
                );
                 // Optional: Add a "bad press" sound or visual feedback?
            }
        }
    }
}

// --- Miss Checking Logic ---
fn check_misses(state: &mut GameState) {
    let seconds_per_beat = 60.0 / config::SONG_BPM;
    let miss_window_beats = (config::MISS_WINDOW_MS / 1000.0) / seconds_per_beat;
    let current_beat = state.current_beat;
    let mut missed_count = 0;

    for (_dir, column_arrows) in state.arrows.iter_mut() {
        // Use retain to efficiently remove missed arrows
        column_arrows.retain(|arrow| {
            let beat_diff = current_beat - arrow.target_beat;
             // If the arrow's target beat is past the current beat by more than the miss window
            if beat_diff > miss_window_beats {
                 info!(
                    "MISSED! {:?} {:?} (Tgt: {:.2}, Now: {:.2}, Diff: {:.1}ms > {:.1}ms)",
                    arrow.direction, arrow.note_type, arrow.target_beat, current_beat,
                    beat_diff * seconds_per_beat * 1000.0, config::MISS_WINDOW_MS
                );
                 missed_count += 1;
                // Optional: Add miss feedback (sound, visual penalty) here
                false // Remove the arrow
            } else {
                true // Keep the arrow
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
    // Assumes renderer.begin_frame() was called

    let arrow_texture = assets.get_texture(TextureId::Arrows).expect("Arrow texture missing");
    let now = Instant::now(); // For checking flash state expiry

    // Calculate UVs for the target/arrow animation frame based on beat
    // 4 frames in the atlas (0, 1, 2, 3)
    // Cycle through them every half beat (beat * 2)
    let frame_index = ((game_state.current_beat * 2.0).floor() as usize) % 4;
    let uv_width = 1.0 / 4.0; // Assuming 4 frames horizontally
    let uv_x_start = frame_index as f32 * uv_width;
    let base_uv_offset = [uv_x_start, 0.0]; // Top-left corner of the frame
    let base_uv_scale = [uv_width, 1.0];    // Size of one frame

    // --- Draw Targets ---
    for target in &game_state.targets {
        // Check for active flash, otherwise use default tint
        let current_tint = game_state.flash_states
            .get(&target.direction)
            .filter(|flash| now < flash.end_time) // Check if flash hasn't expired
            .map_or(config::TARGET_TINT, |flash| flash.color); // Use flash color or default tint

        // Determine rotation based on direction
        let rotation_angle = match target.direction {
            ArrowDirection::Left => Rad(PI / 2.0),
            ArrowDirection::Down => Rad(0.0),
            ArrowDirection::Up => Rad(PI),
            ArrowDirection::Right => Rad(-PI / 2.0), // or 3*PI/2
        };

        renderer.draw_quad(
            device, cmd_buf, DescriptorSetId::Gameplay, // Use the gameplay set (arrow texture)
            Vector3::new(target.x, target.y, 0.0),
            (config::TARGET_SIZE, config::TARGET_SIZE),
            rotation_angle,
            current_tint,
            base_uv_offset, // Use base animation frame UVs
            base_uv_scale,
        );
    }

    // --- Draw Arrows ---
    for column_arrows in game_state.arrows.values() {
        for arrow in column_arrows {
            // Basic culling: Don't draw arrows too far off-screen
            if arrow.y > game_state.window_size.1 + config::ARROW_SIZE || arrow.y < -config::ARROW_SIZE {
                continue;
            }

            // Determine tint based on note type
            let arrow_tint = match arrow.note_type {
                NoteType::Quarter => config::ARROW_TINT_QUARTER,
                NoteType::Eighth => config::ARROW_TINT_EIGHTH,
                NoteType::Sixteenth => config::ARROW_TINT_SIXTEENTH,
            };

            // Determine rotation based on direction
            let rotation_angle = match arrow.direction {
                ArrowDirection::Left => Rad(PI / 2.0),
                ArrowDirection::Down => Rad(0.0),
                ArrowDirection::Up => Rad(PI),
                ArrowDirection::Right => Rad(-PI / 2.0),
            };

            renderer.draw_quad(
                device, cmd_buf, DescriptorSetId::Gameplay, // Use the gameplay set (arrow texture)
                Vector3::new(arrow.x, arrow.y, 0.0),
                (config::ARROW_SIZE, config::ARROW_SIZE),
                rotation_angle,
                arrow_tint,
                base_uv_offset, // Use base animation frame UVs
                base_uv_scale,
            );
        }
    }

    // Optional: Draw score, judgment text, combo, etc. using renderer.draw_text
    // let font = assets.get_font(FontId::Main).expect("Font needed for score");
    // renderer.draw_text(device, cmd_buf, font, "Score: 12345", 10.0, 30.0, [1.0; 4]);
}