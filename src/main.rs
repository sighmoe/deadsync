mod api;
mod core;
mod input;
mod renderer;
mod screen;
mod screens;

use input::InputState;
use log::{error, info};
use renderer::{create_backend, BackendType};
use screens::{gameplay, menu, options, Screen as CurrentScreen, ScreenAction};
use std::{collections::HashMap, error::Error, path::Path, sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::Window,
};

const WINDOW_WIDTH: u32 = 800;
const WINDOW_HEIGHT: u32 = 600;

// The main application struct, now with asset management.
struct App {
    window: Option<Arc<Window>>,
    backend: Option<renderer::Backend>,
    backend_type: BackendType,
    // NEW: Manages textures that have been loaded into the GPU.
    texture_manager: HashMap<String, renderer::Texture>,
    current_screen: CurrentScreen,
    menu_state: menu::State,
    gameplay_state: gameplay::State,
    options_state: options::State,
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
            texture_manager: HashMap::new(), // Initialize as empty.
            current_screen: CurrentScreen::Menu,
            menu_state: menu::init(),
            gameplay_state: gameplay::init(),
            options_state: options::init(),
            input_state: input::init_state(),
            frame_count: 0,
            last_title_update: Instant::now(),
            last_frame_time: Instant::now(),
        }
    }

    // NEW: A function to load all required textures from disk into the GPU.
    fn load_textures(&mut self) -> Result<(), Box<dyn Error>> {
        info!("Loading textures...");
        let backend = self
            .backend
            .as_mut()
            .ok_or("Backend not initialized when trying to load textures")?;

        let texture_paths = [
            "logo.png",
            "dance.png",
            "meter_arrow.png",
            "fallback_banner.png",
        ];

        for path_str in texture_paths {
            let full_path = Path::new("assets/graphics").join(path_str);
            let image = image::open(&full_path)?.to_rgba8();

            // NOTE: The following line will cause a compile error until we update `renderer.rs`
            let texture = renderer::create_texture(backend, &image)?;

            self.texture_manager
                .insert(path_str.to_string(), texture);
            info!("Loaded texture: {}", full_path.display());
        }

        Ok(())
    }

    fn handle_action(
        &mut self,
        action: ScreenAction,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), Box<dyn Error>> {
        match action {
            ScreenAction::Navigate(screen) => {
                info!("Navigating to screen: {:?}", screen);
                self.current_screen = screen;

                let (ui_elements, clear_color) = self.get_current_ui_elements();
                let new_screen_data = create_screen_from_ui(&ui_elements, clear_color);

                if let Some(backend) = &mut self.backend {
                    renderer::load_screen(backend, &new_screen_data)?;
                }
            }
            ScreenAction::Exit => {
                info!("Exit action received. Shutting down.");
                event_loop.exit();
            }
            ScreenAction::None => {}
        }
        Ok(())
    }

    fn get_current_ui_elements(&self) -> (Vec<api::UIElement>, [f32; 4]) {
        match self.current_screen {
            CurrentScreen::Menu => (
                menu::get_ui_elements(&self.menu_state),
                [0.03, 0.03, 0.03, 1.0],
            ),
            CurrentScreen::Gameplay => (
                gameplay::get_ui_elements(&self.gameplay_state),
                [0.03, 0.03, 0.03, 1.0],
            ),
            CurrentScreen::Options => (
                options::get_ui_elements(&self.options_state),
                [0.03, 0.03, 0.03, 1.0],
            ),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window_attributes = Window::default_attributes()
                .with_title(format!("Simple Renderer - {:?}", self.backend_type))
                .with_inner_size(PhysicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT))
                .with_resizable(true);

            match event_loop.create_window(window_attributes) {
                Ok(window) => {
                    let window = Arc::new(window);

                    let (ui_elements, clear_color) = self.get_current_ui_elements();
                    let initial_screen = create_screen_from_ui(&ui_elements, clear_color);

                    match create_backend(self.backend_type, window.clone(), &initial_screen) {
                        Ok(backend) => {
                            self.window = Some(window);
                            self.backend = Some(backend);

                            // Load textures now that the backend exists.
                            if let Err(e) = self.load_textures() {
                                error!("Failed to load textures: {}", e);
                                event_loop.exit();
                                return;
                            }

                            info!("Starting event loop...");
                        }
                        Err(e) => {
                            error!("Failed to initialize graphics backend: {}", e);
                            event_loop.exit();
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
        event_loop: &ActiveEventLoop,
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
                    WindowEvent::KeyboardInput {
                        event: key_event, ..
                    } => {
                        input::handle_keyboard_input(&key_event, &mut self.input_state);

                        let action = match self.current_screen {
                            CurrentScreen::Menu => {
                                menu::handle_key_press(&mut self.menu_state, &key_event)
                            }
                            CurrentScreen::Gameplay => {
                                gameplay::handle_key_press(&mut self.gameplay_state, &key_event)
                            }
                            CurrentScreen::Options => {
                                options::handle_key_press(&mut self.options_state, &key_event)
                            }
                        };
                        if let Err(e) = self.handle_action(action, event_loop) {
                            error!("Failed to handle action: {}", e);
                            event_loop.exit();
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let delta_time = now.duration_since(self.last_frame_time).as_secs_f32();
                        self.last_frame_time = now;

                        if self.current_screen == CurrentScreen::Gameplay {
                            gameplay::update(
                                &mut self.gameplay_state,
                                &self.input_state,
                                delta_time,
                            );
                        }

                        let (ui_elements, clear_color) = self.get_current_ui_elements();
                        let screen = create_screen_from_ui(&ui_elements, clear_color);

                        self.frame_count += 1;
                        let elapsed = now.duration_since(self.last_title_update);
                        if elapsed.as_secs_f32() >= 1.0 {
                            let fps = self.frame_count as f32 / elapsed.as_secs_f32();
                            let screen_name = format!("{:?}", self.current_screen);
                            window.set_title(&format!(
                                "Simple Renderer - {:?} | {} | {:.2} FPS",
                                self.backend_type, screen_name, fps
                            ));
                            self.frame_count = 0;
                            self.last_title_update = now;
                        }

                        if let Some(backend) = &mut self.backend {
                            // NOTE: This call is updated and will cause a compile error
                            // until `renderer.rs` is also updated.
                            if let Err(e) = renderer::draw(backend, &screen, &self.texture_manager)
                            {
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

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        info!("Cleaning up resources...");
        self.texture_manager.clear();
        if let Some(backend) = &mut self.backend {
            renderer::cleanup(backend);
        }
    }
}

fn create_screen_from_ui(
    elements: &[api::UIElement],
    clear_color: [f32; 4],
) -> screen::Screen {
    let objects = elements.iter().map(api::to_screen_object).collect();
    screen::Screen {
        clear_color,
        objects,
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

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