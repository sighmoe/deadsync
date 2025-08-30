pub mod gameplay;
pub mod menu;
pub mod options;
pub mod init;
pub mod select_color;
pub mod select_music;
use std::path::PathBuf;

use crate::core::song_loading::ChartData;

#[derive(Debug, Clone, PartialEq)]
pub enum ScreenAction {
    None,
    Navigate(Screen),
    Exit,
    RequestBanner(Option<PathBuf>),
    RequestDensityGraph(Option<ChartData>),
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