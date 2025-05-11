use crate::app::App; // Use crate:: for items within the same crate (binary)
use log::{error, info, LevelFilter};
use std::error::Error;
use winit::event_loop::EventLoop;

mod app;
mod assets;
mod audio;
mod config;
mod graphics;
mod screens;
mod state;
mod utils;

fn main() -> Result<(), Box<dyn Error>> {
    // --- Logging Setup ---
    env_logger::Builder::from_default_env()
        .filter_level(LevelFilter::Info) // Default level
        // Example: Override specific module levels
        .filter_module("deadsync::graphics::vulkan_base", LevelFilter::Warn) // Reduce vulkan spam unless debugging
        .filter_module("deadsync::screens::gameplay", LevelFilter::Debug) // More detail for gameplay
        .init();

    info!("Application starting...");

    // --- Event Loop Setup ---
    // Create event loop before the App, as App might need it for window creation
    let event_loop = EventLoop::new()?; // Create the event loop

    // --- Application Creation ---
    // App::new now handles window creation, Vulkan init, asset loading etc.
    let app = match App::new(&event_loop) {
        Ok(app) => app,
        Err(e) => {
            error!("Failed to initialize application: {}", e);
            // Optionally: Display a message box to the user
            return Err(e); // Return the error to indicate failure
        }
    };

    // --- Run Application ---
    // App::run takes ownership of the event loop and starts the main loop
    if let Err(e) = app.run(event_loop) {
        error!("Application exited with error: {}", e);
        return Err(e);
    }

    info!("Application exited gracefully.");
    Ok(())
}
