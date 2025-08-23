use crate::ui::actors::{Actor, SizeSpec, Background};
use crate::act;
use crate::core::space::globals::*;
use crate::ui::color;

// --- Constants ---
const BAR_H: f32 = 32.0;
const TOP_BAR_TITLE_PX: f32 = 33.0;
const BOTTOM_BAR_TITLE_PX: f32 = 28.0;
const SIDE_TEXT_PX: f32 = 15.0;
const SIDE_TEXT_MARGIN: f32 = 42.0;

pub enum ScreenBarPosition {
    Top,
    Bottom,
}

pub struct ScreenBarParams<'a> {
    pub title: &'a str,
    pub position: ScreenBarPosition,
    pub transparent: bool,
    pub left_text: Option<&'a str>,
    pub right_text: Option<&'a str>,
    pub fg_color: [f32; 4], // New field for text color
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
        Some(Background::Color(color::rgba_hex("#a6a6a6")))
    };

    let mut children = vec![
        act!(text:
            align(0.5, 0.5):
            xy(screen_center_x(), 0.5 * BAR_H):
            px(title_px):
            diffuse(params.fg_color[0], params.fg_color[1], params.fg_color[2], params.fg_color[3]):
            font("wendy"): text(params.title): talign(center)
        )
    ];

    if let Some(text) = params.left_text {
        children.push(act!(text:
            align(0.0, 0.5):
            xy(SIDE_TEXT_MARGIN, 0.5 * BAR_H):
            px(SIDE_TEXT_PX):
            diffuse(params.fg_color[0], params.fg_color[1], params.fg_color[2], params.fg_color[3]):
            font("miso"): text(text): talign(left)
        ));
    }

    if let Some(text) = params.right_text {
        children.push(act!(text:
            align(1.0, 0.5):
            xy(screen_width() - SIDE_TEXT_MARGIN, 0.5 * BAR_H):
            px(SIDE_TEXT_PX):
            diffuse(params.fg_color[0], params.fg_color[1], params.fg_color[2], params.fg_color[3]):
            font("miso"): text(text): talign(right)
        ));
    }

    Actor::Frame {
        align,
        offset,
        size:   [SizeSpec::Fill, SizeSpec::Px(BAR_H)],
        children,
        background,
        z: 0i16,
    }
}
