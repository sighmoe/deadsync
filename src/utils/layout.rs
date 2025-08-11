// Pure, stateless helpers for logical coordinates à la StepMania.

#[inline(always)]
pub const fn logical_height() -> f32 { 480.0 }               // fixed
#[inline(always)]
pub const fn design_width_4_3() -> f32 { 640.0 }              // SM 4:3
#[inline(always)]
pub const fn design_width_16_9() -> f32 { 854.0 }             // SM 16:9 (clamp target)

#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub width:  f32,  // logical width (clamped ≤ 16:9)
    pub height: f32,  // always 480
    pub aspect: f32,  // pixels aspect (w/h)

    // convenient edges/centers in logical coords
    pub left:   f32,
    pub right:  f32,
    pub top:    f32,
    pub bottom: f32,
    pub cx:     f32,
    pub cy:     f32,
}

// Compute logical metrics for a pixel window size.
// Width grows linearly with aspect up to 16:9, then clamps.
#[inline(always)]
pub fn metrics_for_window(px_w: u32, px_h: u32) -> Metrics {
    let aspect = if px_h == 0 { 1.0 } else { px_w as f32 / px_h as f32 };
    let h = logical_height();
    let unclamped_w = h * aspect;
    let w = unclamped_w.min(design_width_16_9());

    let half_w = 0.5 * w;
    let half_h = 0.5 * h;

    Metrics {
        width: w, height: h, aspect,
        left: -half_w, right: half_w,
        bottom: -half_h, top: half_h,
        cx: 0.0, cy: 0.0,
    }
}

// StepMania/Simply Love-style wide scale between 4:3 and 16:9, clamped.
#[inline(always)]
pub fn wide_scale(v_4_3: f32, v_16_9: f32, m: &Metrics) -> f32 {
    let x = m.width;
    let a = design_width_4_3();
    let b = design_width_16_9();

    if x <= a { return v_4_3; }
    if x >= b { return v_16_9; }

    let t = (x - a) / (b - a);
    v_4_3 + t * (v_16_9 - v_4_3)
}

#[inline(always)]
pub fn sm_point_to_world(x_tl: f32, y_tl: f32, m: &Metrics) -> [f32; 2] {
    // point at (x,y) from top-left -> world coords
    [m.left + x_tl, m.top - y_tl]
}

#[inline(always)]
pub fn sm_rect_to_center_size(x_tl: f32, y_tl: f32, w: f32, h: f32, m: &Metrics)
-> ([f32; 2], [f32; 2]) {
    // rectangle given by top-left + size -> world center + size
    let cx = m.left + x_tl + 0.5 * w;
    let cy = m.top  - (y_tl + 0.5 * h);
    ([cx, cy], [w, h])
}

// Sugar: anchors like SCREEN_LEFT etc. in SM logical space
#[inline(always)] pub fn SCREEN_LEFT(m: &Metrics)   -> f32 { 0.0 }
#[inline(always)] pub fn SCREEN_TOP(m: &Metrics)    -> f32 { 0.0 }
#[inline(always)] pub fn SCREEN_RIGHT(m: &Metrics)  -> f32 { m.width }
#[inline(always)] pub fn SCREEN_BOTTOM(m: &Metrics) -> f32 { m.height }

// Also handy when positioning from edges in SM space:
#[inline(always)] pub fn from_left(px: f32, _m: &Metrics) -> f32 { px }
#[inline(always)] pub fn from_top(px: f32, _m: &Metrics)  -> f32 { px }
#[inline(always)] pub fn from_right(px: f32, m: &Metrics) -> f32 { m.width - px }
#[inline(always)] pub fn from_bottom(px: f32, m: &Metrics)-> f32 { m.height - px }