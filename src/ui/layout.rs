// src/ui/layout.rs
use std::collections::HashMap;
use cgmath::{Matrix4, Vector2, Vector3};

use crate::core::space::Metrics;
use crate::ui::actors::{self, Actor, SizeSpec};
use crate::ui::msdf;
use crate::core::gfx as renderer;

/* ======================= RENDERER SCREEN BUILDER ======================= */

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

    renderer::Screen { clear_color, objects }
}

#[inline(always)]
fn estimate_object_count(actors: &[Actor]) -> usize {
    fn count(a: &Actor) -> usize {
        match a {
            Actor::Quad { .. } | Actor::Sprite { .. } | Actor::SpriteCell { .. } => 1,
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
        Actor::Quad { anchor, offset, size, color } => {
            let rect = place_rect(parent, *anchor, *offset, *size);
            push_rect(out, rect, m, renderer::ObjectType::SolidColor { color: *color });
        }

        // CHANGE: Sprite now uses the Sprite pipeline (tint + UVs), with defaults:
        // tint = white, UVs = full image. This keeps visuals the same but unifies logic.
        Actor::Sprite { anchor, offset, size, texture } => {
            let rect = place_rect(parent, *anchor, *offset, *size);
            push_sprite(out, rect, m, *texture, [1.0, 1.0, 1.0, 1.0], None);
        }

        // SpriteCell reuses the same helper, passing in a cell and its tint.
        Actor::SpriteCell { anchor, offset, size, texture, tint, cell } => {
            let rect = place_rect(parent, *anchor, *offset, *size);
            push_sprite(out, rect, m, *texture, *tint, Some(*cell));
        }

        Actor::Text { anchor, offset, px, color, font, content, align } => {
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
                    });
                }
            }
        }

        Actor::Frame { anchor, offset, size, children, background } => {
            let rect = place_rect(parent, *anchor, *offset, *size);

            if let Some(bg) = background {
                match bg {
                    actors::Background::Color(c) => {
                        push_rect(out, rect, m, renderer::ObjectType::SolidColor { color: *c });
                    }
                    actors::Background::Texture(tex) => {
                        // This can stay as a full textured rectangle.
                        push_rect(out, rect, m, renderer::ObjectType::Textured { texture_id: *tex });
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
) {
    out.push(renderer::ScreenObject { object_type, transform: rect_transform(rect, m) });
}

#[inline(always)]
fn push_sprite(
    out: &mut Vec<renderer::ScreenObject>,
    rect: SmRect,
    m: &Metrics,
    texture: &'static str,
    tint: [f32; 4],
    cell: Option<(u32, u32)>,
) {
    let (uv_scale, uv_offset) = match cell {
        Some((cx, cy)) => {
            let (cols, rows) = parse_sheet_dims_from_filename(texture);
            if cols > 1 || rows > 1 {
                let scale = [1.0 / cols.max(1) as f32, 1.0 / rows.max(1) as f32];
                let offset = [cx as f32 * scale[0], cy as f32 * scale[1]];
                (scale, offset)
            } else {
                // Filename didn't encode a grid; fall back to full texture.
                ([1.0, 1.0], [0.0, 0.0])
            }
        }
        None => ([1.0, 1.0], [0.0, 0.0]),
    };

    out.push(renderer::ScreenObject {
        object_type: renderer::ObjectType::Sprite {
            texture_id: texture,
            tint,
            uv_scale,
            uv_offset,
        },
        transform: rect_transform(rect, m),
    });
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
