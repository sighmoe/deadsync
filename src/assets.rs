use crate::audio::AudioManager; // Assuming AudioManager handles sound loading internally
use crate::config;
use crate::graphics::font::{load_font, Font};
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
    Main, // e.g., Miso font
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SoundId {
    MenuChange,
    MenuStart,
}

// AssetManager holds the loaded assets
pub struct AssetManager {
    // Store resources in HashMaps keyed by their IDs
    textures: HashMap<TextureId, TextureResource>,
    fonts: HashMap<FontId, Font>,
    // Sounds might be managed entirely within AudioManager,
    // but AssetManager could trigger loading.
    // sounds: HashMap<SoundId, SoundResource>, // Or similar
}

impl AssetManager {
    pub fn new() -> Self {
        AssetManager {
            textures: HashMap::new(),
            fonts: HashMap::new(),
        }
    }

    /// Loads all essential assets. Call this during initialization.
    pub fn load_all(
        &mut self,
        base: &VulkanBase, // Needed for texture/font loading
        renderer: &Renderer, // Needed to update descriptor sets
        audio_manager: &mut AudioManager, // Needed to load sounds
    ) -> Result<(), Box<dyn Error>> {
        info!("Loading all assets...");

        // --- Load Textures ---
        info!("Loading textures...");
        let logo_texture = load_texture(base, Path::new(config::LOGO_TEXTURE_PATH))?;
        renderer.update_texture_descriptor(&base.device, DescriptorSetId::Logo, &logo_texture);
        self.textures.insert(TextureId::Logo, logo_texture);

        let dance_texture = load_texture(base, Path::new(config::DANCE_TEXTURE_PATH))?;
        renderer.update_texture_descriptor(&base.device, DescriptorSetId::Dancer, &dance_texture);
        self.textures.insert(TextureId::Dancer, dance_texture);

        let arrow_texture = load_texture(base, Path::new(config::ARROW_TEXTURE_PATH))?;
        // Update the 'Gameplay' descriptor set to use the arrow texture
        renderer.update_texture_descriptor(&base.device, DescriptorSetId::Gameplay, &arrow_texture);
        self.textures.insert(TextureId::Arrows, arrow_texture);
        info!("Textures loaded and descriptor sets updated.");

        // --- Load Fonts ---
        info!("Loading fonts...");
        let main_font = load_font(
            base,
            Path::new(config::FONT_INI_PATH),
            Path::new(config::FONT_TEXTURE_PATH),
        )?;
        // Update the 'Font' descriptor set to use the font's texture
        renderer.update_texture_descriptor(&base.device, DescriptorSetId::Font, &main_font.texture);
        self.fonts.insert(FontId::Main, main_font);
        info!("Fonts loaded and descriptor sets updated.");

        // --- Load Sounds ---
        info!("Loading sounds...");
        audio_manager.load_sfx(SoundId::MenuChange, Path::new(config::SFX_CHANGE_PATH))?;
        audio_manager.load_sfx(SoundId::MenuStart, Path::new(config::SFX_START_PATH))?;
        // Load gameplay sounds here if needed
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

    // `get_sound` might not be needed if AudioManager handles playback internally via ID

    /// Cleans up all loaded assets that need explicit destruction (Vulkan resources).
    pub fn destroy(&mut self, device: &Device) {
        info!("Destroying AssetManager resources...");
        for (_, texture) in self.textures.iter_mut() {
            texture.destroy(device);
        }
        self.textures.clear();
        info!("Textures destroyed.");

        for (_, font) in self.fonts.iter_mut() {
            font.destroy(device); // Font internally destroys its texture
        }
        self.fonts.clear();
        info!("Fonts destroyed.");
        // Sounds might be handled by AudioManager's drop
        info!("AssetManager resources destroyed.");
    }
}