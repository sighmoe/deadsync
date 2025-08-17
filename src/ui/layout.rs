use std::collections::HashMap;
use cgmath::{Matrix4, Vector2, Vector3};

use crate::core::space::Metrics;
use crate::ui::actors::{self, Actor, SizeSpec};
use crate::ui::msdf;
use crate::core::gfx as renderer;
use crate::core::gfx::types::BlendMode;

/* ======================= RENDERER SCREEN BUILDER ======================= */

#[inline(always)]
fn object_sort_key(object: &renderer::ScreenObject) -> (u8, usize) {
    match &object.object_type {
        renderer::ObjectType::SolidColor { .. } => (0, 0),
        renderer::ObjectType::Textured { texture_id } => (1, texture_id.as_ptr() as usize),
        renderer::ObjectType::Sprite { texture_id, .. } => (2, texture_id.as_ptr() as usize),
        renderer::ObjectType::MsdfGlyph { texture_id, .. } => (3, texture_id.as_ptr() as usize),
    }
}

#[inline(always)]
pub fn build_screen(
    actors: &[Actor],
    clear_color: [f32; 4],
    m: &Metrics,
    fonts: &HashMap<&'static str, msdf::Font>,
) -> renderer::Screen {
    let mut objects = Vec::with_capacity(estimate_object_count(actors));
    let root_rect = SmRect { x: 0.0, y: 0.0, w: m.right - m.left, h: m.top - m.bottom };

    for actor in actors {
        build_actor_recursive(actor, root_rect, m, fonts, &mut objects);
    }

    // Sort objects to group by pipeline and texture, minimizing state changes.
    objects.sort_unstable_by_key(object_sort_key);

    renderer::Screen { clear_color, objects }
}

#[inline(always)]
fn estimate_object_count(actors: &[Actor]) -> usize {
    fn count(a: &Actor) -> usize {
        match a {
            Actor::Sprite { visible, .. } => if *visible { 1 } else { 0 },
            Actor::Text { content, .. } => content.chars().filter(|&c| c != '\n').count(),
            Actor::Frame { children, background, .. } => {
                let bg = if background.is_some() { 1 } else { 0 };
                bg + children.iter().map(count).sum::<usize>()
            }
        }
    }
    actors.iter().map(count).sum()
}

/* ======================= ACTOR -> OBJECT CONVERSION ======================= */

#[derive(Clone, Copy)]
struct SmRect { x: f32, y: f32, w: f32, h: f32 } // top-left "SM px" space

fn build_actor_recursive(
    actor: &Actor,
    parent: SmRect,
    m: &Metrics,
    fonts: &HashMap<&'static str, msdf::Font>,
    out: &mut Vec<renderer::ScreenObject>,
) {
    match actor {
        actors::Actor::Sprite {
            anchor, offset, size, source, tint, cell, grid, uv_rect,
            visible, flip_x, flip_y,
            cropleft, cropright, croptop, cropbottom,
            blend,
        } => {
            if !*visible { return; }
            let rect = place_rect(parent, *anchor, *offset, *size);
            push_sprite(
                out, rect, m, *source, *tint, *uv_rect, *cell, *grid,
                *flip_x, *flip_y, *cropleft, *cropright, *croptop, *cropbottom, *blend,
            );
        }

        actors::Actor::Text { anchor, offset, px, color, font, content, align } => {
            if let Some(fm) = fonts.get(font) {
                let measured = fm.measure_line_width(content, *px);
                let origin = place_text_baseline(
                    parent, *anchor, *offset, *align, measured, fm, content, *px, m,
                );

                for g in msdf::layout_line(fm, content, *px, origin) {
                    let t = Matrix4::from_translation(Vector3::new(g.center.x, g.center.y, 0.0))
                        * Matrix4::from_nonuniform_scale(g.size.x, g.size.y, 1.0);
                    out.push(renderer::ScreenObject {
                        object_type: renderer::ObjectType::MsdfGlyph {
                            texture_id: fm.atlas_tex_key,
                            uv_scale: g.uv_scale,
                            uv_offset: g.uv_offset,
                            color: *color,
                            px_range: fm.px_range,
                        },
                        transform: t,
                        blend: BlendMode::Alpha, // text uses standard alpha by default
                    });
                }
            }
        }

        actors::Actor::Frame { anchor, offset, size, children, background } => {
            let rect = place_rect(parent, *anchor, *offset, *size);

            if let Some(bg) = background {
                match bg {
                    actors::Background::Color(c) => {
                        push_rect(out, rect, m, renderer::ObjectType::SolidColor { color: *c }, BlendMode::Alpha);
                    }
                    actors::Background::Texture(tex) => {
                        push_rect(out, rect, m, renderer::ObjectType::Textured { texture_id: *tex }, BlendMode::Alpha);
                    }
                }
            }

            for child in children {
                build_actor_recursive(child, rect, m, fonts, out);
            }
        }
    }
}

/* ======================= LAYOUT HELPERS ======================= */

/// Parses sheet dimensions from a filename like "name_4x4.png" -> (4, 4).
/// Returns (1, 1) on failure to parse.
#[inline(always)]
fn parse_sheet_dims_from_filename(filename: &str) -> (u32, u32) {
    let Some(name_without_ext) = filename.rsplit_once('.').map(|(name, _)| name) else {
        return (1, 1);
    };

    let Some(last_part) = name_without_ext.rsplit('_').next() else {
        return (1, 1);
    };

    let Some((w_str, h_str)) = last_part.split_once('x') else {
        return (1, 1);
    };

    let w = w_str.parse::<u32>().unwrap_or(1);
    let h = h_str.parse::<u32>().unwrap_or(1);

    (w.max(1), h.max(1)) // Ensure dims are at least 1x1
}

#[inline(always)]
fn place_rect(parent: SmRect, anchor: actors::Anchor, offset: [f32; 2], size: [SizeSpec; 2]) -> SmRect {
    let w = match size[0] {
        SizeSpec::Px(w) => w,
        SizeSpec::Fill => parent.w,
    };
    let h = match size[1] {
        SizeSpec::Px(h) => h,
        SizeSpec::Fill => parent.h,
    };

    let (rx, ry) = anchor_ref(parent, anchor);
    let (ax, ay) = anchor_factors(anchor);

    SmRect {
        x: rx + offset[0] - ax * w,
        y: ry + offset[1] - ay * h,
        w,
        h,
    }
}

#[inline(always)]
fn rect_transform(rect: SmRect, m: &Metrics) -> Matrix4<f32> {
    // top-left "SM px" rect -> world center/size transform
    let (center, size) = sm_rect_to_world_center_size(rect, m);
    Matrix4::from_translation(Vector3::new(center.x, center.y, 0.0))
        * Matrix4::from_nonuniform_scale(size.x, size.y, 1.0)
}

#[inline(always)]
fn push_rect(
    out: &mut Vec<renderer::ScreenObject>,
    rect: SmRect,
    m: &Metrics,
    object_type: renderer::ObjectType,
    blend: BlendMode,
) {
    out.push(renderer::ScreenObject { object_type, transform: rect_transform(rect, m), blend });
}

#[inline(always)]
fn calculate_uvs(
    texture: &'static str,
    uv_rect: Option<[f32; 4]>,
    cell: Option<(u32, u32)>,
    grid: Option<(u32, u32)>,
    flip_x: bool,
    flip_y: bool,
    // Expects pre-clamped fractions
    cl: f32, cr: f32, ct: f32, cb: f32,
) -> ([f32; 2], [f32; 2]) {
    // 1. Determine base UV subrect (from explicit rect, cell, or full texture)
    let (mut uv_scale, mut uv_offset) = if let Some([u0, v0, u1, v1]) = uv_rect {
        let du = (u1 - u0).abs().max(1e-6);
        let dv = (v1 - v0).abs().max(1e-6);
        ([du, dv], [u0.min(u1), v0.min(v1)])
    } else if let Some((cx, cy)) = cell {
        let (cols, rows) = grid.unwrap_or_else(|| parse_sheet_dims_from_filename(texture));
        let s = [1.0 / cols.max(1) as f32, 1.0 / rows.max(1) as f32];
        let o = [cx.min(cols - 1) as f32 * s[0], cy.min(rows - 1) as f32 * s[1]];
        (s, o)
    } else {
        ([1.0, 1.0], [0.0, 0.0])
    };

    // 2. Apply pre-clamped crop to the base UVs
    uv_offset[0] += uv_scale[0] * cl;
    uv_offset[1] += uv_scale[1] * ct;
    uv_scale[0] *= (1.0 - cl - cr).max(0.0);
    uv_scale[1] *= (1.0 - ct - cb).max(0.0);

    // 3. Apply flips
    if flip_x { uv_offset[0] += uv_scale[0]; uv_scale[0] = -uv_scale[0]; }
    if flip_y { uv_offset[1] += uv_scale[1]; uv_scale[1] = -uv_scale[1]; }

    (uv_scale, uv_offset)
}

#[inline(always)]
fn push_sprite(
    out: &mut Vec<renderer::ScreenObject>,
    rect: SmRect,
    m: &Metrics,
    source: actors::SpriteSource,
    tint: [f32; 4],
    uv_rect: Option<[f32; 4]>,
    cell: Option<(u32, u32)>,
    grid: Option<(u32, u32)>,
    flip_x: bool,
    flip_y: bool,
    cropleft: f32,
    cropright: f32,
    croptop: f32,
    cropbottom: f32,
    blend: BlendMode,
) {
    // Clamp crop values ONCE here.
    let (cl, cr, ct, cb) = clamp_crop_fractions(cropleft, cropright, croptop, cropbottom);

    // Apply crop to geometry first.
    let cropped_rect = apply_crop_to_rect(rect, cl, cr, ct, cb);
    if cropped_rect.w <= 0.0 || cropped_rect.h <= 0.0 { return; }

    let object_type = match source {
        actors::SpriteSource::Solid => {
            renderer::ObjectType::SolidColor { color: tint }
        }
        actors::SpriteSource::Texture(texture) => {
            let (uv_scale, uv_offset) = calculate_uvs(
                texture, uv_rect, cell, grid, flip_x, flip_y,
                cl, cr, ct, cb, // Pass clamped values
            );
            renderer::ObjectType::Sprite {
                texture_id: texture,
                tint,
                uv_scale,
                uv_offset,
            }
        }
    };

    out.push(renderer::ScreenObject {
        object_type,
        transform: rect_transform(cropped_rect, m),
        blend,
    });
}

#[inline(always)]
fn clamp_crop_fractions(l: f32, r: f32, t: f32, b: f32) -> (f32, f32, f32, f32) {
    let l = l.clamp(0.0, 1.0);
    let r = r.clamp(0.0, 1.0);
    let t = t.clamp(0.0, 1.0);
    let b = b.clamp(0.0, 1.0);
    // If sums exceed 1, normalize proportionally to avoid negative sizes.
    let sum_x = l + r;
    let sum_y = t + b;
    let (l, r) = if sum_x > 1.0 { (l / sum_x, r / sum_x) } else { (l, r) };
    let (t, b) = if sum_y > 1.0 { (t / sum_y, b / sum_y) } else { (t, b) };
    (l, r, t, b)
}

#[inline(always)]
fn apply_crop_to_rect(mut rect: SmRect, l: f32, r: f32, t: f32, b: f32) -> SmRect {
    // Assumes l,r,t,b are already clamped and normalized.
    let dx = rect.w * l;
    let dy = rect.h * t;
    rect.x += dx;
    rect.y += dy;
    rect.w *= (1.0 - l - r).max(0.0);
    rect.h *= (1.0 - t - b).max(0.0);
    rect
}

#[inline(always)]
fn place_text_baseline(
    parent: SmRect,
    anchor: actors::Anchor,
    offset: [f32; 2],
    align: actors::TextAlign,
    measured_width: f32,
    font: &msdf::Font,
    content: &str,
    pixel_height: f32,
    m: &Metrics,
) -> Vector2<f32> {
    let (rx, ry) = anchor_ref(parent, anchor);

    let align_offset = match align {
        actors::TextAlign::Left   => 0.0,
        actors::TextAlign::Center => -0.5 * measured_width,
        actors::TextAlign::Right  => -measured_width,
    };
    let left_sm_x = rx + offset[0] + align_offset;

    let (asc, desc) = line_extents_px(font, content, pixel_height);
    let line_h_px = asc + desc;
    let (_, ay) = anchor_factors(anchor);
    let text_top_sm_y = ry + offset[1] - ay * line_h_px;
    let baseline_sm_y = text_top_sm_y + asc;

    let world_x = m.left + left_sm_x;
    let world_y = m.top  - baseline_sm_y;
    Vector2::new(world_x, world_y)
}

#[inline(always)]
fn anchor_ref(parent: SmRect, anchor: actors::Anchor) -> (f32, f32) {
    let (fx, fy) = anchor_factors(anchor);
    (parent.x + fx * parent.w, parent.y + fy * parent.h)
}

#[inline(always)]
const fn anchor_factors(anchor: actors::Anchor) -> (f32, f32) {
    match anchor {
        actors::Anchor::TopLeft      => (0.0, 0.0),
        actors::Anchor::TopCenter    => (0.5, 0.0),
        actors::Anchor::TopRight     => (1.0, 0.0),
        actors::Anchor::CenterLeft   => (0.0, 0.5),
        actors::Anchor::Center       => (0.5, 0.5),
        actors::Anchor::CenterRight  => (1.0, 0.5),
        actors::Anchor::BottomLeft   => (0.0, 1.0),
        actors::Anchor::BottomCenter => (0.5, 1.0),
        actors::Anchor::BottomRight  => (1.0, 1.0),
    }
}

#[inline(always)]
fn line_extents_px(font: &msdf::Font, text: &str, pixel_height: f32) -> (f32, f32) {
    if pixel_height <= 0.0 || font.line_h == 0.0 || text.is_empty() {
        return (0.0, 0.0);
    }
    let s = pixel_height / font.line_h;
    let (mut any, mut min_top, mut max_bottom) = (false, 0.0f32, 0.0f32);

    for g in text.chars().filter_map(|ch| font.glyphs.get(&ch)) {
        let top_rel_down = g.yoff * s;
        let bottom_rel_down = top_rel_down + g.plane_h * s;
        if !any {
            min_top = top_rel_down;
            max_bottom = bottom_rel_down;
            any = true;
        } else {
            if top_rel_down < min_top { min_top = top_rel_down; }
            if bottom_rel_down > max_bottom { max_bottom = bottom_rel_down; }
        }
    }

    if !any { return (0.0, 0.0) }
    ((-min_top).max(0.0), max_bottom.max(0.0))
}

#[inline(always)]
fn sm_rect_to_world_center_size(rect: SmRect, m: &Metrics) -> (Vector2<f32>, Vector2<f32>) {
    (
        Vector2::new(m.left + rect.x + 0.5 * rect.w, m.top - (rect.y + 0.5 * rect.h)),
        Vector2::new(rect.w, rect.h),
    )
}
