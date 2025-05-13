use crate::app::App;
use log::{error, info, LevelFilter};
use std::error::Error;
use winit::event_loop::EventLoop;

mod app;
mod assets;
mod audio;
mod config;
mod graphics;
mod parsing;
mod screens;
mod state;
mod utils;

fn main() -> Result<(), Box<dyn Error>> {
    // --- Logging Setup ---
    env_logger::Builder::from_default_env()
        .filter_level(LevelFilter::Info) // Default level
        // Example: Override specific module levels
        .filter_module("deadsync::graphics::vulkan_base", LevelFilter::Warn)
        .filter_module("deadsync::parsing", LevelFilter::Debug) // More detail for parsing
        .filter_module("deadsync::screens", LevelFilter::Debug) // More detail for screens
        .init();

    info!("Application starting...");

    // --- Event Loop Setup ---
    let event_loop = EventLoop::new()?;

    // --- Application Creation ---
    let app = match App::new(&event_loop) {
        Ok(app) => app,
        Err(e) => {
            error!("Failed to initialize application: {}", e);
            return Err(e);
        }
    };

    // --- Run Application ---
    if let Err(e) = app.run(event_loop) {
        error!("Application exited with error: {}", e);
        return Err(e);
    }

    info!("Application exited gracefully.");
    Ok(())
}