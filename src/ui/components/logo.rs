// src/ui/components/logo.rs
use crate::core::space::Metrics;
use crate::ui::primitives::UIElement;
use crate::ui::actors::{self}; // build_actors(...)
use crate::sprite;              // brings the sprite! macro into scope

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
    /// Positive values move the banner *up* inside the logo (same semantics as before).
    pub banner_y_offset_inside: f32,
}

impl Default for LogoParams {
    fn default() -> Self {
        Self { target_h: 238.0, top_margin: 102.0, banner_y_offset_inside: 0.0 }
    }
}

/// What the logo builder returns.
pub struct LogoOut {
    pub ui: Vec<UIElement>,
    pub logo_bottom_y: f32, // world-space Y of the logo bottom edge (unchanged)
}

#[inline(always)]
fn screen_width(m: &Metrics) -> f32 { m.right - m.left }

#[inline(always)]
fn center_x_tl(w: f32, m: &Metrics) -> f32 {
    // top-left UI space (pixels from left): center by width w
    0.5 * (screen_width(m) - w)
}

/// Build the “banner inside logo” stack with the actor DSL.
/// - Banner width == Logo width; banner centered within; order = banner, then logo (logo on top).
pub fn build_logo(m: &Metrics, params: LogoParams) -> LogoOut {
    // Logo size from target height
    let logo_aspect = LOGO_NATIVE_W / LOGO_NATIVE_H;
    let logo_h = params.target_h;
    let logo_w = logo_h * logo_aspect;

    // Center horizontally in SM top-left space, `top_margin` from the top
    let logo_x_tl = center_x_tl(logo_w, m);
    let logo_y_tl = params.top_margin;

    // World-space bottom Y (used by menu layout), same math as before:
    // top-left world y = m.top - y_tl; bottom = top - height
    let logo_bottom_y = (m.top - logo_y_tl) - logo_h;

    // Banner (same width as logo, centered inside)
    let dance_aspect = DANCE_NATIVE_W / DANCE_NATIVE_H;
    let dance_w = logo_w;
    let dance_h = dance_w / dance_aspect;
    // Center inside logo in top-left UI space. Positive inside offset moves up, so subtract.
    let dance_x_tl = logo_x_tl;
    let dance_y_tl = logo_y_tl + 0.5 * (logo_h - dance_h) - params.banner_y_offset_inside;

    // Declare with DSL: banner first (behind), logo second (on top)
    let scene = vec![
        sprite!(anchor: TopLeft, offset: [dance_x_tl, dance_y_tl], size: [dance_w, dance_h], texture: "dance.png"),
        sprite!(anchor: TopLeft, offset: [logo_x_tl,  logo_y_tl ], size: [logo_w, logo_h ], texture: "logo.png"),
    ];

    let ui = actors::build_actors(&scene, m);
    LogoOut { ui, logo_bottom_y }
}

/// Convenience: build with default params.
pub fn build_logo_default(m: &Metrics) -> LogoOut {
    build_logo(m, LogoParams::default())
}
