pub mod gameplay;
pub mod menu;
pub mod options;
pub mod init;       // NEW

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenAction {
    None,
    Navigate(Screen),
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Menu,
    Gameplay,
    Options,
    Init,            // NEW
}