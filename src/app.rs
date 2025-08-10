// src/app.rs
use crate::core::input;
use crate::core::input::InputState;
use crate::core::gfx as renderer;
use crate::core::gfx::{create_backend, BackendType};
use crate::ui::primitives as api;
use crate::screens::{gameplay, menu, options, Screen as CurrentScreen, ScreenAction};

use log::{error, info};
use image;
use std::{collections::HashMap, error::Error, path::Path, sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::Window,
};

const WINDOW_WIDTH: u32 = 1024;
const WINDOW_HEIGHT: u32 = 768;

// ---- args ----
fn parse_args(args: &[String]) -> (BackendType, bool) {
    use log::warn;

    let mut backend = BackendType::Vulkan;
    let mut vsync = true;

    let mut it = args.iter().skip(1).peekable();
    while let Some(a) = it.next() {
        let a = a.as_str();

        match a {
            // legacy toggles (kept)
            "--opengl" => backend = BackendType::OpenGL,
            "--vulkan" => backend = BackendType::Vulkan,

            // new: --backend=opengl|vulkan and -b opengl|vulkan
            s if s.starts_with("--backend=") => {
                match &s["--backend=".len()..].to_ascii_lowercase()[..] {
                    "opengl" => backend = BackendType::OpenGL,
                    "vulkan" => backend = BackendType::Vulkan,
                    other => warn!("Unknown backend '{}'; keeping {:?}", other, backend),
                }
            }
            "-b" => {
                if let Some(next) = it.peek() {
                    let v = next.to_ascii_lowercase();
                    match v.as_str() {
                        "opengl" => { backend = BackendType::OpenGL; it.next(); }
                        "vulkan" => { backend = BackendType::Vulkan; it.next(); }
                        _ => warn!("Unknown backend '{}'; keeping {:?}", v, backend),
                    }
                } else {
                    warn!("-b requires a value (opengl|vulkan); keeping {:?}", backend);
                }
            }

            // vsync controls
            "--no-vsync" => vsync = false,
            s if s.starts_with("--vsync=") => {
                let v = s["--vsync=".len()..].to_ascii_lowercase();
                vsync = matches!(v.as_str(), "on" | "true" | "1");
            }
            "--vsync" => {
                if let Some(next) = it.peek() {
                    let v = next.to_ascii_lowercase();
                    if matches!(v.as_str(), "on" | "off" | "true" | "false" | "1" | "0") {
                        vsync = matches!(v.as_str(), "on" | "true" | "1");
                        it.next();
                    } else {
                        // plain `--vsync` means "on"
                        vsync = true;
                    }
                } else {
                    vsync = true;
                }
            }

            // unknown
            other => warn!("Unknown arg: {}", other),
        }
    }

    (backend, vsync)
}

// ---- app state ----
pub struct App {
    window: Option<Arc<Window>>,
    backend: Option<renderer::Backend>,
    backend_type: BackendType,
    texture_manager: HashMap<&'static str, renderer::Texture>,
    current_screen: CurrentScreen,
    menu_state: menu::State,
    gameplay_state: gameplay::State,
    options_state: options::State,
    input_state: InputState,
    frame_count: u32,
    last_title_update: Instant,
    last_frame_time: Instant,
    vsync_enabled: bool,
}

impl App {
    fn new(backend_type: BackendType, vsync_enabled: bool) -> Self {
        Self {
            window: None,
            backend: None,
            backend_type,
            texture_manager: HashMap::new(),
            current_screen: CurrentScreen::Menu,
            menu_state: menu::init(),
            gameplay_state: gameplay::init(),
            options_state: options::init(),
            input_state: input::init_state(),
            frame_count: 0,
            last_title_update: Instant::now(),
            last_frame_time: Instant::now(),
            vsync_enabled,
        }
    }

    fn load_textures(&mut self) -> Result<(), Box<dyn Error>> {
        use log::{info, warn};

        info!("Loading textures...");
        let backend = self.backend.as_mut().ok_or("Backend not initialized")?;

        // Helper: tiny 2x2 magenta checker fallback
        #[inline(always)]
        fn fallback_rgba() -> image::RgbaImage {
            // 2x2: M G
            //      G M   (M=magenta, G=gray)
            let data: [u8; 16] = [
                255, 0,   255, 255,   128, 128, 128, 255,
                128, 128, 128, 255,   255, 0,   255, 255,
            ];
            image::RgbaImage::from_raw(2, 2, data.to_vec()).expect("fallback image")
        }

        // Keep desired logical IDs -> filenames
        let texture_paths: [&'static str; 4] = [
            "logo.png",
            "dance.png",
            "meter_arrow.png",
            "fallback_banner.png",
        ];

        for key in texture_paths {
            let full_path = Path::new("assets/graphics").join(key);
            match image::open(&full_path) {
                Ok(img_dyn) => {
                    let image = img_dyn.to_rgba8();
                    let texture = renderer::create_texture(backend, &image)?;
                    self.texture_manager.insert(key, texture);
                    info!("Loaded texture: {}", full_path.display());
                }
                Err(e) => {
                    warn!(
                        "Failed to load '{}': {}. Using generated fallback.",
                        full_path.display(),
                        e
                    );
                    let fb = fallback_rgba();
                    let texture = renderer::create_texture(backend, &fb)?;
                    self.texture_manager.insert(key, texture);
                }
            }
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
            CurrentScreen::Menu => (menu::get_ui_elements(&self.menu_state), [0.03, 0.03, 0.03, 1.0]),
            CurrentScreen::Gameplay => (gameplay::get_ui_elements(&self.gameplay_state), [0.03, 0.03, 0.03, 1.0]),
            CurrentScreen::Options => (options::get_ui_elements(&self.options_state), [0.03, 0.03, 0.03, 1.0]),
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

                    match create_backend(self.backend_type, window.clone(), &initial_screen, self.vsync_enabled) {
                        Ok(backend) => {
                            self.window = Some(window);
                            self.backend = Some(backend);
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
                    WindowEvent::KeyboardInput { event: key_event, .. } => {
                        input::handle_keyboard_input(&key_event, &mut self.input_state);

                        let action = match self.current_screen {
                            CurrentScreen::Menu => menu::handle_key_press(&mut self.menu_state, &key_event),
                            CurrentScreen::Gameplay => gameplay::handle_key_press(&mut self.gameplay_state, &key_event),
                            CurrentScreen::Options => options::handle_key_press(&mut self.options_state, &key_event),
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
                            gameplay::update(&mut self.gameplay_state, &self.input_state, delta_time);
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
                            if let Err(e) = renderer::draw(backend, &screen, &self.texture_manager) {
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
        if let Some(backend) = &mut self.backend {
            renderer::dispose_textures(backend, &mut self.texture_manager);
            renderer::cleanup(backend);
        }
    }
}

// ---- helpers ----
fn create_screen_from_ui(
    elements: &[api::UIElement],
    clear_color: [f32; 4],
) -> renderer::Screen {
    let objects = elements.iter().map(api::to_screen_object).collect();
    renderer::Screen { clear_color, objects }
}

// ---- public entry point ----
pub fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    let (backend_type, vsync_enabled) = parse_args(&args);

    let backend_was_specified = args.iter().any(|a| a == "--opengl" || a == "--vulkan");
    if !backend_was_specified {
        info!("No backend specified. Defaulting to Vulkan.");
        info!("Use '--opengl' or '--vulkan' to select a backend.");
    }

    let event_loop = EventLoop::new()?;
    let mut app = App::new(backend_type, vsync_enabled);
    event_loop.run_app(&mut app)?;
    Ok(())
}
