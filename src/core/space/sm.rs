// src/core/space/sm.rs
//! StepMania-style screen globals in *top-left (SM px)* space.
//! All functions are pure, inline, and derived from `Metrics`.

use super::Metrics;

#[inline(always)] pub fn width (m: &Metrics) -> f32 { m.right - m.left }
#[inline(always)] pub fn height(m: &Metrics) -> f32 { m.top   - m.bottom }

#[inline(always)] pub fn left  (_: &Metrics) -> f32 { 0.0 }           // SCREEN_LEFT
#[inline(always)] pub fn top   (_: &Metrics) -> f32 { 0.0 }           // SCREEN_TOP
#[inline(always)] pub fn right (m: &Metrics) -> f32 { width(m) }      // SCREEN_RIGHT
#[inline(always)] pub fn bottom(m: &Metrics) -> f32 { height(m) }     // SCREEN_BOTTOM

#[inline(always)] pub fn cx(m: &Metrics) -> f32 { 0.5 * width(m) }    // SCREEN_CENTER_X
#[inline(always)] pub fn cy(m: &Metrics) -> f32 { 0.5 * height(m) }   // SCREEN_CENTER_Y
#[inline(always)] pub fn center(m: &Metrics) -> (f32, f32) { (cx(m), cy(m)) }

/// Convenience: position by fractional anchors of the parent (0..1, 0..1) in SM TL space.
#[inline(always)] pub fn at(m: &Metrics, hx: f32, vy: f32) -> (f32, f32) {
    (hx * width(m), vy * height(m))
}