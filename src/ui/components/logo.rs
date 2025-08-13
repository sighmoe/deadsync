use cgmath::Vector2;
use crate::ui::primitives::{UIElement, Sprite};
use crate::core::space::Metrics;

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
    pub logo_bottom_y: f32,
}

/// Build the “banner inside logo” stack.
/// - The banner is the same WIDTH as the logo and centered within it.
pub fn build_logo(m: &Metrics, params: LogoParams) -> LogoOut {
    let logo_aspect = LOGO_NATIVE_W / LOGO_NATIVE_H;
    let logo_h = params.target_h;
    let logo_w = logo_h * logo_aspect;

    let logo_center = Vector2::new(0.0, m.top - params.top_margin - 0.5 * logo_h);
    let logo_bottom_y = logo_center.y - 0.5 * logo_h;

    let dance_aspect = DANCE_NATIVE_W / DANCE_NATIVE_H;
    let dance_w = logo_w;
    let dance_h = dance_w / dance_aspect;
    let dance_center = Vector2::new(logo_center.x, logo_center.y + params.banner_y_offset_inside);

    let mut ui = Vec::with_capacity(2);
    ui.push(UIElement::Sprite(Sprite {
        center: dance_center,
        size: Vector2::new(dance_w, dance_h),
        texture_id: "dance.png",
    }));
    ui.push(UIElement::Sprite(Sprite {
        center: logo_center,
        size: Vector2::new(logo_w, logo_h),
        texture_id: "logo.png",
    }));

    LogoOut { ui, logo_bottom_y }
}

/// Convenience: build with default params.
pub fn build_logo_default(m: &Metrics) -> LogoOut {
    build_logo(m, LogoParams::default())
}
