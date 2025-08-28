use crate::core::gfx::{self as renderer, create_backend, BackendType, RenderList};
use crate::core::input;
use crate::core::input::InputState;
use crate::core::space::{self as space, Metrics};
use crate::ui::actors::Actor;
use crate::ui::msdf;
use crate::ui::color;
use crate::screens::{gameplay, menu, options, init, select_color, Screen as CurrentScreen, ScreenAction};

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

/* -------------------- transition timing constants -------------------- */
// global full-screen fade (kept as default for non-Init→Menu)
const FADE_OUT_DELAY: f32 = 0.1;
const FADE_OUT_DURATION: f32 = 0.4;
const FADE_IN_DURATION: f32 = 0.4;

// menu actors fade duration (only used after the special Init→Menu squish)
const MENU_ACTORS_FADE_DURATION: f32 = 0.65;

// special Init→Menu: center bar collapse
const BAR_SQUISH_DURATION: f32 = 0.35;

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
                        vsync = true;
                    }
                } else {
                    vsync = true;
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
                        fullscreen = true;
                    }
                } else {
                    fullscreen = true;
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

/* -------------------- transition state machine -------------------- */
#[derive(Clone, Copy, Debug)]
enum TransitionState {
    Idle,

    // Generic global fade — for everything EXCEPT Init→Menu
    FadingOut { elapsed: f32, target: CurrentScreen },
    FadingIn  { elapsed: f32 },

    // Special Init→Menu: squish the bar while still on Init
    BarSquishOut { elapsed: f32, target: CurrentScreen },

    // Then, on Menu, fade ONLY the Menu actors in (no global overlay)
    ActorsFadeIn { elapsed: f32 },
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
    start_time: Instant,
    vsync_enabled: bool,
    fullscreen_enabled: bool,
    fonts: HashMap<&'static str, msdf::Font>,
    metrics: Metrics,
    last_fps: f32,
    last_vpf: u32,
    show_overlay: bool,
    transition: TransitionState,
    init_state: init::State,
    select_color_state: select_color::State,
}

impl App {
    fn new(backend_type: BackendType, vsync_enabled: bool, fullscreen_enabled: bool) -> Self {
        Self {
            window: None, backend: None, backend_type, texture_manager: HashMap::new(),
            current_screen: CurrentScreen::Init, init_state: init::init(), menu_state: menu::init(), gameplay_state: gameplay::init(), options_state: options::init(),
            select_color_state: select_color::init(), input_state: input::init_state(), frame_count: 0, last_title_update: Instant::now(), last_frame_time: Instant::now(),
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

        // 1x1 white solid
        {
            let white = image::RgbaImage::from_raw(1, 1, vec![255, 255, 255, 255]).unwrap();
            let white_tex = renderer::create_texture(
                backend,
                &white,
                renderer::TextureColorSpace::Srgb,
            )?;
            self.texture_manager.insert("__white", white_tex);
            info!("Loaded built-in texture: __white");
        }

        let texture_paths: [&'static str; 6] = [
            "logo.png",
            "init_arrow.png",
            "dance.png",
            "meter_arrow.png",
            "fallback_banner.png",
            "heart.png",
        ];

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

        let fallback_image = std::sync::Arc::new(fallback_rgba());
        let mut decoded: Vec<(&'static str, std::sync::Arc<image::RgbaImage>)> = Vec::with_capacity(texture_paths.len());

        for h in handles {
            match h.join().expect("texture decode thread panicked") {
                Ok((key, rgba)) => decoded.push((key, std::sync::Arc::new(rgba))),
                Err((key, msg)) => {
                    warn!("Failed to load 'assets/graphics/{}': {}. Using fallback.", key, msg);
                    decoded.push((key, fallback_image.clone()));
                }
            }
        }

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

        let json_data  = std::fs::read(&json_path)?;
        let image_data = image::open(&png_path)?.to_rgba8();

        let texture = renderer::create_texture(
            backend,
            &image_data,
            renderer::TextureColorSpace::Linear,
        )?;
        self.texture_manager.insert(name, texture);

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
                if matches!(self.transition, TransitionState::Idle) {
                    if self.current_screen == CurrentScreen::Init && screen == CurrentScreen::Menu {
                        // SPECIAL: Init → Menu = bar squish + menu actors fade-in
                        info!("Starting special Init→Menu transition (bar squish; no global fade)");
                        self.transition = TransitionState::BarSquishOut { elapsed: 0.0, target: screen };
                    } else {
                        // Generic: use global black overlay fade
                        info!("Starting global fade out to screen: {:?}", screen);
                        self.transition = TransitionState::FadingOut { elapsed: 0.0, target: screen };
                    }
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
        crate::ui::compose::build_screen(actors, clear_color, &self.metrics, &self.fonts, total_elapsed)
    }

    fn get_current_actors(&self) -> (Vec<Actor>, [f32; 4]) {
        const CLEAR: [f32; 4] = [0.03, 0.03, 0.03, 1.0];

        // Menu fade-in only
        let mut screen_alpha_multiplier = 1.0;
        if let TransitionState::ActorsFadeIn { elapsed } = self.transition {
            if self.current_screen == CurrentScreen::Menu {
                screen_alpha_multiplier = (elapsed / MENU_ACTORS_FADE_DURATION).clamp(0.0, 1.0);
            }
        }

        // ⬇️ key change is inside the Init branch
        let mut actors = match self.current_screen {
            CurrentScreen::Menu     => menu::get_actors(&self.menu_state, screen_alpha_multiplier),
            CurrentScreen::Gameplay => gameplay::get_actors(&self.gameplay_state),
            CurrentScreen::Options  => options::get_actors(&self.options_state),
            CurrentScreen::SelectColor => select_color::get_actors(&self.select_color_state),
            CurrentScreen::Init     => {
                // During the squish phase, draw ONLY background (no original bar)
                if matches!(self.transition, TransitionState::BarSquishOut { .. }) {
                    init::get_actors_bg_only(&self.init_state)
                } else {
                    init::get_actors(&self.init_state)
                }
            }
        };

        if self.show_overlay {
            let overlay = crate::ui::components::stats_overlay::build(
                self.backend_type,
                self.last_fps,
                self.last_vpf,
            );
            actors.extend(overlay);
        }

        // No global overlay for the special squish flow
        let overlay_alpha = match self.transition {
            TransitionState::FadingOut { elapsed, .. } => ((elapsed - FADE_OUT_DELAY) / FADE_OUT_DURATION).clamp(0.0, 1.0),
            TransitionState::FadingIn  { elapsed, .. } => 1.0 - (elapsed / FADE_IN_DURATION).clamp(0.0, 1.0),
            _ => 0.0,
        };

        if overlay_alpha > 0.0 {
            actors.push(crate::ui::components::fade::black(overlay_alpha));
        }

        // Special squish bar: put it ABOVE the hearts so it’s visible,
        // and since we suppressed the original bar, this is the only bar drawn.
        if let TransitionState::BarSquishOut { elapsed, .. } = self.transition {
            let t = (elapsed / BAR_SQUISH_DURATION).clamp(0.0, 1.0);
            actors.push(crate::screens::init::build_squish_bar(t));
        }

        (actors, CLEAR)
    }

    #[inline(always)]
    fn update_fps_title(&mut self, window: &Window, now: Instant) {
        self.frame_count += 1;
        let elapsed = now.duration_since(self.last_title_update);
        if elapsed.as_secs_f32() >= 1.0 {
            let fps = self.frame_count as f32 / elapsed.as_secs_f32();
            self.last_fps = fps;

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
                    log::warn!(
                        "No exact EXCLUSIVE mode {}x{}; using BORDERLESS.",
                        WINDOW_WIDTH,
                        WINDOW_HEIGHT
                    );
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
        let Some(window) = self.window.as_ref().cloned() else { return; };
        if window_id != window.id() {
            return;
        }

        let is_transitioning = !matches!(self.transition, TransitionState::Idle);

        match event {
            WindowEvent::CloseRequested => {
                info!("Close requested. Shutting down.");
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if new_size.width > 0 && new_size.height > 0 {
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

                    // Only exit directly from the menu.
                    if let winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Escape) = key_event.physical_key {
                        if self.current_screen == CurrentScreen::Menu {
                            if let Err(e) = self.handle_action(ScreenAction::Exit, event_loop) {
                                error!("Failed to handle exit action: {}", e);
                                event_loop.exit();
                            }
                            return;
                        }
                    }
                }

                if is_transitioning { return; }

                let action = match self.current_screen {
                    CurrentScreen::Menu     => menu::handle_key_press(&mut self.menu_state, &key_event),
                    CurrentScreen::Gameplay => gameplay::handle_key_press(&mut self.gameplay_state, &key_event),
                    CurrentScreen::Options  => options::handle_key_press(&mut self.options_state, &key_event),
                    CurrentScreen::SelectColor => select_color::handle_key_press(&mut self.select_color_state, &key_event),
                    CurrentScreen::Init     => init::handle_key_press(&mut self.init_state, &key_event),
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

                match &mut self.transition {
                    // Generic overlay fade
                    TransitionState::FadingOut { elapsed, target } => {
                        *elapsed += delta_time;
                        if *elapsed >= FADE_OUT_DELAY + FADE_OUT_DURATION {
                            let prev = self.current_screen;
                            self.current_screen = *target;

                            // When leaving SelectColor -> Gameplay, apply chosen DECORATIVE color
                            if prev == CurrentScreen::SelectColor && *target == CurrentScreen::Gameplay {
                                let idx = self.select_color_state.active_color_index;
                                self.gameplay_state.player_color = color::decorative_rgba(idx);
                            }

                            // When returning SelectColor -> Menu, keep Menu’s active index in sync
                            if prev == CurrentScreen::SelectColor && *target == CurrentScreen::Menu {
                                self.menu_state.active_color_index = self.select_color_state.active_color_index;
                            }

                            self.transition = TransitionState::FadingIn { elapsed: 0.0 };
                            crate::ui::runtime::clear_all();
                        }
                    }
                    TransitionState::FadingIn { elapsed } => {
                        *elapsed += delta_time;
                        if *elapsed >= FADE_IN_DURATION {
                            self.transition = TransitionState::Idle;
                        }
                    }

                    // Special flow
                    TransitionState::BarSquishOut { elapsed, target } => {
                        *elapsed += delta_time;
                        if *elapsed >= BAR_SQUISH_DURATION {
                            self.current_screen = *target;
                            // (No SelectColor involved in this special init→menu path.)
                            self.transition = TransitionState::ActorsFadeIn { elapsed: 0.0 };
                            crate::ui::runtime::clear_all();
                        }
                    }
                    TransitionState::ActorsFadeIn { elapsed } => {
                        *elapsed += delta_time;
                        if *elapsed >= MENU_ACTORS_FADE_DURATION {
                            self.transition = TransitionState::Idle;
                        }
                    }

                    // Idle → run per-screen logic
                    TransitionState::Idle => {
                        match self.current_screen {
                            CurrentScreen::Gameplay => {
                                gameplay::update(&mut self.gameplay_state, &self.input_state, delta_time);
                            }
                            CurrentScreen::Init => {
                                let action = init::update(&mut self.init_state, delta_time);
                                if let ScreenAction::Navigate(_) | ScreenAction::Exit = action {
                                    if self.handle_action(action, event_loop).is_err() { /* ... */ }
                                }
                            }
                            CurrentScreen::SelectColor => {
                                select_color::update(&mut self.select_color_state, delta_time);  // ⬅ this is the whole point
                            }
                            _ => {}
                        }
                    }
                }

                let (actors, clear_color) = self.get_current_actors();
                let screen = self.build_screen(&actors, clear_color, total_elapsed);

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
        if let Some(backend) = &mut self.backend {
            renderer::dispose_textures(backend, &mut self.texture_manager);
            renderer::cleanup(backend);
        }
    }
}

// ---- public entry point ----
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .try_init();

    let args: Vec<String> = std::env::args().collect();
    let (backend_type, vsync_enabled, fullscreen_enabled) = parse_args(&args);

    let event_loop = EventLoop::new()?;
    let mut app = App::new(backend_type, vsync_enabled, fullscreen_enabled);
    event_loop.run_app(&mut app)?;
    Ok(())
}