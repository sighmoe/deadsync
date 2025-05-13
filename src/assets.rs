use crate::audio::AudioManager;
use crate::config;
use crate::graphics::font::{load_font, Font, LoadedFontData};
use crate::graphics::renderer::{DescriptorSetId, Renderer};
use crate::graphics::texture::{load_texture, TextureResource};
use crate::graphics::vulkan_base::VulkanBase;
use crate::parsing::simfile::SongInfo; // <-- Import SongInfo
use ash::Device;
use log::{error, info, warn};
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

// TextureId (keep as is)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureId {
    Logo,
    Dancer,
    Arrows,
    FallbackBanner,
    // No need for CurrentSongBanner here, managed dynamically
}

// FontId, SoundId (keep as is)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum FontId { Wendy, Miso, Cjk }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] pub enum SoundId { MenuChange, MenuStart }


pub struct AssetManager {
    textures: HashMap<TextureId, TextureResource>,
    fonts: HashMap<FontId, Font>,
    // Store the dynamically loaded banner resource
    current_banner: Option<TextureResource>,
    // Flag to know if current_banner is *actually* the fallback banner
    // This prevents us from trying to destroy the fallback banner
    current_banner_is_fallback: bool,
}

impl AssetManager {
    pub fn new() -> Self {
        AssetManager {
            textures: HashMap::new(),
            fonts: HashMap::new(),
            current_banner: None,
            current_banner_is_fallback: true, // Start assuming fallback
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

        // Load Fallback Banner
        let fallback_banner_texture = load_texture(base, Path::new("assets/graphics/fallback_banner.png"))?;
        renderer.update_texture_descriptor(&base.device, DescriptorSetId::FallbackBanner, &fallback_banner_texture);
        // Don't store the resource itself in current_banner yet, just load it.
        self.textures.insert(TextureId::FallbackBanner, fallback_banner_texture);
        info!("Fallback banner texture loaded and its static descriptor set updated.");

        // --- Initialize Dynamic Banner Descriptor with Fallback ---
        if let Some(fallback_res) = self.textures.get(&TextureId::FallbackBanner) {
             renderer.update_texture_descriptor(
                 &base.device,
                 DescriptorSetId::DynamicBanner, // Update the DYNAMIC set
                 fallback_res,
             );
             self.current_banner_is_fallback = true; // Explicitly set
             info!("Initialized DynamicBanner descriptor set with Fallback Banner.");
        } else {
            error!("Fallback banner failed to load, DynamicBanner descriptor not initialized!");
            // Handle this error appropriately, maybe panic or use solid color?
        }

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

    /// Loads the banner for the given song info, updating the DynamicBanner descriptor set.
    /// Unloads the previous dynamic banner if it wasn't the fallback.
    pub fn load_song_banner(
        &mut self,
        base: &VulkanBase,
        renderer: &Renderer, // Need renderer to update descriptor
        song_info: &SongInfo,
    ) {
        info!("Attempting to load banner for song: {}", song_info.title);

        // --- Clean up previous dynamic banner (if it wasn't the fallback) ---
        if !self.current_banner_is_fallback {
            if let Some(mut old_banner) = self.current_banner.take() {
                info!("Destroying previous dynamic banner texture.");
                old_banner.destroy(&base.device);
            }
        }
        // Reset banner state before loading new one
        self.current_banner = None;
        self.current_banner_is_fallback = false; // Assume not fallback initially

        // --- Try loading the song-specific banner ---
        let mut loaded_successfully = false;
        if let Some(banner_path) = &song_info.banner_path {
            info!("Found banner path: {:?}", banner_path);
            match load_texture(base, banner_path) {
                Ok(new_banner_texture) => {
                    info!("Successfully loaded banner texture: {:?}", banner_path);
                    renderer.update_texture_descriptor(
                        &base.device,
                        DescriptorSetId::DynamicBanner, // Update the DYNAMIC set
                        &new_banner_texture,
                    );
                    self.current_banner = Some(new_banner_texture); // Store the new resource
                    self.current_banner_is_fallback = false;
                    loaded_successfully = true;
                }
                Err(e) => {
                    error!(
                        "Failed to load banner texture from {:?}: {}",
                        banner_path, e
                    );
                    // Proceed to fallback
                }
            }
        } else {
            info!("No banner path specified for song: {}", song_info.title);
            // Proceed to fallback
        }

        // --- Use Fallback if loading failed or no path was specified ---
        if !loaded_successfully {
            info!("Using fallback banner.");
            if let Some(fallback_res) = self.textures.get(&TextureId::FallbackBanner) {
                 renderer.update_texture_descriptor(
                     &base.device,
                     DescriptorSetId::DynamicBanner, // Update the DYNAMIC set
                     fallback_res,
                 );
                 // Don't store the fallback resource in self.current_banner,
                 // as it's managed by self.textures map.
                 self.current_banner = None;
                 self.current_banner_is_fallback = true;
            } else {
                // This should ideally not happen if load_all succeeded
                error!("Fallback banner resource not found in textures map!");
                // Leave DynamicBanner descriptor potentially pointing to invalid/old texture?
                // Or point it to solid white?
                // For now, just log the error. The renderer might crash or show garbage.
                self.current_banner_is_fallback = true; // Treat as fallback state anyway
            }
        }
    }

    // --- Accessor Methods ---

    pub fn get_texture(&self, id: TextureId) -> Option<&TextureResource> {
        self.textures.get(&id)
    }

    pub fn get_font(&self, id: FontId) -> Option<&Font> {
        self.fonts.get(&id)
    }

    // --- Destroy ---
    pub fn destroy(&mut self, device: &Device) {
        info!("Destroying AssetManager resources...");

        // Destroy static textures
        for (_, texture) in self.textures.iter_mut() {
            texture.destroy(device);
        }
        self.textures.clear();
        info!("Static textures destroyed.");

        // Destroy dynamic banner if it exists and is not the fallback
        if !self.current_banner_is_fallback {
            if let Some(mut banner) = self.current_banner.take() {
                info!("Destroying dynamic banner texture.");
                banner.destroy(device);
            }
        }
        info!("Dynamic banner resources checked/destroyed.");


        // Destroy fonts (which destroy their textures)
        for (_, font) in self.fonts.iter_mut() {
            font.destroy(device);
        }
        self.fonts.clear();
        info!("Font resources destroyed.");
        info!("AssetManager resources destroyed.");
    }
}
