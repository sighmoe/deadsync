use std::collections::HashMap;
use cgmath::{Matrix4, Vector3};

use crate::core::space::{Metrics, logical_height, design_width_16_9};
use crate::ui::primitives as api;
use crate::ui::msdf;
use crate::core::gfx as renderer;

/* ---------- StepMania-style anchors & helpers (UI space, SM top-left) ---------- */

#[inline(always)] fn screen_width(m: &Metrics)  -> f32 { m.right - m.left }
#[inline(always)] fn screen_height(m: &Metrics) -> f32 { m.top   - m.bottom }

/// Wide scale between 4:3 and 16:9 based on logical width.
#[inline(always)]
pub fn wide_scale(v_4_3: f32, v_16_9: f32, m: &Metrics) -> f32 {
    let w_43  = logical_height() * (4.0 / 3.0);
    let w_169 = design_width_16_9();
    let w     = screen_width(m);
    if w <= w_43 { return v_4_3; }
    if w >= w_169 { return v_16_9; }
    let t = (w - w_43) / (w_169 - w_43);
    v_4_3 + t * (v_16_9 - v_4_3)
}

#[inline(always)] pub fn screen_left(_m: &Metrics)   -> f32 { 0.0 }
#[inline(always)] pub fn screen_top(_m: &Metrics)    -> f32 { 0.0 }
#[inline(always)] pub fn screen_right(m: &Metrics)   -> f32 { screen_width(m) }
#[inline(always)] pub fn screen_bottom(m: &Metrics)  -> f32 { screen_height(m) }

#[inline(always)] pub fn from_left(px: f32, _m: &Metrics)  -> f32 { px }
#[inline(always)] pub fn from_top(px: f32, _m: &Metrics)   -> f32 { px }
#[inline(always)] pub fn from_right(px: f32, m: &Metrics)  -> f32 { screen_width(m)  - px }
#[inline(always)] pub fn from_bottom(px: f32, m: &Metrics) -> f32 { screen_height(m) - px }

#[inline(always)]
pub fn sm_point_to_world(x_tl: f32, y_tl: f32, m: &Metrics) -> [f32; 2] {
    [m.left + x_tl, m.top - y_tl]
}

#[inline(always)]
pub fn sm_rect_to_center_size(x_tl: f32, y_tl: f32, w: f32, h: f32, m: &Metrics)
-> ([f32; 2], [f32; 2]) {
    let cx = m.left + x_tl + 0.5 * w;
    let cy = m.top  - (y_tl + 0.5 * h);
    ([cx, cy], [w, h])
}

/* ---------- Builder: UI -> renderer::Screen ---------- */

#[inline(always)]
pub fn build_screen(
    elements: &[api::UIElement],
    clear_color: [f32; 4],
    fonts: &HashMap<&'static str, msdf::Font>,
) -> renderer::Screen {
    renderer::Screen {
        clear_color,
        objects: expand_ui_to_objects(elements, fonts),
    }
}

pub fn expand_ui_to_objects(
    elements: &[api::UIElement],
    fonts: &HashMap<&'static str, msdf::Font>,
) -> Vec<renderer::ScreenObject> {
    use cgmath::{Matrix4, Vector3};

    let mut objects = Vec::with_capacity(estimate_object_count(elements, fonts));

    for e in elements {
        match e {
            api::UIElement::Quad(q) => {
                let t = Matrix4::from_translation(Vector3::new(q.center.x, q.center.y, 0.0))
                    * Matrix4::from_nonuniform_scale(q.size.x, q.size.y, 1.0);
                objects.push(renderer::ScreenObject {
                    object_type: renderer::ObjectType::SolidColor { color: q.color },
                    transform: t,
                });
            }
            api::UIElement::Sprite(s) => {
                let t = Matrix4::from_translation(Vector3::new(s.center.x, s.center.y, 0.0))
                    * Matrix4::from_nonuniform_scale(s.size.x, s.size.y, 1.0);
                objects.push(renderer::ScreenObject {
                    object_type: renderer::ObjectType::Textured { texture_id: s.texture_id },
                    transform: t,
                });
            }
            api::UIElement::Text(txt) => {
                if let Some(font) = fonts.get(txt.font_id) {
                    let laid = msdf::layout_line(font, &txt.content, txt.pixel_height, txt.origin);
                    for g in laid {
                        let t = Matrix4::from_translation(Vector3::new(g.center.x, g.center.y, 0.0))
                            * Matrix4::from_nonuniform_scale(g.size.x, g.size.y, 1.0);
                        objects.push(renderer::ScreenObject {
                            object_type: renderer::ObjectType::MsdfGlyph {
                                texture_id: font.atlas_tex_key,
                                uv_scale: g.uv_scale,
                                uv_offset: g.uv_offset,
                                color: txt.color,
                                px_range: font.px_range,
                            },
                            transform: t,
                        });
                    }
                }
            }
        }
    }
    objects
}

#[inline(always)]
fn estimate_object_count(
    elements: &[api::UIElement],
    fonts: &HashMap<&'static str, msdf::Font>,
) -> usize {
    let mut total = 0usize;
    for e in elements {
        match e {
            api::UIElement::Quad(_) | api::UIElement::Sprite(_) => total += 1,
            api::UIElement::Text(t) => {
                if let Some(font) = fonts.get(t.font_id) {
                    let mut n = 0usize;
                    for ch in t.content.chars() {
                        if ch == '\n' { continue; }
                        if let Some(g) = font.glyphs.get(&ch) {
                            if g.plane_w > 0.0 && g.plane_h > 0.0 {
                                n += 1;
                            }
                        }
                    }
                    total += n;
                }
            }
        }
    }
    total
}
