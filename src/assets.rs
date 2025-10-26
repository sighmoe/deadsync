use crate::core::gfx::{self as renderer, Backend, Texture as GfxTexture};
use crate::gameplay::profile;
use crate::ui::font::{self, Font, FontLoadData};
use image::RgbaImage;
use log::{info, warn};
use std::{
    collections::HashMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

// --- Texture Metadata ---

#[derive(Clone, Copy, Debug)]
pub struct TexMeta {
    pub w: u32,
    pub h: u32,
}

static TEX_META: once_cell::sync::Lazy<RwLock<HashMap<String, TexMeta>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(HashMap::new()));

pub fn register_texture_dims(key: &str, w: u32, h: u32) {
    let mut m = TEX_META.write().unwrap();
    m.insert(key.to_string(), TexMeta { w, h });
}

pub fn texture_dims(key: &str) -> Option<TexMeta> {
    TEX_META.read().unwrap().get(key).copied()
}

pub fn canonical_texture_key<P: AsRef<Path>>(p: P) -> String {
    let p = p.as_ref();
    let rel = p.strip_prefix(Path::new("assets")).unwrap_or(p);
    rel.to_string_lossy().replace('\\', "/")
}

pub fn parse_sprite_sheet_dims(filename: &str) -> (u32, u32) {
    let s = filename;
    let bytes = s.as_bytes();
    let n = bytes.len();

    let lower = s.to_ascii_lowercase();
    let lb = lower.as_bytes();
    let mut res_spans: Vec<(usize, usize)> = Vec::new();
    let mut i = 0usize;
    while i < n {
        if lb[i] == b'(' && i + 4 <= n && &lb[i..i + 4] == b"(res" {
            let mut j = i + 4;
            while j < n && lb[j] != b')' {
                j += 1;
            }
            if j < n && lb[j] == b')' {
                res_spans.push((i, j));
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    let in_res = |idx: usize| -> bool {
        for (a, b) in &res_spans {
            if idx >= *a && idx <= *b {
                return true;
            }
        }
        false
    };

    let mut pairs: Vec<(usize, u32, u32)> = Vec::new();
    i = 0;
    while i < n {
        if (bytes[i] == b'x' || bytes[i] == b'X') && i > 0 && bytes[i - 1].is_ascii_digit() {
            let mut l = i;
            while l > 0 && bytes[l - 1].is_ascii_digit() {
                l -= 1;
            }
            let mut r = i + 1;
            while r < n && bytes[r].is_ascii_digit() {
                r += 1;
            }
            if l < i && i + 1 < r {
                if let (Ok(ws), Ok(hs)) = (std::str::from_utf8(&bytes[l..i]), std::str::from_utf8(&bytes[i + 1..r])) {
                    if let (Ok(w), Ok(h)) = (ws.parse::<u32>(), hs.parse::<u32>()) {
                        if w > 0 && h > 0 {
                            pairs.push((l, w, h));
                        }
                    }
                }
            }
        }
        i += 1;
    }

    for (pos, w, h) in pairs.into_iter().rev() {
        if !in_res(pos) {
            return (w, h);
        }
    }
    (1, 1)
}


// --- Asset Manager ---

pub struct AssetManager {
    pub textures: HashMap<String, GfxTexture>,
    fonts: HashMap<&'static str, Font>,
    current_dynamic_banner: Option<(String, PathBuf)>,
    current_density_graph: Option<(String, String)>,
    current_dynamic_background: Option<(String, PathBuf)>,
    current_profile_avatar: Option<(String, PathBuf)>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            fonts: HashMap::new(),
            current_dynamic_banner: None,
            current_density_graph: None,
            current_dynamic_background: None,
            current_profile_avatar: None,
        }
    }

    // --- Font Management (moved from ui/font.rs global static) ---

    pub fn register_font(&mut self, name: &'static str, font: Font) {
        self.fonts.insert(name, font);
    }

    pub fn with_fonts<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&HashMap<&'static str, Font>) -> R,
    {
        f(&self.fonts)
    }

    pub fn with_font<F, R>(&self, name: &str, f: F) -> Option<R>
    where
        F: FnOnce(&Font) -> R,
    {
        self.fonts.get(name).map(f)
    }

    // --- Loading Logic (moved from app.rs) ---

    pub fn load_initial_assets(&mut self, backend: &mut Backend) -> Result<(), Box<dyn Error>> {
        self.load_initial_textures(backend)?;
        self.load_initial_fonts(backend)?;
        Ok(())
    }

    fn load_initial_textures(&mut self, backend: &mut Backend) -> Result<(), Box<dyn Error>> {
        info!("Loading initial textures...");

        #[inline(always)]
        fn fallback_rgba() -> RgbaImage {
            let data: [u8; 16] = [
                255, 0, 255, 255, 128, 128, 128, 255, 128, 128, 128, 255, 255, 0, 255, 255,
            ];
            RgbaImage::from_raw(2, 2, data.to_vec()).expect("fallback image")
        }

        // Load __white texture
        let white_img = RgbaImage::from_raw(1, 1, vec![255, 255, 255, 255]).unwrap();
        let white_tex = renderer::create_texture(backend, &white_img)?;
        self.textures.insert("__white".to_string(), white_tex);
        register_texture_dims("__white", 1, 1);
        info!("Loaded built-in texture: __white");

        let mut textures_to_load: Vec<(String, String)> = vec![
            ("logo.png".to_string(), "logo.png".to_string()),
            ("init_arrow.png".to_string(), "init_arrow.png".to_string()),
            ("dance.png".to_string(), "dance.png".to_string()),
            ("meter_arrow.png".to_string(), "meter_arrow.png".to_string()),
            ("rounded-square.png".to_string(), "rounded-square.png".to_string()),
            ("swoosh.png".to_string(), "swoosh.png".to_string()),
            ("heart.png".to_string(), "heart.png".to_string()),
            ("banner1.png".to_string(), "_fallback/banner1.png".to_string()),
            ("banner2.png".to_string(), "_fallback/banner2.png".to_string()),
            ("banner3.png".to_string(), "_fallback/banner3.png".to_string()),
            ("banner4.png".to_string(), "_fallback/banner4.png".to_string()),
            ("banner5.png".to_string(), "_fallback/banner5.png".to_string()),
            ("banner6.png".to_string(), "_fallback/banner6.png".to_string()),
            ("banner7.png".to_string(), "_fallback/banner7.png".to_string()),
            ("banner8.png".to_string(), "_fallback/banner8.png".to_string()),
            ("banner9.png".to_string(), "_fallback/banner9.png".to_string()),
            ("banner10.png".to_string(), "_fallback/banner10.png".to_string()),
            ("banner11.png".to_string(), "_fallback/banner11.png".to_string()),
            ("banner12.png".to_string(), "_fallback/banner12.png".to_string()),
            ("noteskins/bar/tex notes.png".to_string(), "noteskins/bar/tex notes.png".to_string()),
            ("noteskins/bar/tex receptors.png".to_string(), "noteskins/bar/tex receptors.png".to_string()),
            ("noteskins/bar/tex glow.png".to_string(), "noteskins/bar/tex glow.png".to_string()),
            (
                "judgements/Love 2x7 (doubleres).png".to_string(),
                "judgements/Love 2x7 (doubleres).png".to_string(),
            ),
            ("grades/grades 1x19.png".to_string(), "grades/grades 1x19.png".to_string()),
        ];

        if let Ok(entries) = fs::read_dir("assets/noteskins/cel") {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("png") {
                        if let Ok(name) = entry.file_name().into_string() {
                            let key = format!("noteskins/cel/{}", name);
                            textures_to_load.push((key.clone(), key));
                        }
                    }
                }
            }
        }

        let mut handles = Vec::with_capacity(textures_to_load.len());
        for (key, relative_path) in textures_to_load {
            handles.push(std::thread::spawn(move || {
                let path = if relative_path.starts_with("noteskins/") {
                    Path::new("assets").join(&relative_path)
                } else {
                    Path::new("assets/graphics").join(&relative_path)
                };
                match image::open(&path) {
                    Ok(img) => Ok::<(String, RgbaImage), (String, String)>((key, img.to_rgba8())),
                    Err(e) => Err((key, e.to_string())),
                }
            }));
        }

        let fallback_image = Arc::new(fallback_rgba());
        for h in handles {
            match h.join().expect("texture decode thread panicked") {
                Ok((key, rgba)) => {
                    let texture = renderer::create_texture(backend, &rgba)?;
                    register_texture_dims(&key, rgba.width(), rgba.height());
                    info!("Loaded texture: {}", key);
                    self.textures.insert(key, texture);
                }
                Err((key, msg)) => {
                    warn!("Failed to load texture for key '{}': {}. Using fallback.", key, msg);
                    let texture = renderer::create_texture(backend, &fallback_image)?;
                    register_texture_dims(&key, fallback_image.width(), fallback_image.height());
                    self.textures.insert(key, texture);
                }
            }
        }

        let profile = profile::get();
        self.set_profile_avatar(backend, profile.avatar_path);

        Ok(())
    }

    fn load_initial_fonts(&mut self, backend: &mut Backend) -> Result<(), Box<dyn Error>> {
        for &name in &["wendy", "miso", "cjk", "emoji", "game", "wendy_monospace_numbers", "wendy_screenevaluation", "wendy_combo", "wendy_white" ] {
            let ini_path_str = match name {
                "wendy" => "assets/fonts/wendy/_wendy small.ini",
                "miso"  => "assets/fonts/miso/_miso light.ini",
                "cjk" => "assets/fonts/cjk/_jfonts 16px.ini",
                "emoji" => "assets/fonts/emoji/_emoji 16px.ini",
                "game" => "assets/fonts/game/_game chars 16px.ini",
                "wendy_monospace_numbers" => "assets/fonts/wendy/_wendy monospace numbers.ini",
                "wendy_screenevaluation" => "assets/fonts/wendy/_ScreenEvaluation numbers.ini",
                "wendy_combo" => "assets/fonts/_combo/wendy/Wendy.ini",
                "wendy_white" => "assets/fonts/wendy/_wendy white.ini",
                _ => return Err(format!("Unknown font name: {}", name).into()),
            };

            let FontLoadData { mut font, required_textures } = font::parse(ini_path_str)?;

            if name == "miso" {
                font.fallback_font_name = Some("cjk");
                info!("Font 'miso' configured to use 'cjk' as fallback.");
            }

            if name == "cjk" {
                font.fallback_font_name = Some("emoji");
                info!("Font 'cjk' configured to use 'emoji' as fallback.");
            }

            for tex_path in &required_textures {
                let key = canonical_texture_key(tex_path);
                if !self.textures.contains_key(&key) {
                    let image_data = image::open(tex_path)?.to_rgba8();
                    let texture = renderer::create_texture(backend, &image_data)?;
                    register_texture_dims(&key, image_data.width(), image_data.height());
                    self.textures.insert(key.clone(), texture);
                    info!("Loaded font texture: {}", key);
                }
            }
            self.register_font(name, font);
            info!("Loaded font '{}' from '{}'", name, ini_path_str);
        }
        Ok(())
    }

    // --- Dynamic Asset Management (moved from app.rs) ---

    pub fn destroy_dynamic_assets(&mut self, backend: &mut Backend) {
        if let Some((key, _)) = self.current_dynamic_banner.take() {
            if let Backend::Vulkan(vk_state) = backend {
                if let Some(device) = &vk_state.device { unsafe { let _ = device.device_wait_idle(); } }
            }
            self.textures.remove(&key);
        }
        if let Some((key, _)) = self.current_density_graph.take() {
            if let Backend::Vulkan(vk_state) = backend {
                if let Some(device) = &vk_state.device { unsafe { let _ = device.device_wait_idle(); } }
            }
            self.textures.remove(&key);
        }
        if let Some((key, _)) = self.current_dynamic_background.take() {
            if let Backend::Vulkan(vk_state) = backend {
                if let Some(device) = &vk_state.device { unsafe { let _ = device.device_wait_idle(); } }
            }
            self.textures.remove(&key);
        }
        self.destroy_current_profile_avatar(backend);
    }

    pub fn set_dynamic_banner(&mut self, backend: &mut Backend, path_opt: Option<PathBuf>) -> String {
        if let Some(path) = path_opt {
            if self.current_dynamic_banner.as_ref().map_or(false, |(_, p)| p == &path) {
                return self.current_dynamic_banner.as_ref().unwrap().0.clone();
            }

            self.destroy_current_dynamic_banner(backend);

            match image::open(&path) {
                Ok(img) => {
                    let rgba = img.to_rgba8();
                    match renderer::create_texture(backend, &rgba) {
                        Ok(texture) => {
                            let key = path.to_string_lossy().into_owned();
                            self.textures.insert(key.clone(), texture);
                            register_texture_dims(&key, rgba.width(), rgba.height());
                            self.current_dynamic_banner = Some((key.clone(), path));
                            key
                        }
                        Err(e) => {
                            warn!("Failed to create GPU texture for {:?}: {}. Using fallback.", path, e);
                            "banner1.png".to_string()
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to open banner image {:?}: {}. Using fallback.", path, e);
                    "banner1.png".to_string()
                }
            }
        } else {
            self.destroy_current_dynamic_banner(backend);
            "banner1.png".to_string()
        }
    }

    pub fn set_density_graph(
        &mut self,
        backend: &mut Backend,
        data: Option<(String, rssp::graph::GraphImageData)>,
    ) -> String {
        const FALLBACK_KEY: &str = "__white";

        if let Some((key, graph_data)) = data {
            // The cache check key is the second item in the tuple.
            if self.current_density_graph.as_ref().map_or(false, |(_, cache_key)| cache_key == &key) {
                return self.current_density_graph.as_ref().unwrap().0.clone();
            }

            self.destroy_current_density_graph(backend);

            let rgba_image = match RgbaImage::from_raw(graph_data.width, graph_data.height, graph_data.data) {
                Some(img) => img,
                None => {
                    warn!("Failed to create RgbaImage from raw graph data for key '{}'.", key);
                    return FALLBACK_KEY.to_string();
                }
            };

            match renderer::create_texture(backend, &rgba_image) {
                Ok(texture) => {
                    // The texture key is the same as the cache key.
                    self.textures.insert(key.clone(), texture);
                    register_texture_dims(&key, rgba_image.width(), rgba_image.height());
                    self.current_density_graph = Some((key.clone(), key.clone()));
                    key
                }
                Err(e) => {
                    warn!("Failed to create GPU texture for density graph ('{}'): {}.", key, e);
                    FALLBACK_KEY.to_string()
                }
            }
        } else {
            self.destroy_current_density_graph(backend);
            FALLBACK_KEY.to_string()
        }
    }

    pub fn set_dynamic_background(&mut self, backend: &mut Backend, path_opt: Option<PathBuf>) -> String {
        const FALLBACK_KEY: &str = "__white";

        if let Some(path) = path_opt {
            if self.current_dynamic_background.as_ref().map_or(false, |(_, p)| p == &path) {
                return self.current_dynamic_background.as_ref().unwrap().0.clone();
            }

            self.destroy_current_dynamic_background(backend);

            match image::open(&path) {
                Ok(img) => {
                    let rgba = img.to_rgba8();
                    match renderer::create_texture(backend, &rgba) {
                        Ok(texture) => {
                            let key = path.to_string_lossy().into_owned();
                            self.textures.insert(key.clone(), texture);
                            register_texture_dims(&key, rgba.width(), rgba.height());
                            self.current_dynamic_background = Some((key.clone(), path));
                            key
                        }
                        Err(e) => {
                            warn!("Failed to create GPU texture for background {:?}: {}. Using fallback.", path, e);
                            FALLBACK_KEY.to_string()
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to open background image {:?}: {}. Using fallback.", path, e);
                    FALLBACK_KEY.to_string()
                }
            }
        } else {
            self.destroy_current_dynamic_background(backend);
            FALLBACK_KEY.to_string()
        }
    }

    pub fn set_profile_avatar(&mut self, backend: &mut Backend, path_opt: Option<PathBuf>) {
        if let Some(path) = path_opt {
            if self.current_profile_avatar.as_ref().map_or(false, |(_, p)| p == &path) {
                if let Some((key, _)) = &self.current_profile_avatar {
                    profile::set_avatar_texture_key(Some(key.clone()));
                }
                return;
            }

            self.destroy_current_profile_avatar(backend);

            match image::open(&path) {
                Ok(img) => {
                    let rgba = img.to_rgba8();
                    match renderer::create_texture(backend, &rgba) {
                        Ok(texture) => {
                            let key = path.to_string_lossy().into_owned();
                            self.textures.insert(key.clone(), texture);
                            register_texture_dims(&key, rgba.width(), rgba.height());
                            self.current_profile_avatar = Some((key.clone(), path));
                            profile::set_avatar_texture_key(Some(key));
                        }
                        Err(e) => {
                            warn!("Failed to create GPU texture for profile avatar {:?}: {}.", path, e);
                            profile::set_avatar_texture_key(None);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to open profile avatar image {:?}: {}.", path, e);
                    profile::set_avatar_texture_key(None);
                }
            }
        } else {
            self.destroy_current_profile_avatar(backend);
        }
    }

    fn destroy_current_dynamic_banner(&mut self, backend: &mut Backend) {
        if let Some((key, _)) = self.current_dynamic_banner.take() {
            if let Backend::Vulkan(vk_state) = backend {
                if let Some(device) = &vk_state.device { unsafe { let _ = device.device_wait_idle(); } }
            }
            self.textures.remove(&key);
        }
    }

    fn destroy_current_density_graph(&mut self, backend: &mut Backend) {
        if let Some((key, _)) = self.current_density_graph.take() {
            if let Backend::Vulkan(vk_state) = backend {
                if let Some(device) = &vk_state.device { unsafe { let _ = device.device_wait_idle(); } }
            }
            self.textures.remove(&key);
        }
    }

    fn destroy_current_dynamic_background(&mut self, backend: &mut Backend) {
        if let Some((key, _)) = self.current_dynamic_background.take() {
            if let Backend::Vulkan(vk_state) = backend {
                if let Some(device) = &vk_state.device { unsafe { let _ = device.device_wait_idle(); } }
            }
            self.textures.remove(&key);
        }
    }

    fn destroy_current_profile_avatar(&mut self, backend: &mut Backend) {
        if let Some((key, _)) = self.current_profile_avatar.take() {
            if let Backend::Vulkan(vk_state) = backend {
                if let Some(device) = &vk_state.device { unsafe { let _ = device.device_wait_idle(); } }
            }
            self.textures.remove(&key);
        }
        profile::set_avatar_texture_key(None);
    }
}
