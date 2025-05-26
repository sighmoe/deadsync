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
    // Consider making this configurable via command-line arguments or a config file in the future.
    env_logger::Builder::from_default_env()
        .filter_level(LevelFilter::Info) // Default level
        .filter_module("deadsync::graphics::vulkan_base", LevelFilter::Warn) // Reduce Vulkan spam unless debugging
        .filter_module("deadsync::parsing", LevelFilter::Debug)
        .filter_module("deadsync::screens", LevelFilter::Debug)
        .filter_module("deadsync::audio", LevelFilter::Info) // Set audio to Info by default
        .init();

    info!("Application starting...");

    // --- Event Loop Setup ---
    // EventLoop::new() can return an error, handled by `?`
    let event_loop = EventLoop::new()?;

    // --- Application Creation ---
    let app = match App::new(&event_loop) {
        Ok(app) => app,
        Err(e) => {
            error!("Failed to initialize application: {}", e);
            // Consider more specific error handling or cleanup if partially initialized resources exist.
            return Err(e);
        }
    };

    // --- Run Application ---
    // The app.run() method encapsulates the main loop and its error handling.
    if let Err(e) = app.run(event_loop) {
        error!("Application exited with error: {}", e);
        return Err(e); // Propagate the error
    }

    info!("Application exited gracefully.");
    Ok(())
}
