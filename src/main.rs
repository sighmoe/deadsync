mod game;
mod input;
mod core;
mod renderer;
mod screen;

use cgmath::{Matrix4};
use game::GameState;
use input::InputState;
use log::{error, info};
use renderer::{create_backend, BackendType};
use screen::{Screen, ScreenObject};
use std::{error::Error, sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::EventLoop,
    window::Window,
};

pub struct RenderConfig {
    pub clear_color: [f32; 4],
}

const WINDOW_WIDTH: u32 = 800;
const WINDOW_HEIGHT: u32 = 600;

struct App {
    window: Option<Arc<Window>>,
    backend: Option<renderer::Backend>,
    backend_type: BackendType,
    game_state: GameState,
    input_state: InputState,
    frame_count: u32,
    last_title_update: Instant,
    last_frame_time: Instant,
}

impl App {
    fn new(backend_type: BackendType) -> Self {
        App {
            window: None,
            backend: None,
            backend_type,
            game_state: game::init_state(),
            input_state: input::init_state(),
            frame_count: 0,
            last_title_update: Instant::now(),
            last_frame_time: Instant::now(),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.window.is_none() {
            let window_attributes = Window::default_attributes()
                .with_title(format!("Simple Renderer - {:?}", self.backend_type))
                .with_inner_size(PhysicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT))
                .with_resizable(true);

            match event_loop.create_window(window_attributes) {
                Ok(window) => {
                    let window = Arc::new(window);
                    let initial_screen = create_screen_from_state(&self.game_state);
                    match create_backend(self.backend_type, window.clone(), &initial_screen) {
                        Ok(backend) => {
                            self.window = Some(window);
                            self.backend = Some(backend);
                            info!("Starting event loop...");
                        }
                        Err(e) => {
                            error!("Failed to initialize graphics backend: {}", e);
                            if self.backend_type == BackendType::Vulkan {
                                info!("Vulkan failed. Trying to fall back to OpenGL...");
                                match create_backend(BackendType::OpenGL, window.clone(), &initial_screen) {
                                    Ok(backend) => {
                                        self.window = Some(window);
                                        self.backend = Some(backend);
                                        info!("Starting event loop with OpenGL fallback.");
                                    }
                                    Err(e2) => {
                                        error!("OpenGL fallback also failed: {}", e2);
                                        event_loop.exit();
                                    }
                                }
                            } else {
                                event_loop.exit();
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to create window: {}", e);
                    event_loop.exit();
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        if let Some(window) = &self.window {
            if window_id == window.id() {
                match event {
                    WindowEvent::CloseRequested => {
                        info!("Close requested. Shutting down.");
                        event_loop.exit();
                    }
                    WindowEvent::Resized(new_size) => {
                        info!("Window resized to: {}x{}", new_size.width, new_size.height);
                        if new_size.width > 0 && new_size.height > 0 {
                            if let Some(backend) = &mut self.backend {
                                renderer::resize(backend, new_size.width, new_size.height);
                            }
                        }
                    }
                    WindowEvent::KeyboardInput { event: key_event, .. } => {
                        input::handle_keyboard_input(&key_event, &mut self.input_state);
                    }
                    WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let delta_time = now.duration_since(self.last_frame_time).as_secs_f32();
                        self.last_frame_time = now;

                        game::update_state(&mut self.game_state, &self.input_state, delta_time);
                        let screen = create_screen_from_state(&self.game_state);

                        self.frame_count += 1;
                        let elapsed = now.duration_since(self.last_title_update);
                        if elapsed.as_secs_f32() >= 1.0 {
                            let fps = self.frame_count as f32 / elapsed.as_secs_f32();
                            window.set_title(&format!(
                                "Simple Renderer - {:?} | {:.2} FPS",
                                self.backend_type, fps
                            ));
                            self.frame_count = 0;
                            self.last_title_update = now;
                        }

                        if let Some(backend) = &mut self.backend {
                            if let Err(e) = renderer::draw(backend, &screen) {
                                error!("Failed to draw frame: {}", e);
                                event_loop.exit();
                            }
                        }
                    }
                    _ => (),
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn exiting(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        info!("Cleaning up resources...");
        if let Some(backend) = &mut self.backend {
            renderer::cleanup(backend);
        }
    }
}

fn create_screen_from_state(game_state: &GameState) -> Screen {
    Screen {
        clear_color: [0.03, 0.03, 0.03, 1.0], // #191919
        objects: vec![ScreenObject {
            vertices: vec![
                [-50.0, -50.0],
                [50.0, -50.0],
                [50.0, 50.0],
                [-50.0, 50.0],
            ],
            indices: vec![0, 1, 2, 2, 3, 0],
            color: [0.0, 0.0, 1.0, 1.0], // Blue
            transform: Matrix4::from_translation(game_state.square_position),
        }],
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    let backend_type = match args.get(1).map(|s| s.as_str()) {
        Some("--opengl") => BackendType::OpenGL,
        Some("--vulkan") => BackendType::Vulkan,
        _ => {
            info!("No backend specified. Defaulting to Vulkan.");
            info!("Use '--opengl' or '--vulkan' to select a backend.");
            BackendType::Vulkan
        }
    };

    let event_loop = EventLoop::new()?;
    let mut app = App::new(backend_type);
    event_loop.run_app(&mut app)?;

    Ok(())
}