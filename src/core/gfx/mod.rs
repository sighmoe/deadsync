mod backends;

use crate::core::gfx::backends::{opengl, vulkan};
use cgmath::Matrix4;
use glow::HasContext;
use image::RgbaImage;
use std::{collections::HashMap, error::Error, sync::Arc};
use winit::window::Window;

// --- Public Data Contract ---

/// A simple container for all objects to be drawn in a single frame.
#[derive(Clone)]
pub struct RenderList {
    pub clear_color: [f32; 4],
    pub objects: Vec<RenderObject>,
}

/// The simplest possible representation of a single item to be drawn by the GPU.
#[derive(Clone)]
pub struct RenderObject {
    pub object_type: ObjectType,
    pub transform: Matrix4<f32>,
    pub blend: BlendMode,
    pub z: i16,
    pub order: u32, // for stable sorting
}

/// Defines the type of primitive to be rendered.
#[derive(Clone)]
pub enum ObjectType {
    Sprite {
        texture_id: &'static str,
        tint: [f32; 4],
        uv_scale: [f32; 2],
        uv_offset: [f32; 2],
        edge_fade: [f32; 4],
    },
    MsdfGlyph {
        texture_id: &'static str,
        uv_scale: [f32; 2],
        uv_offset: [f32; 2],
        color: [f32; 4],
        px_range: f32,
    },
}

/// Specifies how an object's color should be blended with the background.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendMode {
    Alpha,
    Add,
    Multiply,
    Subtract,
}

// --- Public API Facade ---

/// Identifies which graphics backend to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    Vulkan,
    OpenGL,
}

/// An opaque handle to a texture managed by the active backend.
pub enum Texture {
    Vulkan(vulkan::Texture),
    OpenGL(opengl::Texture),
}

/// An opaque handle to the active graphics backend state.
pub enum Backend {
    Vulkan(vulkan::State),
    OpenGL(opengl::State),
}

/// Specifies the color space of a texture's data.
pub enum TextureColorSpace {
    Srgb,
    Linear,
}

/// Creates and initializes a new graphics backend.
pub fn create_backend(
    backend_type: BackendType,
    window: Arc<Window>,
    vsync_enabled: bool,
) -> Result<Backend, Box<dyn Error>> {
    match backend_type {
        BackendType::Vulkan => Ok(Backend::Vulkan(vulkan::init(&window, vsync_enabled)?)),
        BackendType::OpenGL => Ok(Backend::OpenGL(opengl::init(window, vsync_enabled)?)),
    }
}

/// Creates a new GPU texture from raw image data.
pub fn create_texture(
    backend: &mut Backend,
    image: &RgbaImage,
    cs: TextureColorSpace,
) -> Result<Texture, Box<dyn Error>> {
    let use_srgb = matches!(cs, TextureColorSpace::Srgb);
    match backend {
        Backend::Vulkan(state) => {
            let tex = vulkan::create_texture(state, image, use_srgb)?;
            Ok(Texture::Vulkan(tex))
        }
        Backend::OpenGL(state) => {
            let tex = opengl::create_texture(&state.gl, image, use_srgb)?;
            Ok(Texture::OpenGL(tex))
        }
    }
}

/// Disposes of all textures currently in the texture manager.
pub fn dispose_textures(backend: &mut Backend, textures: &mut HashMap<&'static str, Texture>) {
    match backend {
        Backend::Vulkan(state) => {
            unsafe {
                if let Some(device) = &state.device {
                    let _ = device.device_wait_idle();
                }
            }
        }
        Backend::OpenGL(state) => {
            unsafe {
                for tex in textures.values() {
                    if let Texture::OpenGL(opengl::Texture(handle)) = tex {
                        state.gl.delete_texture(*handle);
                    }
                }
            }
        }
    }
    textures.clear();
}

/// Draws a single frame to the screen using the provided `RenderList`.
pub fn draw(
    backend: &mut Backend,
    render_list: &RenderList,
    textures: &HashMap<&'static str, Texture>,
) -> Result<u32, Box<dyn Error>> {
    match backend {
        Backend::Vulkan(state) => vulkan::draw(state, render_list, textures),
        Backend::OpenGL(state) => opengl::draw(state, render_list, textures),
    }
}

/// Notifies the backend that the window has been resized.
pub fn resize(backend: &mut Backend, width: u32, height: u32) {
    match backend {
        Backend::Vulkan(state) => vulkan::resize(state, width, height),
        Backend::OpenGL(state) => opengl::resize(state, width, height),
    }
}

/// Cleans up all resources associated with the graphics backend.
pub fn cleanup(backend: &mut Backend) {
    match backend {
        Backend::Vulkan(state) => vulkan::cleanup(state),
        Backend::OpenGL(state) => opengl::cleanup(state),
    }
}

impl core::fmt::Display for BackendType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self { Self::Vulkan => write!(f, "Vulkan"), Self::OpenGL => write!(f, "OpenGL") }
    }
}