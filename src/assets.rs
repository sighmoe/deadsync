use crate::audio::AudioManager;
use crate::config;
use crate::graphics::font::{load_font, Font, LoadedFontData};
use crate::graphics::renderer::{DescriptorSetId, Renderer};
use crate::graphics::texture::{load_texture, TextureResource};
use crate::graphics::vulkan_base::VulkanBase;
use crate::parsing::simfile::SongInfo;
use ash::Device;
use log::{error, info, warn};
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
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

// FontId, SoundId (keep as is)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum FontId { Wendy, Miso, Cjk }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum SoundId { MenuChange, MenuStart }


pub struct AssetManager {
    textures: HashMap<TextureId, TextureResource>,
    fonts: HashMap<FontId, Font>,
    current_banner: Option<TextureResource>, // Stores the currently loaded song-specific banner
    current_banner_is_fallback: bool, // Tracks if current_banner is actually the fallback
}

impl AssetManager {
    pub fn new() -> Self {
        AssetManager {
            textures: HashMap::new(),
            fonts: HashMap::new(),
            current_banner: None,
            current_banner_is_fallback: true, // Initially, dynamic banner points to fallback
        }
    }

    pub fn load_all(
        &mut self,
        base: &VulkanBase,
        renderer: &Renderer,
        audio_manager: &mut AudioManager,
    ) -> Result<(), Box<dyn Error>> {
        info!("Loading all assets...");

        // --- Load Static Textures ---
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

        // Initialize DynamicBanner descriptor set to point to the fallback banner initially
        if let Some(fallback_res) = self.textures.get(&TextureId::FallbackBanner) {
             renderer.update_texture_descriptor(
                 &base.device,
                 DescriptorSetId::DynamicBanner, // This is the one that changes
                 fallback_res,
             );
             self.current_banner_is_fallback = true; // Mark that DynamicBanner currently uses fallback
             info!("Initialized DynamicBanner descriptor set with Fallback Banner.");
        } else {
            error!("Fallback banner failed to load, DynamicBanner descriptor not initialized!");
        }

        // --- Load Gameplay Explosion Textures ---
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


        // --- Load MSDF Fonts ---
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

        // --- Load Sounds ---
        info!("Loading sounds...");
        audio_manager.load_sfx(SoundId::MenuChange, Path::new(config::SFX_CHANGE_PATH))?;
        audio_manager.load_sfx(SoundId::MenuStart, Path::new(config::SFX_START_PATH))?;
        info!("Sounds loaded.");

        info!("All assets loaded successfully.");
        Ok(())
    }

    pub fn load_song_banner(
        &mut self,
        base: &VulkanBase,
        renderer: &Renderer,
        song_info: &Arc<SongInfo>, // MODIFIED
    ) {
        info!("Attempting to load banner for song: {}", song_info.title);
        // If a previous song-specific banner was loaded, destroy it
        if !self.current_banner_is_fallback {
            if let Some(mut old_banner) = self.current_banner.take() {
                info!("Destroying previous dynamic (song-specific) banner texture.");
                old_banner.destroy(&base.device);
            }
        }
        self.current_banner = None; // Reset current banner field
        self.current_banner_is_fallback = false; // Assume we'll load a new one

        let mut loaded_successfully = false;
        if let Some(banner_path) = &song_info.banner_path {
            info!("Found banner path: {:?}", banner_path);
            match load_texture(base, banner_path) {
                Ok(new_banner_texture) => {
                    info!("Successfully loaded banner texture: {:?}", banner_path);
                    renderer.update_texture_descriptor(
                        &base.device,
                        DescriptorSetId::DynamicBanner,
                        &new_banner_texture,
                    );
                    self.current_banner = Some(new_banner_texture); // Store the new song-specific banner
                    // self.current_banner_is_fallback remains false
                    loaded_successfully = true;
                }
                Err(e) => {
                    error!( "Failed to load banner texture from {:?}: {}", banner_path, e );
                }
            }
        } else {
            info!("No banner path specified for song: {}", song_info.title);
        }

        if !loaded_successfully {
            info!("Using fallback banner for DynamicBanner set.");
            if let Some(fallback_res) = self.textures.get(&TextureId::FallbackBanner) {
                 renderer.update_texture_descriptor(
                     &base.device,
                     DescriptorSetId::DynamicBanner,
                     fallback_res, // Use the already loaded fallback texture
                 );
                 // self.current_banner remains None, as it's not a song-specific banner
                 self.current_banner_is_fallback = true;
            } else {
                error!("Fallback banner resource not found in textures map! DynamicBanner set might be invalid.");
                // self.current_banner remains None
                self.current_banner_is_fallback = true; // Still true, as we failed to load a specific one AND fallback
            }
        }
    }

    pub fn get_texture(&self, id: TextureId) -> Option<&TextureResource> {
        self.textures.get(&id)
    }

    pub fn get_font(&self, id: FontId) -> Option<&Font> {
        self.fonts.get(&id)
    }

    pub fn destroy(&mut self, device: &Device) {
        info!("Destroying AssetManager resources...");
        for (_, texture) in self.textures.iter_mut() {
            texture.destroy(device);
        }
        self.textures.clear();
        info!("Static textures (including explosions and fallback banner) destroyed.");

        // Destroy the currently loaded song-specific banner if it's not the fallback
        if !self.current_banner_is_fallback {
            if let Some(mut banner) = self.current_banner.take() {
                info!("Destroying dynamic (song-specific) banner texture.");
                banner.destroy(device);
            }
        }
        info!("Dynamic banner resources checked/destroyed.");

        for (_, font) in self.fonts.iter_mut() {
            font.destroy(device);
        }
        self.fonts.clear();
        info!("Font resources destroyed.");
        info!("AssetManager resources destroyed.");
    }
}