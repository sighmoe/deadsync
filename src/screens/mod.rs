pub mod gameplay;
pub mod menu;
pub mod options;
pub mod init;
pub mod select_color;
pub mod select_music;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScreenAction {
    None,
    Navigate(Screen),
    Exit,
    RequestBanner(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Menu,
    Gameplay,
    Options,
    Init,
    SelectColor,
    SelectMusic,
}