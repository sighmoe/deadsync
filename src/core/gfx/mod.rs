mod backends;

use crate::core::gfx::backends::{opengl, vulkan};
use cgmath::Matrix4;
use glow::HasContext;
use image::RgbaImage;
use std::{collections::HashMap, error::Error, str::FromStr, sync::Arc};
use winit::window::Window;

// --- Public Data Contract ---
#[derive(Clone)]
pub struct RenderList {
    pub clear_color: [f32; 4],
    pub objects: Vec<RenderObject>,
}
#[derive(Clone)]
pub struct RenderObject {
    pub object_type: ObjectType,
    pub transform: Matrix4<f32>,
    pub blend: BlendMode,
    pub z: i16,
    pub order: u32,
}
#[derive(Clone)]
pub enum ObjectType {
    Sprite {
        texture_id: String,
        tint: [f32; 4],
        uv_scale: [f32; 2],
        uv_offset: [f32; 2],
        edge_fade: [f32; 4],
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendMode {
    Alpha,
    Add,
    #[allow(dead_code)]
    Multiply,
    #[allow(dead_code)]
    Subtract,
}

// --- Public API Facade ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    Vulkan,
    OpenGL,
}

// Texture goes back to being an enum holding the concrete backend texture type.
pub enum Texture {
    Vulkan(vulkan::Texture),
    OpenGL(opengl::Texture),
}

// Backend is an enum, not a trait object.
pub enum Backend {
    Vulkan(vulkan::State),
    OpenGL(opengl::State),
}

// This is the core of the enum dispatch pattern. All backend-related logic is
// centralized here, dispatched via a single `match`.
impl Backend {
    pub fn draw(
        &mut self,
        render_list: &RenderList,
        textures: &HashMap<String, Texture>,
    ) -> Result<u32, Box<dyn Error>> {
        match self {
            Backend::Vulkan(state) => vulkan::draw(state, render_list, textures),
            Backend::OpenGL(state) => opengl::draw(state, render_list, textures),
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        match self {
            Backend::Vulkan(state) => vulkan::resize(state, width, height),
            Backend::OpenGL(state) => opengl::resize(state, width, height),
        }
    }

    pub fn cleanup(&mut self) {
        match self {
            Backend::Vulkan(state) => vulkan::cleanup(state),
            Backend::OpenGL(state) => opengl::cleanup(state),
        }
    }

    pub fn create_texture(&mut self, image: &RgbaImage) -> Result<Texture, Box<dyn Error>> {
        match self {
            Backend::Vulkan(state) => {
                let tex = vulkan::create_texture(state, image)?;
                Ok(Texture::Vulkan(tex))
            }
            Backend::OpenGL(state) => {
                let tex = opengl::create_texture(&state.gl, image)?;
                Ok(Texture::OpenGL(tex))
            }
        }
    }

    pub fn dispose_textures(&mut self, textures: &mut HashMap<String, Texture>) {
        let old_textures = std::mem::take(textures);
        match self {
            Backend::Vulkan(_) => {
                // For Vulkan, the texture's Drop implementation handles cleanup.
                drop(old_textures);
            }
            Backend::OpenGL(state) => unsafe {
                for tex in old_textures.values() {
                    if let Texture::OpenGL(opengl::Texture(handle)) = tex {
                        state.gl.delete_texture(*handle);
                    }
                }
            },
        }
    }

    pub fn wait_for_idle(&mut self) {
        match self {
            Backend::Vulkan(state) => {
                if let Some(device) = &state.device {
                    unsafe {
                        let _ = device.device_wait_idle();
                    }
                }
            }
            Backend::OpenGL(_) => {
                // OpenGL does not have a direct equivalent, so this is a no-op.
            }
        }
    }
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

// -- Boilerplate impls --
impl core::fmt::Display for BackendType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Vulkan => write!(f, "Vulkan"),
            Self::OpenGL => write!(f, "OpenGL"),
        }
    }
}
impl FromStr for BackendType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "vulkan" => Ok(BackendType::Vulkan),
            "opengl" => Ok(BackendType::OpenGL),
            _ => Err(format!("'{}' is not a valid video renderer", s)),
        }
    }
}
