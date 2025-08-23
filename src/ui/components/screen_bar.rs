use crate::ui::actors::{Actor, SizeSpec, Background};
use crate::act;
use crate::core::space::globals::*;
use crate::ui::color; // Import the color module

// Define constants for clarity
const BAR_H: f32 = 32.0;
const TOP_BAR_TITLE_PX: f32 = 33.0;
const BOTTOM_BAR_TITLE_PX: f32 = 28.0;
// BG_COLOR is now defined inside the build function
const FG_COLOR: [f32; 4] = [1.00, 1.00, 1.00, 1.0];

pub enum ScreenBarPosition {
    Top,
    Bottom,
}

pub struct ScreenBarParams<'a> {
    pub title: &'a str,
    pub position: ScreenBarPosition,
    pub transparent: bool,
}

pub fn build(params: ScreenBarParams) -> Actor {
    // Determine bar's alignment and title font size based on position
    let (align, offset, title_px) = match params.position {
        ScreenBarPosition::Top => ([0.0, 0.0], [0.0, 0.0], TOP_BAR_TITLE_PX),
        ScreenBarPosition::Bottom => ([0.0, 1.0], [0.0, screen_height()], BOTTOM_BAR_TITLE_PX),
    };

    let background = if params.transparent {
        None
    } else {
        // Use the rgba_hex function to define the background color
        Some(Background::Color(color::rgba_hex("#a6a6a6")))
    };

    Actor::Frame {
        align,
        offset,
        size:   [SizeSpec::Fill, SizeSpec::Px(BAR_H)],
        children: vec![
            act!(text:
                align(0.5, 0.5):
                xy(screen_center_x(), 0.5 * BAR_H):
                px(title_px): // Use the selected font size here
                diffuse(FG_COLOR[0], FG_COLOR[1], FG_COLOR[2], FG_COLOR[3]):
                font("wendy"): text(params.title): talign(center)
            )
        ],
        background,
        z: 0i16,
    }
}
