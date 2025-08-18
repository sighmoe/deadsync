// src/app.rs
use crate::core::input;
use crate::core::input::InputState;
use crate::core::gfx as renderer;
use crate::core::gfx::{create_backend, BackendType};
use crate::core::space::{self as space, Metrics};
use crate::ui::actors::Actor;
use crate::ui::msdf;
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

const WINDOW_WIDTH: u32 = 1280;
const WINDOW_HEIGHT: u32 = 800;

// ---- args ----
fn parse_args(args: &[String]) -> (BackendType, bool) {
    #[inline(always)]
    fn parse_bool_token(s: &str) -> Option<bool> {
        match s {
            "on" | "true" | "1"  => Some(true),
            "off" | "false" | "0" => Some(false),
            _ => None,
        }
    }

    let mut backend = BackendType::Vulkan;
    let mut vsync   = true;

    let mut i = 1;
    while i < args.len() {
        let a = args[i].as_str();
        match a {
            "--opengl"        => backend = BackendType::OpenGL,
            "--vulkan"        => backend = BackendType::Vulkan,
            "--no-vsync"      => vsync = false,
            "--vsync"         => {
                if i + 1 < args.len() {
                    if let Some(v) = parse_bool_token(args[i + 1].as_str()) {
                        vsync = v;
                        i += 1;
                    } else {
                        vsync = true; // plain `--vsync`
                    }
                } else {
                    vsync = true;     // plain `--vsync`
                }
            }
            _ if a.starts_with("--vsync=") => {
                let v = &a["--vsync=".len()..];
                vsync = parse_bool_token(v).unwrap_or(true);
            }
            _ => {}
        }
        i += 1;
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
    fonts: HashMap<&'static str, msdf::Font>,
    metrics: Metrics,
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
            metrics: space::metrics_for_window(WINDOW_WIDTH, WINDOW_HEIGHT),
            vsync_enabled,
            fonts: HashMap::new(),
        }
    }

    fn load_textures(&mut self) -> Result<(), Box<dyn Error>> {
        use log::{info, warn};
        use std::sync::Arc;

        info!("Loading textures...");
        let backend = self.backend.as_mut().ok_or("Backend not initialized")?;

        #[inline(always)]
        fn fallback_rgba() -> image::RgbaImage {
            let data: [u8; 16] = [
                255, 0,   255, 255,   128, 128, 128, 255,
                128, 128, 128, 255,   255, 0,   255, 255,
            ];
            image::RgbaImage::from_raw(2, 2, data.to_vec()).expect("fallback image")
        }

        // Keep desired logical IDs -> filenames
        let texture_paths: [&'static str; 5] = [
            "logo.png",
            "dance.png",
            "meter_arrow.png",
            "fallback_banner.png",
            "hearts_4x4.png",
        ];

        // 1) Decode images in parallel (CPU-only work)
        let handles: Vec<_> = texture_paths
            .iter()
            .map(|&key| {
                let p = Path::new("assets/graphics").join(key);
                std::thread::spawn(move || {
                    match image::open(&p) {
                        Ok(img) => Ok::<(&'static str, image::RgbaImage), (&'static str, String)>((key, img.to_rgba8())),
                        Err(e) => Err((key, e.to_string())),
                    }
                })
            })
            .collect();

        // Create the fallback image once and wrap in an Arc for cheap cloning.
        let fallback_image = Arc::new(fallback_rgba());
        let mut decoded: Vec<(&'static str, Arc<image::RgbaImage>)> = Vec::with_capacity(texture_paths.len());
        for h in handles {
            match h.join().expect("texture decode thread panicked") {
                Ok((key, rgba)) => decoded.push((key, Arc::new(rgba))),
                Err((key, msg)) => {
                    warn!("Failed to load 'assets/graphics/{}': {}. Using generated fallback.", key, msg);
                    // Clone the Arc (cheap) instead of the image buffer (expensive).
                    decoded.push((key, fallback_image.clone()));
                }
            }
        }

        // 2) Create GPU textures sequentially
        for (key, rgba_arc) in decoded {
            // All UI sprites are authored in sRGB space.
            let texture = renderer::create_texture(
                backend,
                &rgba_arc,
                renderer::TextureColorSpace::Srgb,
            )?;

            self.texture_manager.insert(key, texture);
            info!("Loaded texture: assets/graphics/{}", key);
        }
        Ok(())
    }

    fn load_font_asset(&mut self, name: &'static str) -> Result<(), Box<dyn Error>> {
        let backend = self.backend.as_mut().ok_or("Backend not initialized")?;
        let json_path = format!("assets/fonts/{}.json", name);
        let png_path  = format!("assets/fonts/{}.png",  name);

        // Read JSON and atlas image from disk
        let json_data  = std::fs::read(&json_path)?;
        let image_data = image::open(&png_path)?.to_rgba8();

        // Upload atlas texture to GPU (MSDF wants linear color space)
        let texture = renderer::create_texture(
            backend,
            &image_data,
            renderer::TextureColorSpace::Linear,
        )?;
        // Use the font NAME as the texture key; avoids leaking boxed strings.
        self.texture_manager.insert(name, texture);

        // Parse JSON and store font metrics; refer to the texture by NAME
        let font = msdf::load_font(&json_data, name, 4.0);
        self.fonts.insert(name, font);
        info!("Loaded font '{}'", name);
        Ok(())
    }

    fn load_fonts(&mut self) -> Result<(), Box<dyn Error>> {
        self.load_font_asset("wendy")?;
        self.load_font_asset("miso")?;
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
            }
            ScreenAction::Exit => {
                info!("Exit action received. Shutting down.");
                event_loop.exit();
            }
            ScreenAction::None => {}
        }
        Ok(())
    }

    fn build_screen(&self, actors: &[Actor], clear_color: [f32; 4]) -> renderer::Screen {
        crate::ui::layout::build_screen(actors, clear_color, &self.metrics, &self.fonts)
    }

    fn get_current_actors(&self) -> (Vec<Actor>, [f32; 4]) {
        const CLEAR: [f32; 4] = [0.03, 0.03, 0.03, 1.0];

        let actors = match self.current_screen {
            CurrentScreen::Menu => menu::get_actors(&self.menu_state, &self.metrics),
            CurrentScreen::Gameplay => gameplay::get_actors(&self.gameplay_state),
            CurrentScreen::Options => options::get_actors(&self.options_state),
        };

        (actors, CLEAR)
    }

    #[inline(always)]
    fn update_fps_title(&mut self, window: &Window, now: Instant) {
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
    }

    /// Creates the window, initializes the graphics backend, and loads all assets.
    /// This function is designed to be called once when the app resumes.
    fn init_graphics(&mut self, event_loop: &ActiveEventLoop) -> Result<(), Box<dyn Error>> {
        let window_attributes = Window::default_attributes()
            .with_title(format!("Simple Renderer - {:?}", self.backend_type))
            .with_inner_size(PhysicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT))
            .with_resizable(true);

        let window = Arc::new(event_loop.create_window(window_attributes)?);
        let sz = window.inner_size();
        self.metrics = crate::core::space::metrics_for_window(sz.width, sz.height);

        // Backend creation no longer requires a temporary screen.
        let backend =
            create_backend(self.backend_type, window.clone(), self.vsync_enabled)?;
        self.window = Some(window);
        self.backend = Some(backend);

        // Assets can now be loaded directly.
        self.load_textures()?;
        self.load_fonts()?;

        // Now with fonts loaded, build the REAL initial screen.
        let (actors, clear_color) = self.get_current_actors();
        let initial_screen = self.build_screen(&actors, clear_color);
        if let Some(b) = &mut self.backend {
            renderer::load_screen(b, &initial_screen)?;
        }

        info!("Starting event loop...");
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            if let Err(e) = self.init_graphics(event_loop) {
                error!("Failed to initialize graphics: {}", e);
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        // Clone the Arc to avoid holding an immutable borrow of `self` while mutating `self`.
        let Some(window) = self.window.as_ref().cloned() else { return; };
        if window_id != window.id() {
            return;
        }

        match event {
            WindowEvent::CloseRequested => {
                info!("Close requested. Shutting down.");
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                info!("Window resized to: {}x{}", new_size.width, new_size.height);
                if new_size.width > 0 && new_size.height > 0 {
                    // keep metrics in sync
                    self.metrics = space::metrics_for_window(new_size.width, new_size.height);
                    if let Some(backend) = &mut self.backend {
                        renderer::resize(backend, new_size.width, new_size.height);
                    }
                }
            }
            WindowEvent::KeyboardInput { event: key_event, .. } => {
                input::handle_keyboard_input(&key_event, &mut self.input_state);

                let action = match self.current_screen {
                    CurrentScreen::Menu     => menu::handle_key_press(&mut self.menu_state, &key_event),
                    CurrentScreen::Gameplay => gameplay::handle_key_press(&mut self.gameplay_state, &key_event),
                    CurrentScreen::Options  => options::handle_key_press(&mut self.options_state, &key_event),
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

                let (actors, clear_color) = self.get_current_actors();
                let screen = self.build_screen(&actors, clear_color);

                // Update title/FPS without conflicting borrows.
                self.update_fps_title(&window, now);

                if let Some(backend) = &mut self.backend {
                    if let Err(e) = renderer::draw(backend, &screen, &self.texture_manager) {
                        error!("Failed to draw frame: {}", e);
                        event_loop.exit();
                    }
                }
            }
            _ => {}
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

// ---- public entry point ----
pub fn run() -> Result<(), Box<dyn Error>> {
    use log::info;

    let args: Vec<String> = std::env::args().collect();
    let (backend_type, vsync_enabled) = parse_args(&args);

    // Only consider the legacy flags for detection
    let backend_specified = args.iter().any(|a| a == "--opengl" || a == "--vulkan");
    if !backend_specified {
        info!("No backend specified. Defaulting to Vulkan.");
        info!("Use '--opengl' or '--vulkan' to select a backend.");
    }

    let event_loop = EventLoop::new()?;
    let mut app = App::new(backend_type, vsync_enabled);
    event_loop.run_app(&mut app)?;
    Ok(())
}
