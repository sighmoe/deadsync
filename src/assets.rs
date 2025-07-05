use crate::audio::AudioManager;
use crate::config;
use crate::graphics::font::{load_font, Font, LoadedFontData};
use crate::graphics::renderer::{DescriptorSetId, Renderer};
use crate::graphics::texture::{load_texture, TextureResource};
use crate::graphics::vulkan_base::VulkanBase;
use crate::parsing::simfile::SongInfo;
use ash::Device;
use log::{error, info, trace};
use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureId {
    Logo,
    Dancer,
    Arrows,
    FallbackBanner,
    MeterArrow,
    ExplosionW1,
    ExplosionW2,
    ExplosionW3,
    ExplosionW4,
    ExplosionW5,
    JudgmentGraphics,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontId {
    Wendy,
    Miso,
    Cjk,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SoundId {
    MenuChange,
    MenuStart,
    MenuExpandCollapse,
    DifficultyEasier,
    DifficultyHarder,
}

pub struct AssetManager {
    textures: HashMap<TextureId, TextureResource>,
    fonts: HashMap<FontId, Font>,
    current_banner: Option<TextureResource>,
    current_banner_is_fallback: bool,
    current_banner_path_key: Option<PathBuf>,
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

        self.load_static_textures(base, renderer)?;
        self.load_fonts(base, renderer)?;
        self.load_sounds(audio_manager)?;

        self.initialize_dynamic_banner_to_fallback(base, renderer);

        info!("All assets loaded successfully.");
        Ok(())
    }

    fn load_static_textures(
        &mut self,
        base: &VulkanBase,
        renderer: &Renderer,
    ) -> Result<(), Box<dyn Error>> {
        info!("Loading non-font textures...");
        self.load_and_register_texture(
            base,
            renderer,
            TextureId::Logo,
            config::LOGO_TEXTURE_PATH,
            DescriptorSetId::Logo,
        )?;
        self.load_and_register_texture(
            base,
            renderer,
            TextureId::Dancer,
            config::DANCE_TEXTURE_PATH,
            DescriptorSetId::Dancer,
        )?;
        self.load_and_register_texture(
            base,
            renderer,
            TextureId::Arrows,
            config::ARROW_TEXTURE_PATH,
            DescriptorSetId::Gameplay,
        )?;
        self.load_and_register_texture(
            base,
            renderer,
            TextureId::FallbackBanner,
            "assets/graphics/fallback_banner.png",
            DescriptorSetId::FallbackBanner,
        )?;
        self.load_and_register_texture(
            base,
            renderer,
            TextureId::MeterArrow,
            config::METER_ARROW_TEXTURE_PATH,
            DescriptorSetId::MeterArrow,
        )?;
        
        info!("Loading judgment graphic textures");
        self.load_and_register_texture(
            base,
            renderer,
            TextureId::JudgmentGraphics,
            config::JUDGMENT_GRAPHICS_CHROMATIC_PATH,
            DescriptorSetId::JudgmentGraphics,
        )?;

        info!("Loading gameplay explosion textures...");
        self.load_and_register_texture(
            base,
            renderer,
            TextureId::ExplosionW1,
            config::EXPLOSION_W1_TEXTURE_PATH,
            DescriptorSetId::ExplosionW1,
        )?;
        self.load_and_register_texture(
            base,
            renderer,
            TextureId::ExplosionW2,
            config::EXPLOSION_W2_TEXTURE_PATH,
            DescriptorSetId::ExplosionW2,
        )?;
        self.load_and_register_texture(
            base,
            renderer,
            TextureId::ExplosionW3,
            config::EXPLOSION_W3_TEXTURE_PATH,
            DescriptorSetId::ExplosionW3,
        )?;
        self.load_and_register_texture(
            base,
            renderer,
            TextureId::ExplosionW4,
            config::EXPLOSION_W4_TEXTURE_PATH,
            DescriptorSetId::ExplosionW4,
        )?;
        self.load_and_register_texture(
            base,
            renderer,
            TextureId::ExplosionW5,
            config::EXPLOSION_W5_TEXTURE_PATH,
            DescriptorSetId::ExplosionW5,
        )?;
        info!("Gameplay explosion textures loaded.");
        Ok(())
    }

    fn load_and_register_texture(
        &mut self,
        base: &VulkanBase,
        renderer: &Renderer,
        id: TextureId,
        path_str: &str,
        descriptor_set_id: DescriptorSetId,
    ) -> Result<(), Box<dyn Error>> {
        let texture = load_texture(base, Path::new(path_str))?;
        renderer.update_texture_descriptor(&base.device, descriptor_set_id, &texture);
        self.textures.insert(id, texture);
        Ok(())
    }

    fn load_fonts(&mut self, base: &VulkanBase, renderer: &Renderer) -> Result<(), Box<dyn Error>> {
        info!("Loading MSDF fonts...");
        self.load_and_register_font(
            base,
            renderer,
            FontId::Wendy,
            config::WENDY_MSDF_JSON_PATH,
            config::WENDY_MSDF_TEXTURE_PATH,
            DescriptorSetId::FontWendy,
        )?;
        self.load_and_register_font(
            base,
            renderer,
            FontId::Miso,
            config::MISO_MSDF_JSON_PATH,
            config::MISO_MSDF_TEXTURE_PATH,
            DescriptorSetId::FontMiso,
        )?;
        // Example for CJK if it were to be used:
        // self.load_and_register_font(base, renderer, FontId::Cjk, config::CJK_MSDF_JSON_PATH, config::CJK_MSDF_TEXTURE_PATH, DescriptorSetId::FontCjk)?;
        info!("All MSDF fonts loaded.");
        Ok(())
    }

    fn load_and_register_font(
        &mut self,
        base: &VulkanBase,
        renderer: &Renderer,
        id: FontId,
        json_path_str: &str,
        texture_path_str: &str,
        descriptor_set_id: DescriptorSetId,
    ) -> Result<(), Box<dyn Error>> {
        let loaded_data: LoadedFontData =
            load_font(base, Path::new(json_path_str), Path::new(texture_path_str))?;
        renderer.update_texture_descriptor(&base.device, descriptor_set_id, &loaded_data.texture);
        let font = Font {
            metrics: loaded_data.metrics,
            glyphs: loaded_data.glyphs,
            texture: loaded_data.texture,
            space_width: loaded_data.space_width,
            descriptor_set_id,
        };
        self.fonts.insert(id, font);
        info!("{:?} MSDF font loaded and descriptor set updated.", id);
        Ok(())
    }

    fn load_sounds(&mut self, audio_manager: &mut AudioManager) -> Result<(), Box<dyn Error>> {
        info!("Loading sounds...");
        audio_manager.load_sfx(SoundId::MenuChange, Path::new(config::SFX_CHANGE_PATH))?;
        audio_manager.load_sfx(SoundId::MenuStart, Path::new(config::SFX_START_PATH))?;
        audio_manager.load_sfx(
            SoundId::MenuExpandCollapse,
            Path::new(config::SFX_EXPAND_PATH),
        )?;
        audio_manager.load_sfx(
            SoundId::DifficultyEasier,
            Path::new(config::SFX_DIFFICULTY_EASIER_PATH),
        )?;
        audio_manager.load_sfx(
            SoundId::DifficultyHarder,
            Path::new(config::SFX_DIFFICULTY_HARDER_PATH),
        )?;
        info!("Sounds loaded.");
        Ok(())
    }

    fn initialize_dynamic_banner_to_fallback(&mut self, base: &VulkanBase, renderer: &Renderer) {
        if let Some(fallback_res) = self.textures.get(&TextureId::FallbackBanner) {
            renderer.update_texture_descriptor(
                &base.device,
                DescriptorSetId::DynamicBanner,
                fallback_res,
            );
            self.current_banner_is_fallback = true; // No specific current_banner resource is stored for this, only descriptor updated
            info!("Initialized DynamicBanner descriptor set with Fallback Banner.");
        } else {
            error!("Fallback banner failed to load, DynamicBanner descriptor not initialized with fallback!");
        }
    }

    fn destroy_current_dynamic_banner_resource(&mut self, device: &Device) {
        if !self.current_banner_is_fallback {
            // Only destroy if it was a custom-loaded banner
            if let Some(mut old_banner) = self.current_banner.take() {
                info!(
                    "Destroying previous dynamic banner texture (path key: {:?}).",
                    self.current_banner_path_key
                );
                old_banner.destroy(device);
            }
        }
        // Reset state regardless of whether a resource was destroyed
        self.current_banner = None;
        self.current_banner_is_fallback = true;
        self.current_banner_path_key = None;
    }

    fn load_dynamic_banner_internal(
        &mut self,
        base: &VulkanBase,
        renderer: &Renderer,
        new_banner_path_to_load: Option<&Path>, // The actual path to load the texture from
        new_path_key: Option<PathBuf>, // The key for comparison (could be same as path_to_load or derived)
        banner_type_for_log: &str,
    ) {
        // If the new key matches the current one, and it's not a None key (meaning something is loaded), do nothing.
        if self.current_banner_path_key == new_path_key && new_path_key.is_some() {
            trace!(
                "{} banner for {:?} already loaded and active.",
                banner_type_for_log,
                new_path_key.as_ref().unwrap()
            );
            return;
        }

        // Destroy the old custom banner resource if one existed.
        self.destroy_current_dynamic_banner_resource(&base.device);

        let mut loaded_successfully = false;
        if let Some(banner_path) = new_banner_path_to_load {
            info!(
                "Attempting to load {} banner: {:?}",
                banner_type_for_log, banner_path
            );
            match load_texture(base, banner_path) {
                Ok(new_banner_texture) => {
                    info!(
                        "Successfully loaded {} banner texture: {:?}",
                        banner_type_for_log, banner_path
                    );
                    renderer.update_texture_descriptor(
                        &base.device,
                        DescriptorSetId::DynamicBanner,
                        &new_banner_texture,
                    );
                    self.current_banner = Some(new_banner_texture); // Store the new resource
                    self.current_banner_is_fallback = false;
                    self.current_banner_path_key = new_path_key; // Use the provided key
                    loaded_successfully = true;
                }
                Err(e) => {
                    error!(
                        "Failed to load {} banner texture from {:?}: {}",
                        banner_type_for_log, banner_path, e
                    );
                }
            }
        } else {
            info!("No banner path specified for {}.", banner_type_for_log);
        }

        if !loaded_successfully {
            info!(
                "Using fallback banner for DynamicBanner set after attempting {} banner.",
                banner_type_for_log
            );
            if let Some(fallback_res) = self.textures.get(&TextureId::FallbackBanner) {
                renderer.update_texture_descriptor(
                    &base.device,
                    DescriptorSetId::DynamicBanner,
                    fallback_res,
                );
                // current_banner remains None, current_banner_is_fallback is true (set by destroy_current_dynamic_banner_resource)
                // current_banner_path_key is None (set by destroy_current_dynamic_banner_resource)
            } else {
                error!("Fallback banner resource not found! DynamicBanner set might be invalid.");
            }
        }
    }

    pub fn load_song_banner(
        &mut self,
        base: &VulkanBase,
        renderer: &Renderer,
        song_info: &Arc<SongInfo>,
    ) {
        let banner_path_to_load = song_info.banner_path.as_deref(); // Option<&Path>
        let new_path_key = song_info.banner_path.clone(); // Option<PathBuf> for keying
        self.load_dynamic_banner_internal(
            base,
            renderer,
            banner_path_to_load,
            new_path_key,
            "song",
        );
    }

    pub fn load_pack_banner(
        &mut self,
        base: &VulkanBase,
        renderer: &Renderer,
        pack_banner_path_opt: Option<&Path>,
    ) {
        let banner_path_to_load = pack_banner_path_opt; // Option<&Path>
        let new_path_key = pack_banner_path_opt.map(|p| p.to_path_buf()); // Option<PathBuf> for keying
        self.load_dynamic_banner_internal(
            base,
            renderer,
            banner_path_to_load,
            new_path_key,
            "pack",
        );
    }

    pub fn clear_current_banner(&mut self, device: &Device) {
        self.destroy_current_dynamic_banner_resource(device);
        // App state transition will ensure the DynamicBanner descriptor is updated to fallback if necessary.
        info!(
            "Cleared current dynamic banner resource. Descriptor will be reset to fallback by App."
        );
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
        for (_, mut texture) in self.textures.drain() {
            // drain to take ownership for destroy
            texture.destroy(device);
        }
        info!("Static textures (including explosions and fallback banner) destroyed.");

        self.destroy_current_dynamic_banner_resource(device); // Ensure custom banner is destroyed
        info!("Dynamic banner resource checked/destroyed.");

        for (_, mut font) in self.fonts.drain() {
            // drain for fonts
            font.destroy(device);
        }
        info!("Font resources destroyed.");
        info!("AssetManager resources destroyed.");
    }
}
