use crate::{core::{opengl, vulkan}, screen::Screen};
use image::RgbaImage;
use std::{collections::HashMap, error::Error, sync::Arc};
use winit::window::Window;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    Vulkan,
    OpenGL,
}

// NEW: Texture abstractions
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
) -> Result<Backend, Box<dyn Error>> {
    match backend_type {
        BackendType::Vulkan => Ok(Backend::Vulkan(vulkan::init(&window, screen)?)),
        BackendType::OpenGL => Ok(Backend::OpenGL(opengl::init(window.clone(), screen)?)),
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

pub fn load_screen(backend: &mut Backend, screen: &Screen) -> Result<(), Box<dyn Error>> {
    match backend {
        Backend::Vulkan(state) => vulkan::load_screen(state, screen),
        Backend::OpenGL(state) => opengl::load_screen(state, screen),
    }
}

pub fn draw(
    backend: &mut Backend,
    screen: &Screen,
    textures: &HashMap<String, Texture>,
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

pub fn cleanup(backend: &mut Backend, textures: &mut HashMap<String, Texture>) {
    match backend {
        Backend::Vulkan(state) => vulkan::cleanup(state, textures),
        Backend::OpenGL(state) => {
            // OpenGL textures are also managed by the backend state, clearing the map is correct.
            textures.clear();
            opengl::cleanup(state);
        }
    }
}