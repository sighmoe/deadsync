// src/core/space.rs
use cgmath::Matrix4;
use std::cell::Cell; // NEW

#[inline(always)] pub const fn logical_height() -> f32 { 480.0 }
#[inline(always)] pub const fn design_width_16_9() -> f32 { 854.0 }

#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub left:   f32,
    pub right:  f32,
    pub top:    f32,
    pub bottom: f32,
}

// ---------- NEW: thread-local current metrics + setters/getters ----------
thread_local! {
    static CURRENT_METRICS: Cell<Metrics> = Cell::new(default_metrics());
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

/// StepMania-style globals as zero-arg getters.
/// Usage:
///   use crate::core::space::globals::*;
///   let w = SCREEN_WIDTH();
#[allow(non_snake_case)]
#[allow(dead_code)]
pub mod globals {
    use super::CURRENT_METRICS;

    #[inline(always)] pub fn SCREEN_WIDTH()  -> f32 { CURRENT_METRICS.with(|c| { let m=c.get(); m.right - m.left }) }
    #[inline(always)] pub fn SCREEN_HEIGHT() -> f32 { CURRENT_METRICS.with(|c| { let m=c.get(); m.top   - m.bottom }) }

    #[inline(always)] pub fn SCREEN_LEFT()   -> f32 { 0.0 }
    #[inline(always)] pub fn SCREEN_TOP()    -> f32 { 0.0 }
    #[inline(always)] pub fn SCREEN_RIGHT()  -> f32 { SCREEN_WIDTH()  }
    #[inline(always)] pub fn SCREEN_BOTTOM() -> f32 { SCREEN_HEIGHT() }

    #[inline(always)] pub fn SCREEN_CENTER_X() -> f32 { 0.5 * SCREEN_WIDTH()  }
    #[inline(always)] pub fn SCREEN_CENTER_Y() -> f32 { 0.5 * SCREEN_HEIGHT() }
}
// ------------------------------------------------------------------------

/// Compute logical metrics for a pixel window size (clamped ≤ 16:9).
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

/// World-space orthographic matrix for the current window.
#[inline(always)]
pub fn ortho_for_window(width: u32, height: u32) -> Matrix4<f32> {
    let m = metrics_for_window(width, height);
    cgmath::ortho(m.left, m.right, m.bottom, m.top, -1.0, 1.0)
}
