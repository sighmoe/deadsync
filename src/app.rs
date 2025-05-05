use crate::assets::{AssetManager};
use crate::audio::AudioManager;
use crate::config;
use crate::graphics::renderer::Renderer;
use crate::graphics::vulkan_base::VulkanBase;
use crate::screens::{gameplay, menu};
use crate::state::{AppState, GameState, MenuState};
use crate::utils::fps::FPSCounter;

use log::{error, info, trace, warn};
use ash::vk;
use std::error::Error;
use std::path::Path;
use std::time::{Duration, Instant};
use winit::{
    event::{Event, WindowEvent, KeyEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{WindowBuilder},
    platform::run_on_demand::EventLoopExtRunOnDemand, // Use this for run_on_demand
};

// Potentially define a trait for screen event handling later
pub trait AppEventHandler {
     fn handle_event(&mut self, event: &WindowEvent) -> Option<AppState>;
     fn update(&mut self, dt: f32);
     // Draw might take more specific arguments like renderer, assets
     // fn draw(&self, renderer: &mut Renderer, assets: &AssetManager);
}

pub struct App {
    // Core systems
    vulkan_base: VulkanBase, // Owns Vulkan core, window, etc.
    renderer: Renderer,
    audio_manager: AudioManager,
    asset_manager: AssetManager,

    // Application state
    current_app_state: AppState,
    menu_state: MenuState,
    game_state: Option<GameState>, // Option because it's created on demand

    // Timing and Utils
    fps_counter: FPSCounter,
    last_frame_time: Instant,
    rng: rand::rngs::ThreadRng, // RNG for gameplay

    // Control Flow / State Change Request
    next_app_state: Option<AppState>,
    resize_needed: bool,
}

impl App {
    pub fn new(event_loop: &EventLoop<()>) -> Result<Self, Box<dyn Error>> {
        info!("Creating Application...");

        // --- Window Setup ---
        info!("Initializing Winit Window...");
        let window = WindowBuilder::new()
            .with_title(config::WINDOW_TITLE)
            .with_inner_size(winit::dpi::LogicalSize::new(
                f64::from(config::WINDOW_WIDTH),
                f64::from(config::WINDOW_HEIGHT),
            ))
            .build(event_loop)?;
        let initial_window_size = window.inner_size();
        let initial_window_size_f32 = (initial_window_size.width as f32, initial_window_size.height as f32);

        // --- Core Systems Setup ---
        let vulkan_base = VulkanBase::new(window)?; // Pass window ownership to VulkanBase
        info!("Vulkan Initialized. GPU: {}", vulkan_base.get_gpu_name());

        let renderer = Renderer::new(&vulkan_base, initial_window_size_f32)?;
        info!("Renderer Initialized.");

        let mut audio_manager = AudioManager::new()?;
        info!("Audio Manager Initialized.");

        let mut asset_manager = AssetManager::new();
        // Load assets and update descriptor sets within the renderer
        asset_manager.load_all(&vulkan_base, &renderer, &mut audio_manager)?;
        info!("Asset Manager Initialized and Assets Loaded.");

        // Wait for any GPU setup tasks (like texture uploads) to finish before starting
        vulkan_base.wait_idle().map_err(|e| format!("Error waiting for GPU idle after setup: {}", e))?;
        info!("GPU idle after setup.");

        Ok(App {
            vulkan_base,
            renderer,
            audio_manager,
            asset_manager,
            current_app_state: AppState::Menu, // Start in the menu
            menu_state: MenuState::default(),
            game_state: None, // Gameplay state initialized later
            fps_counter: FPSCounter::new(),
            last_frame_time: Instant::now(),
            rng: rand::rng(),
            next_app_state: None,
            resize_needed: false,
        })
    }

    /// Runs the main application event loop.
    pub fn run(mut self, mut event_loop: EventLoop<()>) -> Result<(), Box<dyn Error>> {
        info!("Starting Event Loop...");
        self.last_frame_time = Instant::now(); // Reset timer before loop start

        // Use run_on_demand for manual polling control
        event_loop.run_on_demand(move |event, elwt| {
            // Default to Poll, but can change based on events
            elwt.set_control_flow(ControlFlow::Poll);

            match event {
                // --- Handle Window Events ---
                Event::WindowEvent { event: window_event, window_id } if window_id == self.vulkan_base.window.id() => {
                    // Explicitly handle RedrawRequested here OR delegate it below
                    match window_event {
                        WindowEvent::RedrawRequested => {
                            // Render logic (moved from its own arm)
                            match self.render() {
                                Ok(needs_resize) => if needs_resize { self.resize_needed = true; },
                                Err(vk::Result::ERROR_OUT_OF_DATE_KHR | vk::Result::SUBOPTIMAL_KHR) => {
                                    self.resize_needed = true; // Handle swapchain issues
                                }
                                Err(e) => {
                                    error!("Failed to render frame: {:?}", e);
                                    elwt.exit(); // Exit on critical render error
                                }
                            }
                        },
                        // Delegate other window events to the helper function
                        // This will handle CloseRequested, Resized, KeyboardInput, etc.
                        _ => self.handle_window_event(window_event, elwt),
                    }
                }

                // --- Main Loop Logic (Update, Draw Request) ---
                Event::AboutToWait => { // Use AboutToWait for main loop ticks
                     // --- State Transition ---
                    if let Some(new_state) = self.next_app_state.take() {
                        self.transition_state(new_state);
                    }

                    // --- Handle Resize ---
                    // Perform Vulkan resize if requested (flagged by handle_window_event)
                    if self.resize_needed {
                         match self.handle_resize() {
                            Ok(_) => self.resize_needed = false,
                             Err(e) => {
                                 error!("Failed to handle resize: {}. Exiting.", e);
                                 elwt.exit(); // Exit on critical resize error
                             }
                         }
                    }

                     // --- Update ---
                    let now = Instant::now();
                    let dt = (now - self.last_frame_time).as_secs_f32().max(0.0).min(config::MAX_DELTA_TIME);
                    self.last_frame_time = now;
                    self.update(dt);

                    // --- Request Render ---
                    // Request redraw AFTER updating state
                    self.vulkan_base.window.request_redraw();
                },

                Event::LoopExiting => {
                    info!("Event loop exiting.");
                }

                _ => {} // Ignore other event types like DeviceEvent, etc.
            }

             // Check if exit was requested (potentially by handle_window_event)
             if self.current_app_state == AppState::Exiting {
                 elwt.exit();
             }
        })?;

        Ok(())
    }

    /// Handles specific window events.
    fn handle_window_event(&mut self, event: WindowEvent, _elwt: &winit::event_loop::EventLoopWindowTarget<()>) {
        match event {
            WindowEvent::CloseRequested => {
                 info!("Close requested, setting state to Exiting.");
                self.next_app_state = Some(AppState::Exiting);
            }
            WindowEvent::Resized(new_size) => {
                if new_size.width > 0 && new_size.height > 0 {
                     info!("Window resized to: {:?}", new_size);
                    // Don't recreate swapchain immediately, just flag it
                    self.resize_needed = true;
                    // Update internal size for projection matrix update during resize handling
                    // self.current_window_size = (new_size.width as f32, new_size.height as f32);
                }
            }
            WindowEvent::KeyboardInput { event: key_event, .. } => {
                self.handle_keyboard_input(key_event);
            }
            // WindowEvent::RedrawRequested handled in main loop match
            _ => {}
        }
    }

    /// Delegates keyboard input to the active screen.
    fn handle_keyboard_input(&mut self, key_event: KeyEvent) {
         trace!("Keyboard Input: {:?}", key_event);
         let requested_state = match self.current_app_state {
             AppState::Menu => {
                 menu::handle_input(&key_event, &mut self.menu_state, &self.audio_manager)
             }
             AppState::Gameplay => {
                 if let Some(ref mut gs) = self.game_state {
                     gameplay::handle_input(&key_event, gs)
                 } else {
                     warn!("Received input in Gameplay state, but game_state is None.");
                     None
                 }
             }
             AppState::Exiting => None, // Ignore input when exiting
         };

         // If the screen handler requested a state change, queue it
         if requested_state.is_some() {
             self.next_app_state = requested_state;
         }
    }


    /// Performs state transitions and associated setup/teardown.
    fn transition_state(&mut self, new_state: AppState) {
        if new_state == self.current_app_state {
            return; // No transition needed
        }

        info!("Transitioning state from {:?} -> {:?}", self.current_app_state, new_state);

        // --- Teardown for the outgoing state ---
        match self.current_app_state {
            AppState::Gameplay => {
                 // Stop gameplay music, destroy game state
                 self.audio_manager.stop_music();
                 self.game_state = None; // Drop the game state
                 info!("Gameplay state cleared.");
            }
            AppState::Menu => {
                 // Nothing specific needed when leaving menu currently
            }
             AppState::Exiting => {} // Should not transition *from* exiting
        }


        // --- Setup for the incoming state ---
        match new_state {
            AppState::Menu => {
                self.menu_state = MenuState::default(); // Reset menu state
                // Optional: Start menu music?
            }
            AppState::Gameplay => {
                // Initialize gameplay state
                let window_size = (self.vulkan_base.surface_resolution.width as f32, self.vulkan_base.surface_resolution.height as f32);
                 let music_path = Path::new(config::SONG_FOLDER_PATH).join(config::SONG_AUDIO_FILENAME);
                 match self.audio_manager.play_music(&music_path, 1.0) {
                     Ok(_) => {
                         let start_time = Instant::now() + Duration::from_millis(config::AUDIO_SYNC_OFFSET_MS as u64); // Compensate for offset
                         self.game_state = Some(gameplay::initialize_game_state(
                            window_size.0, window_size.1, start_time
                         ));
                         info!("Gameplay state initialized and music started.");
                     }
                     Err(e) => {
                         error!("Failed to start gameplay music: {}. Returning to Menu.", e);
                          // Transition immediately back to Menu on critical error
                          self.current_app_state = AppState::Menu; // Force current state before next_app_state check
                          self.next_app_state = Some(AppState::Menu);
                          return; // Skip setting current_app_state to Gameplay
                     }
                 }
            }
            AppState::Exiting => {
                 // No setup needed for exiting state
            }
        }

        // Update the current state
        self.current_app_state = new_state;
        self.vulkan_base.window.set_title(&format!("{} | {:?}", config::WINDOW_TITLE, self.current_app_state));
    }

    /// Calls the update function for the current screen.
    fn update(&mut self, dt: f32) {
         trace!("Update Start (dt: {:.4} s)", dt);
        match self.current_app_state {
            AppState::Menu => {
                menu::update(&mut self.menu_state, dt);
            }
            AppState::Gameplay => {
                if let Some(ref mut gs) = self.game_state {
                    gameplay::update(gs, dt, &mut self.rng);
                }
            }
            AppState::Exiting => {} // No updates needed when exiting
        }

        // Update FPS counter and window title
        if let Some(fps) = self.fps_counter.update() {
             let title = match self.current_app_state {
                 AppState::Gameplay => format!("{} | Gameplay | FPS: {} | Beat: {:.2}", config::WINDOW_TITLE, fps, self.game_state.as_ref().map_or(0.0, |gs| gs.current_beat)),
                 AppState::Menu => format!("{} | Menu | FPS: {}", config::WINDOW_TITLE, fps),
                 AppState::Exiting => format!("{} | Exiting...", config::WINDOW_TITLE),
             };
             self.vulkan_base.window.set_title(&title);
        }
         trace!("Update End");
    }

     /// Handles Vulkan swapchain recreation and related resource updates on resize.
     fn handle_resize(&mut self) -> Result<(), Box<dyn Error>> {
        info!("Handling resize...");
         self.vulkan_base.wait_idle()?; // Ensure GPU is idle before recreating swapchain

         // TODO: Implement Vulkan swapchain recreation logic here.
         // This involves:
         // 1. Querying new surface capabilities.
         // 2. Destroying old framebuffers, image views, swapchain.
         // 3. Creating new swapchain, image views, framebuffers using the new size/capabilities.
         // 4. Potentially recreating pipelines if they depend on swapchain format/extent (unlikely here).
         // 5. Updating the renderer's projection matrix.

         warn!("Swapchain recreation is not fully implemented!");
         // For now, just update the projection matrix with the last known size
         let new_size = self.vulkan_base.window.inner_size();
         let new_size_f32 = (new_size.width as f32, new_size.height as f32);
         self.renderer.update_projection_matrix(&self.vulkan_base, new_size_f32)?;
         self.vulkan_base.surface_resolution = vk::Extent2D{width: new_size.width, height: new_size.height}; // Update base resolution tracker

         // If gameplay is active, update its window size too
          if let Some(ref mut gs) = self.game_state {
             gs.window_size = new_size_f32;
          }


         info!("Resize handling placeholder complete (projection matrix updated).");
         Ok(())
     }


    /// Performs rendering for the current frame.
    fn render(&mut self) -> Result<bool, vk::Result> {
        // Use VulkanBase's draw_frame, passing a closure for drawing commands
        // Read surface extent *before* the closure that captures renderer/assets/state
        let surface_extent = self.vulkan_base.surface_resolution;

        let draw_result = self.vulkan_base.draw_frame(|device, cmd_buf| {
             trace!("Render: Beginning frame drawing...");
             // Use the renderer to set up the frame and draw the current screen
             self.renderer.begin_frame(device, cmd_buf, surface_extent);

             match self.current_app_state {
                 AppState::Menu => {
                      trace!("Render: Drawing Menu screen...");
                     menu::draw(
                         &self.renderer,
                         &self.menu_state,
                         &self.asset_manager,
                         device, // Pass device/cmd_buf for drawing calls
                         cmd_buf,
                     );
                 }
                 AppState::Gameplay => {
                     if let Some(ref gs) = self.game_state {
                          trace!("Render: Drawing Gameplay screen...");
                         gameplay::draw(
                             &self.renderer,
                             gs,
                             &self.asset_manager,
                             device,
                             cmd_buf,
                         );
                     } else {
                          warn!("Attempted to draw Gameplay state, but game_state is None.");
                          // Optionally draw a placeholder/loading screen
                     }
                 }
                  AppState::Exiting => {
                      // Optionally draw a "Shutting down..." screen
                      trace!("Render: In Exiting state, drawing nothing.");
                  }
             }
             trace!("Render: Frame drawing commands recorded.");
             // end_frame logic is handled by vulkan_base.draw_frame implicitly
        });

        match draw_result {
            Ok(needs_resize) => Ok(needs_resize),
            Err(e) => {
                 error!("Error during Vulkan draw_frame: {:?}", e);
                 Err(e) // Propagate Vulkan errors
            }
        }
    }
}

// Implement Drop to ensure Vulkan resources are cleaned up in the correct order
impl Drop for App {
    fn drop(&mut self) {
        info!("Dropping App - Cleaning up resources...");
        // Ensure GPU is idle before destroying resources that might be in use.
        // Ignore error here as we are already dropping.
        let _ = self.vulkan_base.wait_idle();

         // 1. Stop audio
         self.audio_manager.stop_music(); // Ensure music sink is released
         info!("Audio stopped.");

        // 2. Destroy assets (which contain Vulkan resources like textures/fonts)
        self.asset_manager.destroy(&self.vulkan_base.device);
         info!("Assets destroyed.");

        // 3. Destroy renderer resources (pipelines, layouts, buffers)
        self.renderer.destroy(&self.vulkan_base.device);
         info!("Renderer destroyed.");

        // 4. VulkanBase's Drop implementation will handle the rest
        // (command pools, framebuffers, swapchain, device, instance, etc.)
        // The window is also implicitly dropped when VulkanBase drops.
        info!("App cleanup finished.");
    }
}