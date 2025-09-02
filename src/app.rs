use crate::core::gfx::{self as renderer, create_backend, BackendType, RenderList};
use crate::core::input;
use crate::core::input::InputState;
use crate::core::space::{self as space, Metrics};
use crate::ui::actors::Actor;
use crate::ui::msdf;
use crate::ui::color;
use crate::screens::{gameplay, menu, options, init, select_color, select_music, Screen as CurrentScreen, ScreenAction};
use crate::core::song_loading::{self, ChartData};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::Window,
};

use log::{error, info, warn};
use image;
use std::{collections::HashMap, error::Error, path::{Path, PathBuf}, sync::Arc, time::Instant};

/* -------------------- transition timing constants -------------------- */
const FADE_OUT_DELAY: f32 = 0.1;
const FADE_OUT_DURATION: f32 = 0.4;
const FADE_IN_DURATION: f32 = 0.4;
const MENU_ACTORS_FADE_DURATION: f32 = 0.65;
const BAR_SQUISH_DURATION: f32 = 0.35;

/* -------------------- transition state machine -------------------- */
#[derive(Clone, Copy, Debug)]
enum TransitionState {
    Idle,
    FadingOut { elapsed: f32, target: CurrentScreen },
    FadingIn  { elapsed: f32 },
    BarSquishOut { elapsed: f32, target: CurrentScreen },
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
    select_music_state: select_music::State,
    current_dynamic_banner: Option<(&'static str, PathBuf)>,
    current_density_graph: Option<(&'static str, String)>,
    display_width: u32,
    display_height: u32,
}

impl App {
    fn new(
        backend_type: BackendType,
        vsync_enabled: bool,
        fullscreen_enabled: bool,
        show_overlay: bool,
        color_index: i32,
    ) -> Self {
        let config = crate::config::get();
        let display_width = config.display_width;
        let display_height = config.display_height;

        let mut menu_state = menu::init();
        menu_state.active_color_index = color_index;

        let mut select_color_state = select_color::init();
        select_color_state.active_color_index = color_index;
        select_color_state.scroll = color_index as f32;
        select_color_state.bg_from_index = color_index;
        select_color_state.bg_to_index = color_index;

        let mut options_state = options::init(); // <-- ADDED
        options_state.active_color_index = color_index; // <-- ADDED

        Self {
            window: None, backend: None, backend_type, texture_manager: HashMap::new(),
            current_screen: CurrentScreen::Init, init_state: init::init(), menu_state, gameplay_state: gameplay::init(), options_state,
            select_color_state, select_music_state: select_music::init(), input_state: input::init_state(), frame_count: 0, last_title_update: Instant::now(), last_frame_time: Instant::now(),
            start_time: Instant::now(), metrics: space::metrics_for_window(display_width, display_height),
            vsync_enabled, fullscreen_enabled, fonts: HashMap::new(), show_overlay,
            last_fps: 0.0, last_vpf: 0, transition: TransitionState::Idle,
            current_dynamic_banner: None,
            current_density_graph: None,
            display_width,
            display_height,
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

        {
            let white = image::RgbaImage::from_raw(1, 1, vec![255, 255, 255, 255]).unwrap();
            let white_tex = renderer::create_texture(backend, &white, renderer::TextureColorSpace::Srgb)?;
            self.texture_manager.insert("__white", white_tex);
            info!("Loaded built-in texture: __white");
        }

        let textures_to_load: Vec<(&'static str, &'static str)> = vec![
            ("logo.png", "logo.png"), ("init_arrow.png", "init_arrow.png"),
            ("dance.png", "dance.png"), ("meter_arrow.png", "meter_arrow.png"),
            ("heart.png", "heart.png"), ("banner1.png", "_fallback/banner1.png"),
            ("banner2.png", "_fallback/banner2.png"), ("banner3.png", "_fallback/banner3.png"),
            ("banner4.png", "_fallback/banner4.png"), ("banner5.png", "_fallback/banner5.png"),
            ("banner6.png", "_fallback/banner6.png"), ("banner7.png", "_fallback/banner7.png"),
            ("banner8.png", "_fallback/banner8.png"), ("banner9.png", "_fallback/banner9.png"),
            ("banner10.png", "_fallback/banner10.png"), ("banner11.png", "_fallback/banner11.png"),
            ("banner12.png", "_fallback/banner12.png"),
        ];

        let mut handles = Vec::with_capacity(textures_to_load.len());
        for &(key, relative_path) in &textures_to_load {
            let path = Path::new("assets/graphics").join(relative_path);
            handles.push(std::thread::spawn(move || {
                match image::open(&path) {
                    Ok(img) => Ok::<(&'static str, image::RgbaImage), (&'static str, String)>((key, img.to_rgba8())),
                    Err(e) => Err((key, e.to_string())),
                }
            }));
        }

        let fallback_image = std::sync::Arc::new(fallback_rgba());
        for h in handles {
            match h.join().expect("texture decode thread panicked") {
                Ok((key, rgba)) => {
                    let texture = renderer::create_texture(backend, &rgba, renderer::TextureColorSpace::Srgb)?;
                    self.texture_manager.insert(key, texture);
                    info!("Loaded texture: {}", key);
                }
                Err((key, msg)) => {
                    warn!("Failed to load texture for key '{}': {}. Using fallback.", key, msg);
                    let texture = renderer::create_texture(backend, &fallback_image, renderer::TextureColorSpace::Srgb)?;
                    self.texture_manager.insert(key, texture);
                }
            }
        }
        Ok(())
    }

    fn load_font_asset(&mut self, name: &'static str) -> Result<(), Box<dyn Error>> {
        let backend = self.backend.as_mut().ok_or("Backend not initialized")?;
        let json_path = format!("assets/fonts/{}.json", name);
        let png_path  = format!("assets/fonts/{}.png",  name);

        let json_data  = std::fs::read(&json_path)?;
        let image_data = image::open(&png_path)?.to_rgba8();

        let texture = renderer::create_texture(backend, &image_data, renderer::TextureColorSpace::Linear)?;
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

    fn destroy_current_dynamic_banner(&mut self) {
        if let Some((key, _path)) = self.current_dynamic_banner.take() {
            if let Some(backend) = self.backend.as_mut() {
                if let renderer::Backend::Vulkan(vk_state) = backend {
                    if let Some(device) = &vk_state.device {
                        unsafe { let _ = device.device_wait_idle(); }
                    }
                }
            }
            self.texture_manager.remove(key);
            unsafe {
                let _ = Box::from_raw(key as *const str as *mut str);
            }
        }
    }

    fn destroy_current_density_graph(&mut self) {
        if let Some((key, _hash)) = self.current_density_graph.take() {
            if let Some(backend) = self.backend.as_mut() {
                if let renderer::Backend::Vulkan(vk_state) = backend {
                    if let Some(device) = &vk_state.device {
                        unsafe { let _ = device.device_wait_idle(); }
                    }
                }
            }
            self.texture_manager.remove(key);
            unsafe {
                let _ = Box::from_raw(key as *const str as *mut str);
            }
        }
    }

    fn set_dynamic_banner(&mut self, path_opt: Option<PathBuf>) -> &'static str {
        if let Some(path) = path_opt {
            if self.current_dynamic_banner.as_ref().map_or(false, |(_, p)| p == &path) {
                return self.current_dynamic_banner.as_ref().unwrap().0;
            }

            self.destroy_current_dynamic_banner();

            let backend = match self.backend.as_mut() {
                Some(b) => b,
                None => return "banner1.png",
            };

            match image::open(&path) {
                Ok(img) => {
                    let rgba = img.to_rgba8();
                    match renderer::create_texture(backend, &rgba, renderer::TextureColorSpace::Srgb) {
                        Ok(texture) => {
                            let key: &'static str = Box::leak(path.to_string_lossy().into_owned().into_boxed_str());
                            self.texture_manager.insert(key, texture);
                            self.current_dynamic_banner = Some((key, path));
                            key
                        }
                        Err(e) => {
                            warn!("Failed to create GPU texture for {:?}: {}. Using fallback.", path, e);
                            "banner1.png"
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to open banner image {:?}: {}. Using fallback.", path, e);
                    "banner1.png"
                }
            }
        } else {
            self.destroy_current_dynamic_banner();
            "banner1.png"
        }
    }


    fn set_density_graph(&mut self, chart_opt: Option<&ChartData>) -> &'static str {
        const FALLBACK_KEY: &'static str = "__white";

        if let Some(chart) = chart_opt {
            // If the graph for this chart's hash is already the active one, do nothing.
            if self.current_density_graph.as_ref().map_or(false, |(_, h)| h == &chart.short_hash) {
                return self.current_density_graph.as_ref().unwrap().0;
            }

            // It's a new chart, so destroy the old graph texture.
            self.destroy_current_density_graph();
            
            if let Some(graph_data) = &chart.density_graph {
                let backend = match self.backend.as_mut() {
                    Some(b) => b,
                    None => return FALLBACK_KEY,
                };

                // This is where the engine takes on the dependency to create an image object.
                let rgba_image = match image::RgbaImage::from_raw(graph_data.width, graph_data.height, graph_data.data.clone()) {
                    Some(img) => img,
                    None => {
                        warn!("Failed to create RgbaImage from raw graph data for chart hash '{}'. Using fallback.", chart.short_hash);
                        return FALLBACK_KEY;
                    }
                };

                match renderer::create_texture(backend, &rgba_image, renderer::TextureColorSpace::Srgb) {
                    Ok(texture) => {
                        let key: &'static str = Box::leak(chart.short_hash.clone().into_boxed_str());
                        self.texture_manager.insert(key, texture);
                        self.current_density_graph = Some((key, chart.short_hash.clone()));
                        key
                    }
                    Err(e) => {
                        warn!("Failed to create GPU texture for density graph ('{}'): {}. Using fallback.", chart.short_hash, e);
                        FALLBACK_KEY
                    }
                }
            } else {
                // The chart exists, but has no graph data.
                self.destroy_current_density_graph();
                FALLBACK_KEY
            }
        } else {
            // No chart is selected (e.g., a pack header is selected).
            self.destroy_current_density_graph();
            FALLBACK_KEY
        }
    }

    fn handle_action(&mut self, action: ScreenAction, event_loop: &ActiveEventLoop) -> Result<(), Box<dyn Error>> {
        match action {
            ScreenAction::Navigate(screen) => {
                if matches!(self.transition, TransitionState::Idle) {
                    if self.current_screen == CurrentScreen::Init && screen == CurrentScreen::Menu {
                        info!("Starting special Init→Menu transition (bar squish; no global fade)");
                        self.transition = TransitionState::BarSquishOut { elapsed: 0.0, target: screen };
                    } else {
                        info!("Starting global fade out to screen: {:?}", screen);
                        self.transition = TransitionState::FadingOut { elapsed: 0.0, target: screen };
                    }
                }
            }
            ScreenAction::Exit => {
                info!("Exit action received. Shutting down.");
                event_loop.exit();
            }
            // Add the missing pattern arm here
            ScreenAction::RequestBanner(_) => {}
            ScreenAction::RequestDensityGraph(_) => {} // This action is handled in RedrawRequested
            ScreenAction::None => {}
        }
        Ok(())
    }

    fn build_screen(&self, actors: &[Actor], clear_color: [f32; 4], total_elapsed: f32) -> RenderList {
        crate::ui::compose::build_screen(actors, clear_color, &self.metrics, &self.fonts, total_elapsed)
    }

    fn get_current_actors(&self) -> (Vec<Actor>, [f32; 4]) {
        const CLEAR: [f32; 4] = [0.03, 0.03, 0.03, 1.0];
        let mut screen_alpha_multiplier = 1.0;
        if let TransitionState::ActorsFadeIn { elapsed } = self.transition {
            if self.current_screen == CurrentScreen::Menu {
                screen_alpha_multiplier = (elapsed / MENU_ACTORS_FADE_DURATION).clamp(0.0, 1.0);
            }
        }

        let mut actors = match self.current_screen {
            CurrentScreen::Menu     => menu::get_actors(&self.menu_state, screen_alpha_multiplier),
            CurrentScreen::Gameplay => gameplay::get_actors(&self.gameplay_state),
            CurrentScreen::Options  => options::get_actors(&self.options_state),
            CurrentScreen::SelectColor => select_color::get_actors(&self.select_color_state),
            CurrentScreen::SelectMusic => select_music::get_actors(&self.select_music_state),
            CurrentScreen::Init     => {
                if matches!(self.transition, TransitionState::BarSquishOut { .. }) {
                    init::get_actors_bg_only(&self.init_state)
                } else {
                    init::get_actors(&self.init_state)
                }
            }
        };

        if self.show_overlay {
            let overlay = crate::ui::components::stats_overlay::build(self.backend_type, self.last_fps, self.last_vpf);
            actors.extend(overlay);
        }

        let overlay_alpha = match self.transition {
            TransitionState::FadingOut { elapsed, .. } => ((elapsed - FADE_OUT_DELAY) / FADE_OUT_DURATION).clamp(0.0, 1.0),
            TransitionState::FadingIn  { elapsed, .. } => 1.0 - (elapsed / FADE_IN_DURATION).clamp(0.0, 1.0),
            _ => 0.0,
        };

        if overlay_alpha > 0.0 {
            actors.push(crate::ui::components::fade::black(overlay_alpha));
        }

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
            window.set_title(&format!("DeadSync - {:?} | {} | {:.2} FPS", self.backend_type, screen_name, fps));
            self.frame_count = 0;
            self.last_title_update = now;
        }
    }

    fn init_graphics(&mut self, event_loop: &ActiveEventLoop) -> Result<(), Box<dyn Error>> {
        let mut window_attributes = Window::default_attributes()
            .with_title(format!("DeadSync - {:?}", self.backend_type))
            .with_resizable(true);

        let window_width = self.display_width;
        let window_height = self.display_height;

        if self.fullscreen_enabled {
            let fullscreen = if let Some(mon) = event_loop.primary_monitor() {
                let best_mode = mon.video_modes()
                    .filter(|m| { let sz = m.size(); sz.width == window_width && sz.height == window_height })
                    .max_by_key(|m| m.refresh_rate_millihertz());
                if let Some(mode) = best_mode {
                    log::info!("Fullscreen: using EXCLUSIVE {}x{} @ {} mHz", window_width, window_height, mode.refresh_rate_millihertz());
                    Some(winit::window::Fullscreen::Exclusive(mode))
                } else {
                    log::warn!("No exact EXCLUSIVE mode {}x{}; using BORDERLESS.", window_width, window_height);
                    Some(winit::window::Fullscreen::Borderless(Some(mon)))
                }
            } else {
                log::warn!("No primary monitor reported; using BORDERLESS fullscreen.");
                Some(winit::window::Fullscreen::Borderless(None))
            };
            window_attributes = window_attributes.with_fullscreen(fullscreen);
        } else {
            window_attributes = window_attributes.with_inner_size(PhysicalSize::new(window_width, window_height));
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
        if window_id != window.id() { return; }
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
                    CurrentScreen::SelectMusic => select_music::handle_key_press(&mut self.select_music_state, &key_event),
                    CurrentScreen::Init     => init::handle_key_press(&mut self.init_state, &key_event),
                };
                if let Err(e) = self.handle_action(action.clone(), event_loop) {
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
                    TransitionState::FadingOut { elapsed, target } => {
                        *elapsed += delta_time;
                        if *elapsed >= FADE_OUT_DELAY + FADE_OUT_DURATION {
                            let prev = self.current_screen;
                            self.current_screen = *target;
                            
                            // ---- REFACTORED STATE PROPAGATION ----
                            // When leaving the color select screen, propagate the chosen color
                            // to all other relevant screens. This is the new source of truth.
                            if prev == CurrentScreen::SelectColor {
                                let idx = self.select_color_state.active_color_index;
                                self.menu_state.active_color_index = idx;
                                self.select_music_state.active_color_index = idx;
                                self.gameplay_state.player_color = color::decorative_rgba(idx);
                                self.options_state.active_color_index = idx; // <-- ADDED
                            }

                            // Handle initializations for the target screen.
                            if *target == CurrentScreen::SelectMusic {
                                // Re-init the screen but preserve the color we just set.
                                let current_color_index = self.select_music_state.active_color_index;
                                self.select_music_state = select_music::init();
                                self.select_music_state.active_color_index = current_color_index;
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
                    TransitionState::BarSquishOut { elapsed, target } => {
                        *elapsed += delta_time;
                        if *elapsed >= BAR_SQUISH_DURATION {
                            self.current_screen = *target;
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
                    TransitionState::Idle => {
                        match self.current_screen {
                            CurrentScreen::Gameplay => gameplay::update(&mut self.gameplay_state, &self.input_state, delta_time),
                            CurrentScreen::Init => {
                                let action = init::update(&mut self.init_state, delta_time);
                                if let ScreenAction::Navigate(_) | ScreenAction::Exit = action.clone() {
                                    if self.handle_action(action, event_loop).is_err() {}
                                }
                            }
                            CurrentScreen::SelectColor => select_color::update(&mut self.select_color_state, delta_time),
                            CurrentScreen::SelectMusic => {
                                let action = select_music::update(&mut self.select_music_state, delta_time);
                                match action {
                                    ScreenAction::RequestBanner(path_opt) => {
                                        let key = self.set_dynamic_banner(path_opt);
                                        self.select_music_state.current_banner_key = key;
                                    }
                                    ScreenAction::RequestDensityGraph(chart_opt) => {
                                        let key = self.set_density_graph(chart_opt.as_ref());
                                        self.select_music_state.current_graph_key = key;
                                    }
                                    _ => {}
                                }
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
        self.destroy_current_dynamic_banner();
        if let Some(backend) = &mut self.backend {
            renderer::dispose_textures(backend, &mut self.texture_manager);
            renderer::cleanup(backend);
        }
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let _ = env_logger::builder().filter_level(log::LevelFilter::Info).try_init();
    let config = crate::config::get();
    let backend_type = BackendType::Vulkan; // Using a single backend for now.
    let vsync_enabled = config.vsync;
    let fullscreen_enabled = !config.windowed;
    let show_stats = config.show_stats;
    let color_index = config.simply_love_color;

    song_loading::scan_and_load_songs("songs");
    let event_loop = EventLoop::new()?;
    let mut app = App::new(backend_type, vsync_enabled, fullscreen_enabled, show_stats, color_index);
    event_loop.run_app(&mut app)?;
    Ok(())
}
