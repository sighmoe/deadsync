use cgmath::Matrix4;
use std::cell::Cell;

// -----------------------------------------------------------------------------
// Logical design space
// -----------------------------------------------------------------------------
#[inline(always)] pub const fn logical_height() -> f32 { 480.0 }
#[inline(always)] pub const fn design_width_16_9() -> f32 { 854.0 }

// -----------------------------------------------------------------------------
// Metrics (world space)
// -----------------------------------------------------------------------------
#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub left:   f32,
    pub right:  f32,
    pub top:    f32,
    pub bottom: f32,
}

// Thread-local current metrics and current *pixel* size
thread_local! {
    static CURRENT_METRICS: Cell<Metrics> = Cell::new(default_metrics());
    static CURRENT_PIXEL:   Cell<(u32,u32)> = Cell::new((854, 480));
}

#[inline(always)]
fn default_metrics() -> Metrics {
    // sensible default (16:9 design space), used only until the app sets real metrics
    metrics_for_window(854, 480)
}

#[inline(always)]
pub fn set_current_metrics(m: Metrics) {
    CURRENT_METRICS.with(|c| c.set(m));
}

#[inline(always)]
pub fn set_current_window_px(px_w: u32, px_h: u32) {
    CURRENT_PIXEL.with(|c| c.set((px_w, px_h)));
}

// -----------------------------------------------------------------------------
// StepMania-style globals (world space, origin at top-left)
// Usage:
//   use crate::core::space::globals::*;
//   let w = screen_width();
// -----------------------------------------------------------------------------
#[allow(dead_code)]
pub mod globals {
    use super::CURRENT_METRICS;

    #[inline(always)] pub fn screen_width()  -> f32 { CURRENT_METRICS.with(|c| { let m=c.get(); m.right - m.left }) }
    #[inline(always)] pub fn screen_height() -> f32 { CURRENT_METRICS.with(|c| { let m=c.get(); m.top   - m.bottom }) }

    // We use a top-left origin for UI convenience (match SM’s SCREEN_LEFT/TOP = 0)
    #[inline(always)] pub fn screen_left()   -> f32 { 0.0 }
    #[inline(always)] pub fn screen_top()    -> f32 { 0.0 }
    #[inline(always)] pub fn screen_right()  -> f32 { screen_width()  }
    #[inline(always)] pub fn screen_bottom() -> f32 { screen_height() }

    #[inline(always)] pub fn screen_center_x() -> f32 { 0.5 * screen_width()  }
    #[inline(always)] pub fn screen_center_y() -> f32 { 0.5 * screen_height() }
}

// Re-export the common getters at the crate::core::space root for convenience
pub use globals::{screen_width, screen_height, screen_left, screen_top, screen_right, screen_bottom, screen_center_x, screen_center_y};

// -----------------------------------------------------------------------------
// Metrics for a given window (pixels → world space, clamped ≤ 16:9)
// -----------------------------------------------------------------------------
#[inline(always)]
pub fn metrics_for_window(px_w: u32, px_h: u32) -> Metrics {
    let aspect = if px_h == 0 { 1.0 } else { px_w as f32 / px_h as f32 };
    let h = logical_height();
    let unclamped_w = h * aspect;
    let w = unclamped_w.min(design_width_16_9());
    let half_w = 0.5 * w;
    let half_h = 0.5 * h;

    Metrics {
        left: -half_w, right: half_w,
        bottom: -half_h, top: half_h,
    }
}

// -----------------------------------------------------------------------------
// Ortho for current window (also stores CURRENT_PIXEL + CURRENT_METRICS)
// -----------------------------------------------------------------------------
#[inline(always)]
pub fn ortho_for_window(width: u32, height: u32) -> Matrix4<f32> {
    set_current_window_px(width, height);
    let m = metrics_for_window(width, height);
    set_current_metrics(m);
    cgmath::ortho(m.left, m.right, m.bottom, m.top, -1.0, 1.0)
}

// -----------------------------------------------------------------------------
// Aspect helpers
// -----------------------------------------------------------------------------
#[inline(always)]
pub fn is_wide() -> bool {
    let w = screen_width();
    let h = screen_height();
    if h <= 0.0 { return true; } // Avoid div by zero; default to wide
    (w / h) >= 1.6
}

// -----------------------------------------------------------------------------
// Pixel size getters (in *actual* screen pixels)
// -----------------------------------------------------------------------------
#[inline(always)]
pub fn screen_pixel_width() -> f32 {
    CURRENT_PIXEL.with(|c| c.get().0 as f32)
}

#[inline(always)]
pub fn screen_pixel_height() -> f32 {
    CURRENT_PIXEL.with(|c| c.get().1 as f32)
}

// -----------------------------------------------------------------------------
// Pixel ↔ World converters (axis-aware)
// -----------------------------------------------------------------------------
#[inline(always)]
pub fn px_to_world_x(px: f32) -> f32 {
    let pw = screen_pixel_width();
    if pw <= 0.0 { return 0.0; }
    screen_width() * (px / pw)
}

#[inline(always)]
pub fn px_to_world_y(px: f32) -> f32 {
    let ph = screen_pixel_height();
    if ph <= 0.0 { return 0.0; }
    screen_height() * (px / ph)
}

// (Optional) helpers in the other direction if needed later
#[allow(dead_code)]
#[inline(always)]
pub fn world_to_px_x(world: f32) -> f32 {
    let pw = screen_pixel_width();
    let w  = screen_width();
    if w <= 0.0 { return 0.0; }
    pw * (world / w)
}

#[allow(dead_code)]
#[inline(always)]
pub fn world_to_px_y(world: f32) -> f32 {
    let ph = screen_pixel_height();
    let h  = screen_height();
    if h <= 0.0 { return 0.0; }
    ph * (world / h)
}

// -----------------------------------------------------------------------------
// WideScale helpers
// -----------------------------------------------------------------------------

// World-space WideScale (clamp world width between 640 and 854 world units)
#[inline(always)]
pub fn widescale(n43: f32, n169: f32) -> f32 {
    let w   = screen_width();
    let w43 = logical_height() * (4.0 / 3.0);  // 480 * 4/3 = 640
    let w169 = design_width_16_9();            // 854
    if w169 <= w43 { return n169; }
    let t = ((w - w43) / (w169 - w43)).clamp(0.0, 1.0);
    n43 + (n169 - n43) * t
}

// Pixel-space WideScale (exact SL: clamp _screen.w between 640 and 854 *pixels*)
#[inline(always)]
pub fn widescale_px(n43: f32, n169: f32) -> f32 {
    let w = screen_pixel_width();
    let w43: f32 = 640.0;
    let w169: f32 = 854.0;
    if w169 <= w43 { return n169; }
    let t = ((w - w43) / (w169 - w43)).clamp(0.0, 1.0);
    n43 + (n169 - n43) * t
}

