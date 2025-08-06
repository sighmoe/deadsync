use crate::{core::{vulkan, opengl}, screen::Screen};
use std::{error::Error, sync::Arc};
use winit::window::Window;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendType {
    Vulkan,
    OpenGL,
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
        BackendType::Vulkan => {
            let state = vulkan::init(&window, screen)?;
            Ok(Backend::Vulkan(state))
        }
        BackendType::OpenGL => {
            let state = opengl::init(window.clone(), screen)?;
            Ok(Backend::OpenGL(state))
        }
    }
}

pub fn draw(backend: &mut Backend, screen: &Screen) -> Result<(), Box<dyn Error>> {
    match backend {
        Backend::Vulkan(state) => vulkan::draw(state, screen),
        Backend::OpenGL(state) => opengl::draw(state, screen),
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