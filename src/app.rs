// src/app.rs
use crate::core::input;
use crate::core::input::InputState;
use crate::core::gfx as renderer;
use crate::core::gfx::{create_backend, BackendType};
use crate::core::space::{self as space, Metrics};
use crate::ui::primitives as api;
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
    use log::warn;

    let mut backend = BackendType::Vulkan;
    let mut vsync = true;

    let mut it = args.iter().skip(1).peekable();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--opengl" => backend = BackendType::OpenGL,
            "--vulkan" => backend = BackendType::Vulkan,

            "--no-vsync" => vsync = false,

            "--vsync" => {
                if let Some(next) = it.peek() {
                    match next.as_str() {
                        "on" | "true" | "1" => { vsync = true;  it.next(); }
                        "off" | "false" | "0" => { vsync = false; it.next(); }
                        _ => vsync = true, // plain `--vsync` or unknown -> on
                    }
                } else {
                    vsync = true;
                }
            }

            s if s.starts_with("--vsync=") => {
                let v = &s["--vsync=".len()..];
                vsync = matches!(v, "on" | "true" | "1");
            }

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
        let texture_paths: [&'static str; 4] = [
            "logo.png",
            "dance.png",
            "meter_arrow.png",
            "fallback_banner.png",
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

        let mut decoded: Vec<(&'static str, image::RgbaImage)> = Vec::with_capacity(texture_paths.len());
        for h in handles {
            match h.join().expect("texture decode thread panicked") {
                Ok((key, rgba)) => decoded.push((key, rgba)),
                Err((key, msg)) => {
                    warn!("Failed to load 'assets/graphics/{}': {}. Using generated fallback.", key, msg);
                    decoded.push((key, fallback_rgba()));
                }
            }
        }

        // 2) Create GPU textures sequentially
        for (key, rgba) in decoded {
            let texture = if key == "logo.png" {
                // Explicitly exercise the Srgb variant (these UI sprites are authored in sRGB)
                renderer::create_texture_with_colorspace(
                    backend,
                    &rgba,
                    renderer::TextureColorSpace::Srgb,
                )?
            } else {
                renderer::create_texture(backend, &rgba)?
            };

            self.texture_manager.insert(key, texture);
            info!("Loaded texture: assets/graphics/{}", key);
        }
        Ok(())
    }

    fn load_fonts(&mut self) -> Result<(), Box<dyn Error>> {
        let backend = self.backend.as_mut().ok_or("Backend not initialized")?;

        // Read JSON + atlas for Wendy
        let json_wendy = std::fs::read("assets/fonts/wendy.json")?;
        // IMPORTANT: upload atlas as **linear** (no sRGB), and disable mipmaps if your API exposes it
        let img_wendy = image::open("assets/fonts/wendy.png")?.to_rgba8();
        let tex_wendy = renderer::create_texture_with_colorspace(
            backend,
            &img_wendy,
            renderer::TextureColorSpace::Linear,
        )?;
        self.texture_manager.insert("wendy.png", tex_wendy);

        // `px_range_hint` is only a fallback; real value comes from JSON distanceRange
        let font_wendy = msdf::load_font(&json_wendy, "wendy.png", /* px_range_hint */ 4.0);
        self.fonts.insert("wendy", font_wendy);

        // Read JSON + atlas for Miso
        let json_miso = std::fs::read("assets/fonts/miso.json")?;
        let img_miso = image::open("assets/fonts/miso.png")?.to_rgba8();
        let tex_miso = renderer::create_texture_with_colorspace(
            backend,
            &img_miso,
            renderer::TextureColorSpace::Linear,
        )?;
        self.texture_manager.insert("miso.png", tex_miso);
        let font_miso = msdf::load_font(&json_miso, "miso.png", 4.0);
        self.fonts.insert("miso", font_miso);

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
                let new_screen_data = create_screen_from_ui(&ui_elements, clear_color, &self.fonts);
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

    fn build_screen(&self, elements: &[api::UIElement], clear_color: [f32;4]) -> renderer::Screen {
        crate::ui::build::build_screen(elements, clear_color, &self.fonts)
    }

    fn get_current_ui_elements(&self) -> (Vec<api::UIElement>, [f32; 4]) {
        match self.current_screen {
            CurrentScreen::Menu => (
                menu::get_ui_elements(&self.menu_state, &self.metrics, &self.fonts),
                [0.03, 0.03, 0.03, 1.0],
            ),
            CurrentScreen::Gameplay => (
                gameplay::get_ui_elements(&self.gameplay_state, &self.metrics),
                [0.03, 0.03, 0.03, 1.0],
            ),
            CurrentScreen::Options => (
                options::get_ui_elements(&self.options_state, &self.metrics, &self.fonts),
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
                    let sz = window.inner_size();
                    self.metrics = crate::core::space::metrics_for_window(sz.width, sz.height);

                    // Pre-load fonts before building the initial screen.
                    // This is a temporary move; ideally, backend creation doesn't need a screen.
                    // But first, we need the backend to load the font atlas texture.
                    let temp_screen = self.build_screen(&[], [0.0; 4]); // Dummy screen

                    match create_backend(self.backend_type, window.clone(), &temp_screen, self.vsync_enabled) {
                        Ok(backend) => {
                            self.window = Some(window.clone());
                            self.backend = Some(backend);
                            if let Err(e) = self.load_textures() {
                                error!("Failed to load textures: {}", e);
                                event_loop.exit();
                                return;
                            }
                            if let Err(e) = self.load_fonts() {
                                error!("Failed to load fonts: {}", e);
                                event_loop.exit();
                                return;
                            }

                            // Now with fonts loaded, build the REAL initial screen.
                            let (ui_elements, clear_color) = self.get_current_ui_elements();
                            let initial_screen = self.build_screen(&ui_elements, clear_color);
                            if let Some(b) = &mut self.backend {
                                if let Err(e) = renderer::load_screen(b, &initial_screen) {
                                    error!("Failed to load initial screen data: {}", e);
                                    event_loop.exit();
                                    return;
                                }
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
                            // +++ keep metrics in sync
                            self.metrics = space::metrics_for_window(new_size.width, new_size.height);
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
                        let screen = self.build_screen(&ui_elements, clear_color);

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
#[inline(always)]
fn create_screen_from_ui(
    elements: &[api::UIElement],
    clear_color: [f32; 4],
    fonts: &HashMap<&'static str, msdf::Font>,
) -> renderer::Screen {
    crate::ui::build::build_screen(elements, clear_color, fonts)
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
