use crate::audio::AudioManager;
use crate::config;
use crate::graphics::font::{load_font, Font, LoadedFontData};
use crate::graphics::renderer::{DescriptorSetId, Renderer};
use crate::graphics::texture::{load_texture, TextureResource};
use crate::graphics::vulkan_base::VulkanBase;
use crate::parsing::simfile::SongInfo;
use ash::Device;
use log::{error, info, warn, trace};
use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Arc;

// TextureId
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureId {
    Logo,
    Dancer,
    Arrows,
    FallbackBanner,
    // Gameplay Explosion Textures
    ExplosionW1, // Marvelous
    ExplosionW2, // Excellent
    ExplosionW3, // Great
    ExplosionW4, // Decent
    ExplosionW5, // Way Off / Boo
}

// FontId
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontId { Wendy, Miso, Cjk }

// SoundId
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SoundId {
    MenuChange,
    MenuStart,
    MenuExpandCollapse, // Added
}


pub struct AssetManager {
    textures: HashMap<TextureId, TextureResource>,
    fonts: HashMap<FontId, Font>,
    current_banner: Option<TextureResource>,
    current_banner_is_fallback: bool,
    current_banner_path_key: Option<PathBuf>, // To avoid reloading the same banner
}

impl AssetManager {
    pub fn new() -> Self {
        AssetManager {
            textures: HashMap::new(),
            fonts: HashMap::new(),
            current_banner: None,
            current_banner_is_fallback: true,
            current_banner_path_key: None,
        }
    }

    pub fn load_all(
        &mut self,
        base: &VulkanBase,
        renderer: &Renderer,
        audio_manager: &mut AudioManager,
    ) -> Result<(), Box<dyn Error>> {
        info!("Loading all assets...");

        info!("Loading non-font textures...");
        let logo_texture = load_texture(base, Path::new(config::LOGO_TEXTURE_PATH))?;
        renderer.update_texture_descriptor(&base.device, DescriptorSetId::Logo, &logo_texture);
        self.textures.insert(TextureId::Logo, logo_texture);

        let dance_texture = load_texture(base, Path::new(config::DANCE_TEXTURE_PATH))?;
        renderer.update_texture_descriptor(&base.device, DescriptorSetId::Dancer, &dance_texture);
        self.textures.insert(TextureId::Dancer, dance_texture);

        let arrow_texture = load_texture(base, Path::new(config::ARROW_TEXTURE_PATH))?;
        renderer.update_texture_descriptor(&base.device, DescriptorSetId::Gameplay, &arrow_texture);
        self.textures.insert(TextureId::Arrows, arrow_texture);

        let fallback_banner_texture = load_texture(base, Path::new("assets/graphics/fallback_banner.png"))?;
        renderer.update_texture_descriptor(&base.device, DescriptorSetId::FallbackBanner, &fallback_banner_texture);
        self.textures.insert(TextureId::FallbackBanner, fallback_banner_texture);
        info!("Fallback banner texture loaded and its static descriptor set updated.");

        if let Some(fallback_res) = self.textures.get(&TextureId::FallbackBanner) {
             renderer.update_texture_descriptor(
                 &base.device,
                 DescriptorSetId::DynamicBanner,
                 fallback_res,
             );
             self.current_banner_is_fallback = true;
             info!("Initialized DynamicBanner descriptor set with Fallback Banner.");
        } else {
            error!("Fallback banner failed to load, DynamicBanner descriptor not initialized!");
        }

        info!("Loading gameplay explosion textures...");
        let explosion_w1_tex = load_texture(base, Path::new(config::EXPLOSION_W1_TEXTURE_PATH))?;
        renderer.update_texture_descriptor(&base.device, DescriptorSetId::ExplosionW1, &explosion_w1_tex);
        self.textures.insert(TextureId::ExplosionW1, explosion_w1_tex);

        let explosion_w2_tex = load_texture(base, Path::new(config::EXPLOSION_W2_TEXTURE_PATH))?;
        renderer.update_texture_descriptor(&base.device, DescriptorSetId::ExplosionW2, &explosion_w2_tex);
        self.textures.insert(TextureId::ExplosionW2, explosion_w2_tex);

        let explosion_w3_tex = load_texture(base, Path::new(config::EXPLOSION_W3_TEXTURE_PATH))?;
        renderer.update_texture_descriptor(&base.device, DescriptorSetId::ExplosionW3, &explosion_w3_tex);
        self.textures.insert(TextureId::ExplosionW3, explosion_w3_tex);

        let explosion_w4_tex = load_texture(base, Path::new(config::EXPLOSION_W4_TEXTURE_PATH))?;
        renderer.update_texture_descriptor(&base.device, DescriptorSetId::ExplosionW4, &explosion_w4_tex);
        self.textures.insert(TextureId::ExplosionW4, explosion_w4_tex);

        let explosion_w5_tex = load_texture(base, Path::new(config::EXPLOSION_W5_TEXTURE_PATH))?;
        renderer.update_texture_descriptor(&base.device, DescriptorSetId::ExplosionW5, &explosion_w5_tex);
        self.textures.insert(TextureId::ExplosionW5, explosion_w5_tex);
        info!("Gameplay explosion textures loaded and descriptor sets updated.");


        info!("Loading MSDF fonts...");
        let wendy_loaded_data: LoadedFontData = load_font(
            base,
            Path::new(config::WENDY_MSDF_JSON_PATH),
            Path::new(config::WENDY_MSDF_TEXTURE_PATH),
        )?;
        renderer.update_texture_descriptor(
            &base.device,
            DescriptorSetId::FontWendy,
            &wendy_loaded_data.texture,
        );
        let wendy_font = Font {
            metrics: wendy_loaded_data.metrics,
            glyphs: wendy_loaded_data.glyphs,
            texture: wendy_loaded_data.texture,
            space_width: wendy_loaded_data.space_width,
            descriptor_set_id: DescriptorSetId::FontWendy,
        };
        self.fonts.insert(FontId::Wendy, wendy_font);
        info!("Wendy MSDF font (Wendy) loaded and descriptor set updated.");

        let miso_loaded_data: LoadedFontData = load_font(
            base,
            Path::new(config::MISO_MSDF_JSON_PATH),
            Path::new(config::MISO_MSDF_TEXTURE_PATH),
        )?;
        renderer.update_texture_descriptor(
            &base.device,
            DescriptorSetId::FontMiso,
            &miso_loaded_data.texture,
        );
        let miso_font = Font {
            metrics: miso_loaded_data.metrics,
            glyphs: miso_loaded_data.glyphs,
            texture: miso_loaded_data.texture,
            space_width: miso_loaded_data.space_width,
            descriptor_set_id: DescriptorSetId::FontMiso,
        };
        self.fonts.insert(FontId::Miso, miso_font);
        info!("Miso MSDF font loaded and descriptor set updated.");
        info!("All MSDF fonts loaded and descriptor sets updated.");

        info!("Loading sounds...");
        audio_manager.load_sfx(SoundId::MenuChange, Path::new(config::SFX_CHANGE_PATH))?;
        audio_manager.load_sfx(SoundId::MenuStart, Path::new(config::SFX_START_PATH))?;
        audio_manager.load_sfx(SoundId::MenuExpandCollapse, Path::new(config::SFX_EXPAND_PATH))?;
        info!("Sounds loaded.");

        info!("All assets loaded successfully.");
        Ok(())
    }

    fn destroy_current_dynamic_banner(&mut self, device: &Device) {
        if !self.current_banner_is_fallback {
            if let Some(mut old_banner) = self.current_banner.take() {
                info!("Destroying previous dynamic banner texture (path key: {:?}).", self.current_banner_path_key);
                old_banner.destroy(device);
            }
        }
        self.current_banner = None;
        self.current_banner_is_fallback = true; // Assume fallback until a new one is loaded
        self.current_banner_path_key = None;
    }


    pub fn load_song_banner(
        &mut self,
        base: &VulkanBase,
        renderer: &Renderer,
        song_info: &Arc<SongInfo>,
    ) {
        let new_banner_path_key = song_info.banner_path.clone();

        if self.current_banner_path_key == new_banner_path_key && new_banner_path_key.is_some() {
            // Banner is already loaded, no need to reload
            trace!("Song banner for {:?} already loaded.", new_banner_path_key.as_ref().unwrap());
            return;
        }

        self.destroy_current_dynamic_banner(&base.device); // Destroy old banner, reset state

        let mut loaded_successfully = false;
        if let Some(banner_path) = &song_info.banner_path { // This is the song_info.banner_path
            info!("Attempting to load song banner: {:?} for song: {}", banner_path, song_info.title);
            match load_texture(base, banner_path) {
                Ok(new_banner_texture) => {
                    info!("Successfully loaded song banner texture: {:?}", banner_path);
                    renderer.update_texture_descriptor(
                        &base.device,
                        DescriptorSetId::DynamicBanner,
                        &new_banner_texture,
                    );
                    self.current_banner = Some(new_banner_texture);
                    self.current_banner_is_fallback = false;
                    self.current_banner_path_key = Some(banner_path.clone());
                    loaded_successfully = true;
                }
                Err(e) => {
                    error!( "Failed to load song banner texture from {:?}: {}", banner_path, e );
                }
            }
        } else {
            info!("No banner path specified for song: {}", song_info.title);
        }

        if !loaded_successfully {
            info!("Using fallback banner for DynamicBanner set after attempting song banner.");
            if let Some(fallback_res) = self.textures.get(&TextureId::FallbackBanner) {
                 renderer.update_texture_descriptor(
                     &base.device,
                     DescriptorSetId::DynamicBanner,
                     fallback_res,
                 );
                 self.current_banner_is_fallback = true; // Explicitly mark as fallback
                 self.current_banner_path_key = None; // No specific path for fallback
            } else {
                error!("Fallback banner resource not found! DynamicBanner set might be invalid.");
            }
        }
    }

    pub fn load_pack_banner(
        &mut self,
        base: &VulkanBase,
        renderer: &Renderer,
        pack_banner_path_opt: Option<&Path>,
    ) {
        let new_banner_path_key = pack_banner_path_opt.map(|p| p.to_path_buf());

        if self.current_banner_path_key == new_banner_path_key && new_banner_path_key.is_some() {
             trace!("Pack banner for {:?} already loaded.", new_banner_path_key.as_ref().unwrap());
            return;
        }

        self.destroy_current_dynamic_banner(&base.device); // Destroy old banner, reset state

        let mut loaded_successfully = false;
        if let Some(pack_banner_path) = pack_banner_path_opt {
            info!("Attempting to load pack banner: {:?}", pack_banner_path);
            match load_texture(base, pack_banner_path) {
                Ok(new_banner_texture) => {
                    info!("Successfully loaded pack banner texture: {:?}", pack_banner_path);
                    renderer.update_texture_descriptor(
                        &base.device,
                        DescriptorSetId::DynamicBanner,
                        &new_banner_texture,
                    );
                    self.current_banner = Some(new_banner_texture);
                    self.current_banner_is_fallback = false;
                    self.current_banner_path_key = Some(pack_banner_path.to_path_buf());
                    loaded_successfully = true;
                }
                Err(e) => {
                    error!("Failed to load pack banner texture from {:?}: {}", pack_banner_path, e);
                }
            }
        } else {
            info!("No pack banner path provided for current pack selection.");
        }

        if !loaded_successfully {
            info!("Using fallback banner for DynamicBanner set after attempting pack banner.");
            if let Some(fallback_res) = self.textures.get(&TextureId::FallbackBanner) {
                renderer.update_texture_descriptor(
                    &base.device,
                    DescriptorSetId::DynamicBanner,
                    fallback_res,
                );
                self.current_banner_is_fallback = true;
                self.current_banner_path_key = None;
            } else {
                error!("Fallback banner resource not found! DynamicBanner set might be invalid.");
            }
        }
    }

    // Call this when leaving SelectMusic state to ensure fallback is active
    pub fn clear_current_banner(&mut self, device: &Device) {
        self.destroy_current_dynamic_banner(device);
        // The destroy_current_dynamic_banner already sets current_banner_is_fallback to true
        // and current_banner_path_key to None.
        // The DynamicBanner descriptor itself will be reset to fallback by transition_state in app.rs
        info!("Cleared current dynamic banner, DynamicBanner will be reset to fallback by App state transition.");
    }


    pub fn get_texture(&self, id: TextureId) -> Option<&TextureResource> {
        self.textures.get(&id)
    }

    pub fn get_font(&self, id: FontId) -> Option<&Font> {
        self.fonts.get(&id)
    }

    pub fn get_current_banner_path(&self) -> Option<PathBuf> {
        self.current_banner_path_key.clone()
    }

    pub fn destroy(&mut self, device: &Device) {
        info!("Destroying AssetManager resources...");
        for (_, texture) in self.textures.iter_mut() {
            texture.destroy(device);
        }
        self.textures.clear();
        info!("Static textures (including explosions and fallback banner) destroyed.");

        self.destroy_current_dynamic_banner(device); // Ensures any loaded dynamic banner is destroyed
        info!("Dynamic banner resources checked/destroyed.");

        for (_, font) in self.fonts.iter_mut() {
            font.destroy(device);
        }
        self.fonts.clear();
        info!("Font resources destroyed.");
        info!("AssetManager resources destroyed.");
    }
}