pub mod gameplay;
pub mod menu;
pub mod options;

use winit::event::KeyEvent;

// An action that a screen can return after an update or input event.
// This tells the main loop what to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenAction {
    None,
    Navigate(Screen),
    Exit,
}

// An enum to identify each screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Menu,
    Gameplay,
    Options,
}

// A common trait that all screens could implement, though for this procedural
// approach, we'll use free functions in each module instead.
pub trait ScreenState {
    fn handle_input(&mut self, event: &KeyEvent) -> ScreenAction;
    fn update(&mut self, delta_time: f32, input: &crate::input::InputState) -> ScreenAction;
    fn get_ui_elements(&self) -> (Vec<crate::api::UIElement>, [f32; 4]);
}