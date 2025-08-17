// src/ui/components/logo.rs
use crate::ui::actors::{Actor, Anchor, SizeSpec};
use crate::sprite;

/// Native image sizes (only used for aspect).
const LOGO_NATIVE_W: f32 = 752.0;
const LOGO_NATIVE_H: f32 = 634.0;
const DANCE_NATIVE_W: f32 = 1360.0;
const DANCE_NATIVE_H: f32 = 164.0;

/// Parameters to tweak the layout easily.
#[derive(Clone, Copy, Debug)]
pub struct LogoParams {
    pub target_h: f32,
    pub top_margin: f32,
    /// Positive values move the banner *up* inside the logo.
    pub banner_y_offset_inside: f32,
}

impl Default for LogoParams {
    fn default() -> Self {
        Self { target_h: 238.0, top_margin: 102.0, banner_y_offset_inside: 0.0 }
    }
}

/// Build the “banner inside logo” stack with the actor DSL.
/// Returns a `Vec<Actor>` to be included in a screen's actor list.
pub fn build_logo(params: LogoParams, screen_width: f32) -> Vec<Actor> {
    // Logo size from target height
    let logo_aspect = LOGO_NATIVE_W / LOGO_NATIVE_H;
    let logo_h = params.target_h;
    let logo_w = logo_h * logo_aspect;

    // Center horizontally in SM top-left space
    let logo_x_tl = 0.5 * (screen_width - logo_w);
    let logo_y_tl = params.top_margin;

    // Banner (same width as logo, centered inside)
    let dance_aspect = DANCE_NATIVE_W / DANCE_NATIVE_H;
    let dance_w = logo_w;
    let dance_h = dance_w / dance_aspect;
    let dance_x_tl = logo_x_tl;
    let dance_y_tl = logo_y_tl + 0.5 * (logo_h - dance_h) - params.banner_y_offset_inside;

    vec![
        sprite! {
            anchor: Anchor::TopLeft,
            offset: [dance_x_tl, dance_y_tl],
            size:   [SizeSpec::Px(dance_w), SizeSpec::Px(dance_h)],
            texture:"dance.png",
        },
        sprite! {
            anchor: Anchor::TopLeft,
            offset: [logo_x_tl,  logo_y_tl ],
            size:   [SizeSpec::Px(logo_w), SizeSpec::Px(logo_h)],
            texture:"logo.png",
        },
    ]
}

/// Convenience: build with default params.
pub fn build_logo_default(screen_width: f32) -> Vec<Actor> {
    build_logo(LogoParams::default(), screen_width)
}
