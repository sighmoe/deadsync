use crate::ui::actors::{self, Actor, SizeSpec, Background};
use crate::act;
use crate::core::space::globals::*;
use crate::core::space;
use crate::ui::color;

// --- Constants ---
const BAR_H: f32 = 32.0;
const SIDE_TEXT_PX: f32 = 15.0;       // bottom bar small text (Miso font)
const SIDE_TEXT_MARGIN: f32 = 42.0;   // bottom bar small text margin

// --- Positioning for the main title on the top bar when left-aligned ---
const TOP_TITLE_OFFSET_X: f32 = 10.0;
const TOP_TITLE_OFFSET_Y: f32 = 15.0;

pub enum ScreenBarPosition {
    Top,
    Bottom,
}

pub enum ScreenBarTitlePlacement {
    Left,
    Center,
}

pub struct ScreenBarParams<'a> {
    pub title: &'a str,
    pub title_placement: ScreenBarTitlePlacement,
    pub position: ScreenBarPosition,
    pub transparent: bool,

    // Optional extra texts:
    // • Top bar: these are rendered in Wendy at TOP_BAR_TITLE_PX.
    // • Bottom bar: these are rendered in Miso at SIDE_TEXT_PX with SIDE_TEXT_MARGIN.
    pub left_text: Option<&'a str>,
    pub center_text: Option<&'a str>,
    pub right_text: Option<&'a str>,

    pub fg_color: [f32; 4], // text color
}

/// Helper to select a scale factor based on screen aspect ratio.
fn wide_scale(normal: f32, wide: f32) -> f32 {
    if space::is_wide() { wide } else { normal }
}

pub fn build(params: ScreenBarParams) -> Actor {
    // Base placement per bar (height & anchor)
    let (align, offset) = match params.position {
        ScreenBarPosition::Top    => ([0.0, 0.0], [0.0, 0.0]),
        ScreenBarPosition::Bottom => ([0.0, 1.0], [0.0, screen_height()]),
    };

    let background = if params.transparent {
        None
    } else {
        Some(Background::Color(color::rgba_hex("#a6a6a6")))
    };

    let mut children = Vec::with_capacity(4);

    // All titles (Wendy font) use the same aspect-ratio-dependent scaling.
    let title_scale = wide_scale(0.5, 0.6);

    match params.position {
        /* ============================== TOP BAR ============================== */
        ScreenBarPosition::Top => {
            // The main title (Wendy font) is the only text on the top bar.
            let (title_align, title_xy, title_horiz_align) = match params.title_placement {
                ScreenBarTitlePlacement::Left => {
                    // Positioned relative to the bar's top-left corner.
                    // The pivot is at the text's vertical center (0.5), matching SM behavior.
                    ([0.0, 0.5], [TOP_TITLE_OFFSET_X, TOP_TITLE_OFFSET_Y], actors::TextAlign::Left)
                }
                ScreenBarTitlePlacement::Center => {
                    // Centered perfectly within the bar.
                    ([0.5, 0.5], [screen_center_x(), 0.5 * BAR_H], actors::TextAlign::Center)
                }
            };

            // Create the actor first without the horizalign, then modify it.
            let mut title_actor = act!(text:
                align(title_align[0], title_align[1]):
                xy(title_xy[0], title_xy[1]):
                zoom(title_scale):
                z(2):
                diffuse(params.fg_color[0], params.fg_color[1], params.fg_color[2], params.fg_color[3]):
                font("wendy"): settext(params.title)
            );

            // Now, apply the alignment from the variable.
            if let Actor::Text { align_text, .. } = &mut title_actor {
                *align_text = title_horiz_align;
            }

            children.push(title_actor);
        }

        /* ============================ BOTTOM BAR ============================ */
        ScreenBarPosition::Bottom => {
            // Center title (Wendy) uses the same scaling as the top bar
            children.push(act!(text:
                align(0.5, 0.5):
                xy(screen_center_x(), 16.0):
                zoom(0.5):
                z(2):
                diffuse(params.fg_color[0], params.fg_color[1], params.fg_color[2], params.fg_color[3]):
                font("wendy"): settext(params.title): horizalign(center)
            ));

            // Small side texts (Miso) at SIDE_TEXT_PX and SIDE_TEXT_MARGIN
            if let Some(text) = params.left_text {
                children.push(act!(text:
                    align(0.0, 0.5):
                    xy(SIDE_TEXT_MARGIN, 0.5 * BAR_H):
                    zoomtoheight(SIDE_TEXT_PX):
                    z(2):
                    diffuse(params.fg_color[0], params.fg_color[1], params.fg_color[2], params.fg_color[3]):
                    font("miso"): settext(text): horizalign(left)
                ));
            }
            if let Some(text) = params.center_text {
                children.push(act!(text:
                    align(0.5, 0.5):
                    xy(screen_center_x(), 0.5 * BAR_H):
                    zoomtoheight(SIDE_TEXT_PX):
                    z(2):
                    diffuse(params.fg_color[0], params.fg_color[1], params.fg_color[2], params.fg_color[3]):
                    font("miso"): settext(text): horizalign(center)
                ));
            }
            if let Some(text) = params.right_text {
                children.push(act!(text:
                    align(1.0, 0.5):
                    xy(screen_width() - SIDE_TEXT_MARGIN, 0.5 * BAR_H):
                    zoomtoheight(SIDE_TEXT_PX):
                    z(2):
                    diffuse(params.fg_color[0], params.fg_color[1], params.fg_color[2], params.fg_color[3]):
                    font("miso"): settext(text): horizalign(right)
                ));
            }
        }
    }

    Actor::Frame {
        align,
        offset,
        size:   [SizeSpec::Fill, SizeSpec::Px(BAR_H)],
        children,
        background,
        z: 120i16,
    }
}
