use cgmath::Matrix4;
pub mod sm;

#[inline(always)] pub const fn logical_height() -> f32 { 480.0 }
#[inline(always)] pub const fn design_width_16_9() -> f32 { 854.0 }

#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub left:   f32,
    pub right:  f32,
    pub top:    f32,
    pub bottom: f32,
}

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
