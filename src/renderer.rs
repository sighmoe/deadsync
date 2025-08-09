use crate::{
    core::{opengl, vulkan},
    screen::Screen,
};
use glow::HasContext; // <--- ADD THIS LINE
use image::RgbaImage;
use std::{collections::HashMap, error::Error, sync::Arc};
use winit::window::Window;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    Vulkan,
    OpenGL,
}

pub enum Texture {
    Vulkan(vulkan::Texture),
    OpenGL(opengl::Texture),
}

pub enum Backend {
    Vulkan(vulkan::State),
    OpenGL(opengl::State),
}

pub fn create_backend(
    backend_type: BackendType,
    window: Arc<Window>,
    screen: &Screen,
    vsync_enabled: bool,
) -> Result<Backend, Box<dyn Error>> {
    match backend_type {
        BackendType::Vulkan => Ok(Backend::Vulkan(vulkan::init(&window, screen, vsync_enabled)?)),
        BackendType::OpenGL => Ok(Backend::OpenGL(opengl::init(window.clone(), screen, vsync_enabled)?)),
    }
}

pub fn create_texture(
    backend: &mut Backend,
    image: &RgbaImage,
) -> Result<Texture, Box<dyn Error>> {
    match backend {
        Backend::Vulkan(state) => {
            let texture = vulkan::create_texture(state, image)?;
            Ok(Texture::Vulkan(texture))
        }
        Backend::OpenGL(state) => {
            let texture = opengl::create_texture(&state.gl, image)?;
            Ok(Texture::OpenGL(texture))
        }
    }
}

pub fn dispose_textures(backend: &mut Backend, textures: &mut HashMap<&'static str, Texture>) {
    match backend {
        Backend::Vulkan(state) => {
            unsafe {
                if let Some(device) = &state.device {
                    device.device_wait_idle().unwrap();
                }
            }
        }
        Backend::OpenGL(state) => {
            unsafe {
                for tex in textures.values() {
                    if let Texture::OpenGL(opengl::Texture(handle)) = tex {
                        // This now compiles because `HasContext` is in scope
                        state.gl.delete_texture(*handle);
                    }
                }
            }
        }
    }
    textures.clear();
}

pub fn load_screen(backend: &mut Backend, screen: &Screen) -> Result<(), Box<dyn Error>> {
    match backend {
        Backend::Vulkan(state) => vulkan::load_screen(state, screen),
        Backend::OpenGL(state) => opengl::load_screen(state, screen),
    }
}

pub fn draw(
    backend: &mut Backend,
    screen: &Screen,
    textures: &HashMap<&'static str, Texture>,
) -> Result<(), Box<dyn Error>> {
    match backend {
        Backend::Vulkan(state) => vulkan::draw(state, screen, textures),
        Backend::OpenGL(state) => opengl::draw(state, screen, textures),
    }
}

pub fn resize(backend: &mut Backend, width: u32, height: u32) {
    match backend {
        Backend::Vulkan(state) => vulkan::resize(state, width, height),
        Backend::OpenGL(state) => opengl::resize(state, width, height),
    }
}

pub fn cleanup(backend: &mut Backend) {
    match backend {
        Backend::Vulkan(state) => vulkan::cleanup(state),
        Backend::OpenGL(state) => opengl::cleanup(state),
    }
}