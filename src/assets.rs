use crate::audio::AudioManager; // Assuming AudioManager handles sound loading internally
use crate::config;
use crate::graphics::font::{load_font, Font, LoadedFontData};
use crate::graphics::renderer::{DescriptorSetId, Renderer}; // Need Renderer to update descriptors
use crate::graphics::texture::{load_texture, TextureResource};
use crate::graphics::vulkan_base::VulkanBase;
use ash::Device;
use log::{info};
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

// Use specific IDs for assets instead of strings for type safety and clarity
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureId {
    Logo,
    Dancer,
    Arrows,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontId {
    Wendy, // Wendy (MSDF)
    Miso, // Miso (MSDF)
    Cjk,  // For the comprehensive Noto Sans CJK font
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SoundId {
    MenuChange,
    MenuStart,
}

// AssetManager holds the loaded assets
pub struct AssetManager {
    textures: HashMap<TextureId, TextureResource>,
    fonts: HashMap<FontId, Font>, // Stores the final Font struct
}

impl AssetManager {
    pub fn new() -> Self {
        AssetManager {
            textures: HashMap::new(),
            fonts: HashMap::new(),
        }
    }

    pub fn load_all(
        &mut self,
        base: &VulkanBase,
        renderer: &Renderer,
        audio_manager: &mut AudioManager,
    ) -> Result<(), Box<dyn Error>> {
        info!("Loading all assets...");

        // --- Load Textures (Non-Font) ---
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
        info!("Non-font textures loaded and descriptor sets updated.");

        // --- Load MSDF Fonts ---
        info!("Loading MSDF fonts...");

        // Load Wendy Font (Main) using MSDF
        let wendy_loaded_data: LoadedFontData = load_font(
            base,
            Path::new(config::WENDY_MSDF_JSON_PATH), // Wendy MSDF JSON
            Path::new(config::WENDY_MSDF_TEXTURE_PATH), // Wendy MSDF Texture
        )?;
        // The renderer needs to know which descriptor set to use for this font's texture.
        // Assuming FontWendy is still the correct ID.
        renderer.update_texture_descriptor(&base.device, DescriptorSetId::FontWendy, &wendy_loaded_data.texture);
        let wendy_font = Font {
            metrics: wendy_loaded_data.metrics,
            glyphs: wendy_loaded_data.glyphs,
            texture: wendy_loaded_data.texture,
            space_width: wendy_loaded_data.space_width,
            descriptor_set_id: DescriptorSetId::FontWendy, // Store which descriptor set this font uses
        };
        self.fonts.insert(FontId::Wendy, wendy_font);
        info!("Wendy MSDF font (Wendy) loaded and descriptor set updated.");

        // Load Miso Font (Secondary) using MSDF
        let miso_loaded_data: LoadedFontData = load_font(
            base,
            Path::new(config::MISO_MSDF_JSON_PATH), // Miso MSDF JSON
            Path::new(config::MISO_MSDF_TEXTURE_PATH), // Miso MSDF Texture
        )?;
        renderer.update_texture_descriptor(&base.device, DescriptorSetId::FontMiso, &miso_loaded_data.texture);
        let miso_font = Font {
            metrics: miso_loaded_data.metrics,
            glyphs: miso_loaded_data.glyphs,
            texture: miso_loaded_data.texture,
            space_width: miso_loaded_data.space_width,
            descriptor_set_id: DescriptorSetId::FontMiso, // Store which descriptor set this font uses
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

    // --- Accessor Methods ---

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
        info!("Non-font textures destroyed.");

        for (_, font) in self.fonts.iter_mut() {
            font.destroy(device); // Font internally destroys its texture
        }
        self.fonts.clear();
        info!("Font resources destroyed.");
        info!("AssetManager resources destroyed.");
    }
}