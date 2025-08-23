use crate::ui::actors::{Actor, SizeSpec, Background};
use crate::act;
use crate::core::space::globals::*;

/// A full-width bar anchored to the top of the screen, with a centered title.
const BAR_H: f32 = 50.0;
const TITLE_PX: f32 = 40.0;
const BG_COLOR: [f32; 4] = [0.15, 0.15, 0.18, 1.0];
const FG_COLOR: [f32; 4] = [0.90, 0.90, 1.00, 1.0];

pub enum BarPosition {
    Top,
    Bottom,
}

pub struct BarParams<'a> {
    pub title: &'a str,
    pub position: BarPosition,
    pub transparent: bool,
}

pub fn build(params: BarParams) -> Actor {
    let (align, offset) = match params.position {
        BarPosition::Top => ([0.0, 0.0], [0.0, 0.0]),
        BarPosition::Bottom => ([0.0, 1.0], [0.0, screen_height()]),
    };

    let background = if params.transparent {
        None
    } else {
        Some(Background::Color(BG_COLOR))
    };

    Actor::Frame {
        align,
        offset,
        size:   [SizeSpec::Fill, SizeSpec::Px(BAR_H)],
        children: vec![
            act!(text:
                align(0.5, 0.5):
                xy(screen_center_x(), 0.5 * BAR_H):
                px(TITLE_PX):
                diffuse(FG_COLOR[0], FG_COLOR[1], FG_COLOR[2], FG_COLOR[3]):
                font("wendy"): text(params.title): talign(center)
            )
        ],
        background,
        z: 0i16,
    }
}