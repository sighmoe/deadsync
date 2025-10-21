use std::{collections::HashMap, error::Error};

use image::RgbaImage;

use crate::core::gfx::RenderList;


#[cfg(not(target_os = "macos"))]
pub mod opengl;

#[cfg(not(target_os = "macos"))]
pub mod vulkan;

/// An opaque identifier for textures.
/// 
/// Each backend should maintain an internal collection of this id to its specific collection of textures.
pub struct Texture(pub(crate) u64);

/// The rendering backend, and any associated state for performing rendering operations with that backend.
pub trait Backend {
    fn create_texture(&mut self, image: &RgbaImage) -> Result<Texture, Box<dyn Error>>;

    fn drop_textures(&mut self, textures: &mut dyn Iterator<Item = (String, Texture)>) -> Result<(), Box<dyn Error>>;

    fn draw(&mut self, render_list: &RenderList, textures: &HashMap<String, Texture>) -> Result<u32, Box<dyn Error>>;

    fn resize(&mut self, width: u32, height: u32);

    fn cleanup(&mut self);

    fn wait_for_idle(&mut self);
}

