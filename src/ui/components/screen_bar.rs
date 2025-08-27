use crate::ui::actors::{Actor, SizeSpec, Background};
use crate::act;
use crate::core::space::globals::*;
use crate::ui::color;

// --- Constants (unchanged) ---
const BAR_H: f32 = 32.0;
const TOP_BAR_TITLE_PX: f32 = 27.0;      // top bar (Wendy) — left/center/right
const BOTTOM_BAR_TITLE_PX: f32 = 24.0;   // bottom bar center title
const SIDE_TEXT_PX: f32 = 15.0;          // bottom bar small text
const SIDE_TEXT_MARGIN: f32 = 42.0;      // bottom bar small text margin

// --- NEW: margin for TOP bar's Wendy left/right texts ---
const TOP_WENDY_SIDE_MARGIN: f32 = 9.0; // tweak as you like

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

    match params.position {
        /* ============================== TOP BAR ============================== */
        ScreenBarPosition::Top => {
            // Title (Wendy) — left or centered
            match params.title_placement {
                ScreenBarTitlePlacement::Center => {
                    children.push(act!(text:
                        align(0.5, 0.5):
                        xy(screen_center_x(), 0.5 * BAR_H):
                        zoomtoheight(TOP_BAR_TITLE_PX):
                        z(2):
                        diffuse(params.fg_color[0], params.fg_color[1], params.fg_color[2], params.fg_color[3]):
                        font("wendy"): settext(params.title): horizalign(center)
                    ));
                }
                ScreenBarTitlePlacement::Left => {
                    children.push(act!(text:
                        align(0.0, 0.5):
                        xy(TOP_WENDY_SIDE_MARGIN, 0.5 * BAR_H):
                        zoomtoheight(TOP_BAR_TITLE_PX):
                        z(2):
                        diffuse(params.fg_color[0], params.fg_color[1], params.fg_color[2], params.fg_color[3]):
                        font("wendy"): settext(params.title): horizalign(left)
                    ));
                }
            }

            // Optional Wendy left text (same px + new margin)
            if let Some(text) = params.left_text {
                children.push(act!(text:
                    align(0.0, 0.5):
                    xy(TOP_WENDY_SIDE_MARGIN, 0.5 * BAR_H):
                    zoomtoheight(TOP_BAR_TITLE_PX):
                    z(2):
                    diffuse(params.fg_color[0], params.fg_color[1], params.fg_color[2], params.fg_color[3]):
                    font("wendy"): settext(text): horizalign(left)
                ));
            }

            // Optional Wendy center text
            if let Some(text) = params.center_text {
                children.push(act!(text:
                    align(0.5, 0.5):
                    xy(screen_center_x(), 0.5 * BAR_H):
                    zoomtoheight(TOP_BAR_TITLE_PX):
                    z(2):
                    diffuse(params.fg_color[0], params.fg_color[1], params.fg_color[2], params.fg_color[3]):
                    font("wendy"): settext(text): horizalign(center)
                ));
            }

            // Optional Wendy right text (same px + new margin)
            if let Some(text) = params.right_text {
                children.push(act!(text:
                    align(1.0, 0.5):
                    xy(screen_width() - TOP_WENDY_SIDE_MARGIN, 0.5 * BAR_H):
                    zoomtoheight(TOP_BAR_TITLE_PX):
                    z(2):
                    diffuse(params.fg_color[0], params.fg_color[1], params.fg_color[2], params.fg_color[3]):
                    font("wendy"): settext(text): horizalign(right)
                ));
            }
        }

        /* ============================ BOTTOM BAR ============================ */
        ScreenBarPosition::Bottom => {
            // Center title (Wendy) uses BOTTOM_BAR_TITLE_PX
            children.push(act!(text:
                align(0.5, 0.5):
                xy(screen_center_x(), 0.5 * BAR_H):
                zoomtoheight(BOTTOM_BAR_TITLE_PX):
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
