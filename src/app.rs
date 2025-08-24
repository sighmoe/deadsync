use crate::core::gfx::{self as renderer, create_backend, BackendType, RenderList};
use crate::core::input;
use crate::core::input::InputState;
use crate::core::space::{self as space, Metrics};
use crate::ui::actors::Actor;
use crate::ui::msdf;
use crate::screens::{gameplay, menu, options, Screen as CurrentScreen, ScreenAction};
use crate::act;

use log::{error, info, warn};
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

// --- NEW: Transition timing constants ---
const FADE_OUT_DELAY: f32 = 0.1;    // Time before the black overlay starts fading in
const FADE_OUT_DURATION: f32 = 0.4; // Time for the black overlay to fade to full
const FADE_IN_DURATION: f32 = 0.4;  // Time for the black overlay to fade out on the new screen
const MENU_ACTORS_FADE_DURATION: f32 = 0.65; // Time for menu's own actors to fade

// ---- args ----
fn parse_args(args: &[String]) -> (BackendType, bool, bool) {
    #[inline(always)]
    fn parse_bool_token(s: &str) -> Option<bool> {
        match s {
            "on" | "true" | "1"  => Some(true),
            "off" | "false" | "0" => Some(false),
            _ => None,
        }
    }

    let mut backend    = BackendType::Vulkan;
    let mut vsync      = true;
    let mut fullscreen = false;

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

            "--fullscreen" => {
                if i + 1 < args.len() {
                    if let Some(v) = parse_bool_token(args[i + 1].as_str()) {
                        fullscreen = v;
                        i += 1;
                    } else {
                        fullscreen = true; // plain `--fullscreen`
                    }
                } else {
                    fullscreen = true;     // plain `--fullscreen`
                }
            }
            "--windowed" => fullscreen = false,
            _ if a.starts_with("--fullscreen=") => {
                let v = &a["--fullscreen=".len()..];
                fullscreen = parse_bool_token(v).unwrap_or(true);
            }

            _ => {}
        }
        i += 1;
    }
    (backend, vsync, fullscreen)
}

#[derive(Clone, Copy, Debug)]
enum TransitionState {
    Idle,
    // Fading out the OLD screen
    FadingOut { elapsed: f32, target: CurrentScreen },
    // Fading in the NEW screen
    FadingIn { elapsed: f32 },
}

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
    start_time: Instant, // NEW
    vsync_enabled: bool,
    fullscreen_enabled: bool,
    fonts: HashMap<&'static str, msdf::Font>,
    metrics: Metrics,
    last_fps: f32,
    last_vpf: u32,
    show_overlay: bool,
    transition: TransitionState, // Add this new field
}

impl App {
    fn new(backend_type: BackendType, vsync_enabled: bool, fullscreen_enabled: bool) -> Self {
        Self {
            window: None, backend: None, backend_type, texture_manager: HashMap::new(),
            current_screen: CurrentScreen::Menu, menu_state: menu::init(), gameplay_state: gameplay::init(), options_state: options::init(),
            input_state: input::init_state(), frame_count: 0, last_title_update: Instant::now(), last_frame_time: Instant::now(),
            start_time: Instant::now(), metrics: space::metrics_for_window(WINDOW_WIDTH, WINDOW_HEIGHT),
            vsync_enabled, fullscreen_enabled, fonts: HashMap::new(), show_overlay: false,
            last_fps: 0.0, last_vpf: 0, transition: TransitionState::Idle,
        }
    }

    fn load_textures(&mut self) -> Result<(), Box<dyn Error>> {
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

        // ---- 0) built-in 1x1 white texture for "solid quads" ----
        {
            let white = image::RgbaImage::from_raw(1, 1, vec![255, 255, 255, 255]).unwrap();
            let white_tex = renderer::create_texture(
                backend,
                &white,
                renderer::TextureColorSpace::Srgb,
            )?;
            // reserved key for solids:
            self.texture_manager.insert("__white", white_tex);
            info!("Loaded built-in texture: __white (1x1 white)");
        }

        // Logical IDs -> filenames
        let texture_paths: [&'static str; 5] = [
            "logo.png",
            "dance.png",
            "meter_arrow.png",
            "fallback_banner.png",
            "heart.png",
        ];

        // 1) Decode images in parallel (CPU-only work)
        let mut handles = Vec::with_capacity(texture_paths.len());
        for &key in &texture_paths {
            let path = Path::new("assets/graphics").join(key);
            handles.push(std::thread::spawn(move || {
                match image::open(&path) {
                    Ok(img) => Ok::<(&'static str, image::RgbaImage), (&'static str, String)>((key, img.to_rgba8())),
                    Err(e) => Err((key, e.to_string())),
                }
            }));
        }

        // Create the fallback image once and wrap in an Arc for cheap cloning.
        let fallback_image = Arc::new(fallback_rgba());
        let mut decoded: Vec<(&'static str, Arc<image::RgbaImage>)> = Vec::with_capacity(texture_paths.len());

        for h in handles {
            match h.join().expect("texture decode thread panicked") {
                Ok((key, rgba)) => decoded.push((key, Arc::new(rgba))),
                Err((key, msg)) => {
                    warn!("Failed to load 'assets/graphics/{}': {}. Using generated fallback.", key, msg);
                    decoded.push((key, fallback_image.clone()));
                }
            }
        }

        // 2) Create GPU textures sequentially
        for (key, rgba_arc) in decoded {
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
        for &name in &["wendy", "miso"] {
            self.load_font_asset(name)?;
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
                // Instead of navigating directly, start the fade-out process
                if matches!(self.transition, TransitionState::Idle) {
                    info!("Starting fade out to screen: {:?}", screen);
                    self.transition = TransitionState::FadingOut { elapsed: 0.0, target: screen };
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

    fn build_screen(&self, actors: &[Actor], clear_color: [f32; 4], total_elapsed: f32) -> RenderList {
        crate::ui::layout::build_screen(actors, clear_color, &self.metrics, &self.fonts, total_elapsed)
    }

    // `get_current_actors` now calculates fade alphas and adds the global overlay
    fn get_current_actors(&self) -> (Vec<Actor>, [f32; 4]) {
        const CLEAR: [f32; 4] = [0.03, 0.03, 0.03, 1.0];

        // Determine the alpha multiplier for the screen's own actors
        let screen_alpha_multiplier = match self.transition {
            TransitionState::FadingOut { elapsed, .. } => {
                (1.0 - (elapsed / MENU_ACTORS_FADE_DURATION)).clamp(0.0, 1.0)
            }
            _ => 1.0,
        };

        let mut actors = match self.current_screen {
            CurrentScreen::Menu => menu::get_actors(&self.menu_state, &self.metrics, screen_alpha_multiplier),
            CurrentScreen::Gameplay => gameplay::get_actors(&self.gameplay_state, &self.metrics),
            CurrentScreen::Options  => options::get_actors(&self.options_state, &self.metrics),
        };

        if self.show_overlay {
            let overlay = crate::ui::components::stats_overlay::build(
                self.backend_type,
                self.last_fps,
                self.last_vpf,
            );
            actors.extend(overlay);
        }

        // Add the global black fade overlay if needed
        let overlay_alpha = match self.transition {
            TransitionState::FadingOut { elapsed, .. } => {
                ((elapsed - FADE_OUT_DELAY) / FADE_OUT_DURATION).clamp(0.0, 1.0)
            }
            TransitionState::FadingIn { elapsed, .. } => {
                1.0 - (elapsed / FADE_IN_DURATION).clamp(0.0, 1.0)
            }
            TransitionState::Idle => 0.0,
        };

        if overlay_alpha > 0.0 {
            let w = self.metrics.right - self.metrics.left;
            let h = self.metrics.top - self.metrics.bottom;
            actors.push(act!(quad:
                align(0.0, 0.0):
                xy(0.0, 0.0):
                zoomto(w, h):
                diffuse(0.0, 0.0, 0.0, overlay_alpha):
                z(500) // Ensure it's drawn on top of everything
            ));
        }

        (actors, CLEAR)
    }

    #[inline(always)]
    fn update_fps_title(&mut self, window: &Window, now: Instant) {
        self.frame_count += 1;
        let elapsed = now.duration_since(self.last_title_update);
        if elapsed.as_secs_f32() >= 1.0 {
            let fps = self.frame_count as f32 / elapsed.as_secs_f32();
            self.last_fps = fps; // cache for overlay

            let screen_name = format!("{:?}", self.current_screen);
            window.set_title(&format!(
                "Simple Renderer - {:?} | {} | {:.2} FPS",
                self.backend_type, screen_name, fps
            ));
            self.frame_count = 0;
            self.last_title_update = now;
        }
    }

        fn init_graphics(&mut self, event_loop: &ActiveEventLoop) -> Result<(), Box<dyn Error>> {
        let mut window_attributes = Window::default_attributes()
            .with_title(format!("Simple Renderer - {:?}", self.backend_type))
            .with_resizable(true);

        if self.fullscreen_enabled {
            let fullscreen = if let Some(mon) = event_loop.primary_monitor() {
                let best_mode = mon.video_modes()
                    .filter(|m| { let sz = m.size(); sz.width == WINDOW_WIDTH && sz.height == WINDOW_HEIGHT })
                    .max_by_key(|m| m.refresh_rate_millihertz());
                if let Some(mode) = best_mode {
                    log::info!("Fullscreen: using EXCLUSIVE {}x{} @ {} mHz", WINDOW_WIDTH, WINDOW_HEIGHT, mode.refresh_rate_millihertz());
                    Some(winit::window::Fullscreen::Exclusive(mode))
                } else {
                    log::warn!("No exact EXCLUSIVE mode {}x{}; using BORDERLESS on primary monitor.", WINDOW_WIDTH, WINDOW_HEIGHT);
                    Some(winit::window::Fullscreen::Borderless(Some(mon)))
                }
            } else {
                log::warn!("No primary monitor reported; using BORDERLESS fullscreen.");
                Some(winit::window::Fullscreen::Borderless(None))
            };
            window_attributes = window_attributes.with_fullscreen(fullscreen);
        } else {
            window_attributes = window_attributes.with_inner_size(PhysicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));
        }

        let window = Arc::new(event_loop.create_window(window_attributes)?);
        let sz = window.inner_size();
        self.metrics = crate::core::space::metrics_for_window(sz.width, sz.height);
        crate::core::space::set_current_metrics(self.metrics);

        let backend = create_backend(self.backend_type, window.clone(), self.vsync_enabled)?;
        self.window = Some(window);
        self.backend = Some(backend);

        self.load_textures()?;
        self.load_fonts()?;

        // The initial screen is drawn on the first RedrawRequested event,
        // so no need to explicitly draw or load here.
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

        // Disable most input while transitioning
        let is_transitioning = !matches!(self.transition, TransitionState::Idle);

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

                    space::set_current_metrics(self.metrics);

                    if let Some(backend) = &mut self.backend {
                        renderer::resize(backend, new_size.width, new_size.height);
                    }
                }
            }
            WindowEvent::KeyboardInput { event: key_event, .. } => {
                input::handle_keyboard_input(&key_event, &mut self.input_state);
                
                if key_event.state == winit::event::ElementState::Pressed {
                    if let winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::F3) = key_event.physical_key {
                        self.show_overlay = !self.show_overlay;
                        info!("Overlay {}", if self.show_overlay { "ON" } else { "OFF" });
                    }
                    
                    // --- FIX: Make the global Escape handler context-aware ---
                    if let winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Escape) = key_event.physical_key {
                        // Only exit directly from the menu. Other screens will handle this
                        // key press locally to navigate back to the menu.
                        if self.current_screen == CurrentScreen::Menu {
                            if let Err(e) = self.handle_action(ScreenAction::Exit, event_loop) {
                                error!("Failed to handle exit action: {}", e);
                                event_loop.exit();
                            }
                            // Return early to avoid menu's own handler processing Exit again
                            return;
                        }
                    }
                }
                
                // Block other input while fading
                if is_transitioning { return; }

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
                let total_elapsed = now.duration_since(self.start_time).as_secs_f32();

                crate::ui::runtime::tick(delta_time);

                // Update transition state machine
                match &mut self.transition {
                    TransitionState::FadingOut { elapsed, target } => {
                        *elapsed += delta_time;
                        // Total fade out time is delay + duration
                        if *elapsed >= FADE_OUT_DELAY + FADE_OUT_DURATION {
                            self.current_screen = *target;
                            self.transition = TransitionState::FadingIn { elapsed: 0.0 };
                            // Clear tweens for the new screen
                            crate::ui::runtime::clear_all(); 
                        }
                    }
                    TransitionState::FadingIn { elapsed } => {
                        *elapsed += delta_time;
                        if *elapsed >= FADE_IN_DURATION {
                            self.transition = TransitionState::Idle;
                        }
                    }
                    TransitionState::Idle => {
                        // Only run game logic when not transitioning
                        if self.current_screen == CurrentScreen::Gameplay {
                        gameplay::update(&mut self.gameplay_state, &self.input_state, delta_time);
                        }
                    }
                }

                let (actors, clear_color) = self.get_current_actors();
                let screen = self.build_screen(&actors, clear_color, total_elapsed);

                // Update title/FPS without conflicting borrows.
                self.update_fps_title(&window, now);

                if let Some(backend) = &mut self.backend {
                    match renderer::draw(backend, &screen, &self.texture_manager) {
                        Ok(vpf) => self.last_vpf = vpf,
                        Err(e) => {
                            error!("Failed to draw frame: {}", e);
                            event_loop.exit();
                        }
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
    let (backend_type, vsync_enabled, fullscreen_enabled) = parse_args(&args);

    // Only consider the legacy flags for detection
    let backend_specified = args.iter().any(|a| a == "--opengl" || a == "--vulkan");
    if !backend_specified {
        info!("No backend specified. Defaulting to Vulkan.");
        info!("Use '--opengl' or '--vulkan' to select a backend.");
    }

    if fullscreen_enabled {
        info!("Fullscreen enabled (try exclusive at {}x{}).", WINDOW_WIDTH, WINDOW_HEIGHT);
        info!("Use '--windowed' or '--fullscreen=false' to disable.");
    }

    let event_loop = EventLoop::new()?;
    let mut app = App::new(backend_type, vsync_enabled, fullscreen_enabled);
    event_loop.run_app(&mut app)?;
    Ok(())
}
