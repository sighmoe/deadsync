mod core;
mod ui;
mod screens;
mod app;
mod config;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    config::load();
    // env_logger is initialized in app::run()
    app::run()
}
