use crate::assets::AssetManager;
use crate::audio::AudioManager;
use crate::config;
use crate::graphics::renderer::Renderer;
use crate::graphics::vulkan_base::VulkanBase;
use crate::screens::{gameplay, menu, options, select_music};
use crate::state::{AppState, GameState, MenuState, OptionsState, SelectMusicState};
use crate::utils::fps::FPSCounter;

use ash::vk;
use log::{error, info, trace, warn};
use std::error::Error;
use std::path::Path;
use std::time::{Duration, Instant};
use winit::{
    dpi::PhysicalSize,
    event::{Event, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_on_demand::EventLoopExtRunOnDemand,
    window::WindowBuilder,
};

const RESIZE_DEBOUNCE_DURATION: Duration = Duration::from_millis(0); // Increased debounce further

pub struct App {
    vulkan_base: VulkanBase,
    renderer: Renderer,
    audio_manager: AudioManager,
    asset_manager: AssetManager,
    current_app_state: AppState,
    menu_state: MenuState,
    select_music_state: SelectMusicState,
    options_state: OptionsState,
    game_state: Option<GameState>,
    fps_counter: FPSCounter,
    last_frame_time: Instant,
    rng: rand::rngs::ThreadRng,
    next_app_state: Option<AppState>,
    pending_resize: Option<(PhysicalSize<u32>, Instant)>,
    // Flag to indicate if the last render attempt resulted in an OOD/Suboptimal error
    // This helps prevent trying to render again immediately if we know the swapchain is bad.
    swapchain_is_known_bad: bool,
}

impl App {
    pub fn new(event_loop: &EventLoop<()>) -> Result<Self, Box<dyn Error>> {
        info!("Creating Application...");

        let window = WindowBuilder::new()
            .with_title(config::WINDOW_TITLE)
            .with_inner_size(winit::dpi::LogicalSize::new(
                f64::from(config::WINDOW_WIDTH),
                f64::from(config::WINDOW_HEIGHT),
            ))
            .build(event_loop)?;

        let vulkan_base = VulkanBase::new(window)?;
        info!("Vulkan Initialized. GPU: {}", vulkan_base.get_gpu_name());

        let initial_surface_resolution = vulkan_base.surface_resolution;
        let renderer = Renderer::new(
            &vulkan_base,
            (
                initial_surface_resolution.width as f32,
                initial_surface_resolution.height as f32,
            ),
        )?;
        info!("Renderer Initialized.");

        let mut audio_manager = AudioManager::new()?;
        info!("Audio Manager Initialized.");

        let mut asset_manager = AssetManager::new();
        asset_manager.load_all(&vulkan_base, &renderer, &mut audio_manager)?;
        info!("Asset Manager Initialized and Assets Loaded.");

        vulkan_base
            .wait_idle()
            .map_err(|e| format!("Error waiting for GPU idle after setup: {}", e))?;
        info!("GPU idle after setup.");

        Ok(App {
            vulkan_base,
            renderer,
            audio_manager,
            asset_manager,
            current_app_state: AppState::Menu,
            menu_state: MenuState::default(),
            select_music_state: SelectMusicState::default(),
            options_state: OptionsState::default(),
            game_state: None,
            fps_counter: FPSCounter::new(),
            last_frame_time: Instant::now(),
            rng: rand::rng(),
            next_app_state: None,
            pending_resize: None,
            swapchain_is_known_bad: false, // Initially, assume swapchain is good
        })
    }

    pub fn run(mut self, mut event_loop: EventLoop<()>) -> Result<(), Box<dyn Error>> {
        info!("Starting Event Loop...");
        self.last_frame_time = Instant::now();

        event_loop.run_on_demand(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);

            match event {
                Event::WindowEvent { event: window_event, window_id } if window_id == self.vulkan_base.window.id() => {
                    match window_event {
                        WindowEvent::RedrawRequested => {
                            if self.swapchain_is_known_bad {
                                trace!("Skipping render because swapchain is known to be bad. Waiting for resize.");
                                // Request another redraw to re-evaluate after potential resize in AboutToWait
                                self.vulkan_base.window.request_redraw();
                            } else {
                                match self.render() {
                                    Ok(needs_resize_from_render) => {
                                        if needs_resize_from_render {
                                            info!("Render reported suboptimal swapchain, scheduling resize check.");
                                            let current_size = self.vulkan_base.window.inner_size();
                                            self.pending_resize = Some((current_size, Instant::now()));
                                            self.swapchain_is_known_bad = true;
                                        } else {
                                            self.swapchain_is_known_bad = false; // Render was successful
                                        }
                                    }
                                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR) => {
                                        info!("Render failed with out-of-date/suboptimal, scheduling resize check.");
                                        let current_size = self.vulkan_base.window.inner_size();
                                        self.pending_resize = Some((current_size, Instant::now()));
                                        self.swapchain_is_known_bad = true;
                                    }
                                    Err(e) => {
                                        error!("Failed to render frame: {:?}. Exiting.", e);
                                        elwt.exit();
                                    }
                                }
                            }
                        },
                        _ => self.handle_window_event(window_event, elwt),
                    }
                }

                Event::AboutToWait => {
                    if let Some(new_state) = self.next_app_state.take() {
                        self.transition_state(new_state);
                    }

                    if self.try_process_pending_resize().is_err() {
                        error!("Failed to process pending resize in AboutToWait. Exiting.");
                        elwt.exit();
                        return;
                    }

                    // Only update and request redraw if we don't have a pending resize
                    // or if the swapchain isn't known to be bad.
                    // If a resize just happened, swapchain_is_known_bad became false.
                    if self.pending_resize.is_none() && !self.swapchain_is_known_bad {
                        let now = Instant::now();
                        let dt = (now - self.last_frame_time).as_secs_f32().max(0.0).min(config::MAX_DELTA_TIME);
                        self.last_frame_time = now;
                        self.update(dt);
                        self.vulkan_base.window.request_redraw();
                    } else if self.pending_resize.is_some() || self.swapchain_is_known_bad {
                        // If resize is pending or swapchain is bad, still request redraw.
                        // RedrawRequested will skip render if bad, or handle resize if pending.
                        // This ensures we keep polling for the debounce timer.
                        self.vulkan_base.window.request_redraw();
                    }
                },

                Event::LoopExiting => {
                    info!("Event loop exiting.");
                }

                _ => {}
            }

            if self.current_app_state == AppState::Exiting {
                 elwt.exit();
             }
        })?;

        Ok(())
    }

    fn try_process_pending_resize(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some((target_size, last_event_time)) = self.pending_resize {
            if Instant::now().duration_since(last_event_time) >= RESIZE_DEBOUNCE_DURATION {
                info!(
                    "Debounce time elapsed, processing resize to {:?}.",
                    target_size
                );
                let actual_target_size = self.pending_resize.take().unwrap().0; // Consume the pending resize

                match self.handle_actual_resize(actual_target_size) {
                    Ok(_) => {
                        self.swapchain_is_known_bad = false; // Resize successful, swapchain should be good
                        info!("Resize processed successfully.");
                    }
                    Err(e) => {
                        // If resize failed, keep swapchain_is_known_bad = true (or re-set it)
                        // and re-queue the resize to try again.
                        error!("handle_actual_resize failed: {}. Re-queueing resize.", e);
                        self.pending_resize = Some((actual_target_size, Instant::now()));
                        self.swapchain_is_known_bad = true;
                        return Err(e); // Propagate error if critical, or just log and retry
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_window_event(
        &mut self,
        event: WindowEvent,
        _elwt: &winit::event_loop::EventLoopWindowTarget<()>,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                info!("Close requested, setting state to Exiting.");
                self.next_app_state = Some(AppState::Exiting);
            }
            WindowEvent::Resized(new_size) => {
                trace!(
                    "Window resized event (raw): {:?}, updating pending resize.",
                    new_size
                );
                self.pending_resize = Some((new_size, Instant::now()));
                // No need to set swapchain_is_known_bad here, render will determine that.
            }
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                self.handle_keyboard_input(key_event);
            }
            _ => {}
        }
    }

    fn handle_keyboard_input(&mut self, key_event: KeyEvent) {
        // ... (no changes) ...
        trace!("Keyboard Input: {:?}", key_event);
        let requested_state = match self.current_app_state {
            AppState::Menu => {
                menu::handle_input(&key_event, &mut self.menu_state, &self.audio_manager)
            }
            AppState::SelectMusic => select_music::handle_input(
                &key_event,
                &mut self.select_music_state,
                &self.audio_manager,
            ),
            AppState::Options => options::handle_input(&key_event, &mut self.options_state),
            AppState::Gameplay => {
                if let Some(ref mut gs) = self.game_state {
                    gameplay::handle_input(&key_event, gs)
                } else {
                    warn!("Received input in Gameplay state, but game_state is None.");
                    None
                }
            }
            AppState::Exiting => None,
        };

        if requested_state.is_some() {
            self.next_app_state = requested_state;
        }
    }

    fn transition_state(&mut self, new_state: AppState) {
        // ... (no changes) ...
        if new_state == self.current_app_state {
            return;
        }
        info!(
            "Transitioning state from {:?} -> {:?}",
            self.current_app_state, new_state
        );
        match self.current_app_state {
            AppState::Gameplay => {
                self.audio_manager.stop_music();
                self.game_state = None;
                info!("Gameplay state cleared.");
            }
            _ => {}
        }

        match new_state {
            AppState::Menu => {
                self.menu_state = MenuState::default();
            }
            AppState::SelectMusic => {
                self.select_music_state = SelectMusicState::default();
            }
            AppState::Options => {
                self.options_state = OptionsState::default();
            }
            AppState::Gameplay => {
                info!("Initializing Gameplay State...");
                let window_size_f32 = (
                    self.vulkan_base.surface_resolution.width as f32,
                    self.vulkan_base.surface_resolution.height as f32,
                );
                let music_path =
                    Path::new(config::SONG_FOLDER_PATH).join(config::SONG_AUDIO_FILENAME);
                match self.audio_manager.play_music(&music_path, 1.0) {
                    Ok(_) => {
                        let start_time = Instant::now()
                            + Duration::from_millis(config::AUDIO_SYNC_OFFSET_MS as u64);
                        self.game_state = Some(gameplay::initialize_game_state(
                            window_size_f32.0,
                            window_size_f32.1,
                            start_time,
                        ));
                        info!("Gameplay state initialized and music started.");
                    }
                    Err(e) => {
                        error!(
                            "Failed to start gameplay music: {}. Returning to SelectMusic.",
                            e
                        );
                        self.current_app_state = AppState::SelectMusic;
                        self.next_app_state = Some(AppState::SelectMusic);
                        return;
                    }
                }
            }
            AppState::Exiting => { /* No setup */ }
        }
        self.current_app_state = new_state;
        self.vulkan_base.window.set_title(&format!(
            "{} | {:?}",
            config::WINDOW_TITLE,
            self.current_app_state
        ));
    }

    fn update(&mut self, dt: f32) {
        // ... (no changes) ...
        trace!("Update Start (dt: {:.4} s)", dt);
        match self.current_app_state {
            AppState::Menu => menu::update(&mut self.menu_state, dt),
            AppState::SelectMusic => select_music::update(&mut self.select_music_state, dt),
            AppState::Options => options::update(&mut self.options_state, dt),
            AppState::Gameplay => {
                if let Some(ref mut gs) = self.game_state {
                    gameplay::update(gs, dt, &mut self.rng);
                }
            }
            AppState::Exiting => {}
        }

        if let Some(fps) = self.fps_counter.update() {
            let title_suffix = match self.current_app_state {
                AppState::Gameplay => format!(
                    "Gameplay | FPS: {} | Beat: {:.2}",
                    fps,
                    self.game_state.as_ref().map_or(0.0, |gs| gs.current_beat)
                ),
                AppState::Menu => format!("Menu | FPS: {}", fps),
                AppState::SelectMusic => format!("Select Music | FPS: {}", fps),
                AppState::Options => format!("Options | FPS: {}", fps),
                AppState::Exiting => "Exiting...".to_string(),
            };
            self.vulkan_base.window.set_title(&format!(
                "{} | {}",
                config::WINDOW_TITLE,
                title_suffix
            ));
        }
        trace!("Update End");
    }

    fn handle_actual_resize(
        &mut self,
        target_size: PhysicalSize<u32>,
    ) -> Result<(), Box<dyn Error>> {
        info!("Handling actual resize to physical size: {:?}", target_size);

        if target_size.width == 0 || target_size.height == 0 {
            info!(
                "Window is minimized or zero size ({:?}). Deferring Vulkan resize further.",
                target_size
            );
            self.pending_resize = Some((target_size, Instant::now()));
            self.swapchain_is_known_bad = true; // Still bad if we can't resize
            return Ok(());
        }

        // Attempt to rebuild
        self.vulkan_base
            .rebuild_swapchain_resources(target_size.width, target_size.height)?;
        // If successful, swapchain is no longer known to be bad from this path.
        // (It might become bad on the next render, but this attempt was "successful")
        // This is handled in try_process_pending_resize or RedrawRequested.

        let new_vulkan_surface_extent = self.vulkan_base.surface_resolution;
        let new_vulkan_size_f32 = (
            new_vulkan_surface_extent.width as f32,
            new_vulkan_surface_extent.height as f32,
        );

        self.renderer
            .update_projection_matrix(&self.vulkan_base, new_vulkan_size_f32)?;

        if let Some(ref mut gs) = self.game_state {
            gs.window_size = new_vulkan_size_f32;
        }

        info!(
            "Actual resize handling complete. New Vulkan surface resolution: {:?}",
            new_vulkan_surface_extent
        );
        Ok(())
    }

    fn render(&mut self) -> Result<bool, vk::Result> {
        let current_surface_resolution = self.vulkan_base.surface_resolution;
        if current_surface_resolution.width == 0 || current_surface_resolution.height == 0 {
            trace!("Skipping render due to zero-sized surface resolution.");
            return Ok(true); // Needs resize
        }

        let draw_result = self.vulkan_base.draw_frame(|device, cmd_buf| {
            trace!("Render: Beginning frame drawing...");
            self.renderer
                .begin_frame(device, cmd_buf, current_surface_resolution);

            match self.current_app_state {
                AppState::Menu => {
                    menu::draw(
                        &self.renderer,
                        &self.menu_state,
                        &self.asset_manager,
                        device,
                        cmd_buf,
                    );
                }
                AppState::SelectMusic => {
                    select_music::draw(
                        &self.renderer,
                        &self.select_music_state,
                        &self.asset_manager,
                        device,
                        cmd_buf,
                    );
                }
                AppState::Options => {
                    options::draw(
                        &self.renderer,
                        &self.options_state,
                        &self.asset_manager,
                        device,
                        cmd_buf,
                    );
                }
                AppState::Gameplay => {
                    if let Some(ref gs) = self.game_state {
                        gameplay::draw(&self.renderer, gs, &self.asset_manager, device, cmd_buf);
                    } else {
                        warn!("Attempted to draw Gameplay state, but game_state is None.");
                    }
                }
                AppState::Exiting => {
                    trace!("Render: In Exiting state, drawing nothing specific.");
                }
            }
            trace!("Render: Frame drawing commands recorded.");
        });

        match draw_result {
            Ok(needs_resize) => Ok(needs_resize),
            Err(e @ vk::Result::ERROR_OUT_OF_DATE_KHR) | Err(e @ vk::Result::SUBOPTIMAL_KHR) => {
                Err(e)
            }
            Err(e) => {
                error!("Error during Vulkan draw_frame: {:?}", e);
                Err(e)
            }
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        info!("Dropping App - Cleaning up resources...");
        if let Err(e) = self.vulkan_base.wait_idle() {
            error!("Error waiting for GPU idle during App drop: {}", e);
        }

        self.audio_manager.stop_music();
        info!("Audio stopped.");

        self.asset_manager.destroy(&self.vulkan_base.device);
        info!("Assets destroyed.");

        self.renderer.destroy(&self.vulkan_base.device);
        info!("Renderer destroyed.");

        info!("App cleanup finished. VulkanBase will now be dropped.");
    }
}
