mod core;
mod ui;
mod screens;
mod utils;
mod app;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::builder().filter_level(log::LevelFilter::Info).init();
    app::run()
}
