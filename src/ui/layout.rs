use crate::core::gfx as renderer;
use crate::core::gfx::{BlendMode, RenderList, RenderObject};
use crate::core::space::Metrics;
use crate::ui::actors::{self, Actor, SizeSpec};
use crate::ui::msdf;
use cgmath::{Deg, Matrix4, Vector2, Vector3};

/* ======================= RENDERER SCREEN BUILDER ======================= */

#[inline(always)]
pub fn build_screen(
    actors: &[actors::Actor],
    clear_color: [f32; 4],
    m: &Metrics,
    fonts: &std::collections::HashMap<&'static str, msdf::Font>,
    total_elapsed: f32,
) -> RenderList {
    let mut objects = Vec::with_capacity(estimate_object_count(actors));
    let mut order_counter: u32 = 0;

    let root_rect = SmRect {
        x: 0.0,
        y: 0.0,
        w: m.right - m.left,
        h: m.top - m.bottom,
    };

    let parent_z: i16 = 0;

    for actor in actors {
        build_actor_recursive(
            actor,
            root_rect,
            m,
            fonts,
            parent_z,
            &mut order_counter,
            &mut objects,
            total_elapsed,
        );
    }

    objects.sort_by_key(|o| (o.z, o.order));
    RenderList { clear_color, objects }
}

// ===== REPLACE the whole function in src/ui/layout.rs =====
#[inline(always)]
fn estimate_object_count(actors: &[Actor]) -> usize {
    let mut stack: Vec<&Actor> = Vec::with_capacity(actors.len());
    stack.extend(actors.iter());

    let mut total = 0usize;
    while let Some(a) = stack.pop() {
        match a {
            Actor::Sprite { visible, .. } => {
                if *visible {
                    total += 1;
                }
            }
            Actor::Text { content, .. } => {
                // Count Unicode scalar values (skip newlines), not raw bytes.
                // This better matches how we later emit MSDF glyphs.
                total += content.chars().filter(|&ch| ch != '\n').count();
            }
            Actor::Frame { children, background, .. } => {
                if background.is_some() {
                    total += 1;
                }
                stack.extend(children.iter());
            }
        }
    }
    total
}

/* ======================= ACTOR -> OBJECT CONVERSION ======================= */

#[derive(Clone, Copy)]
struct SmRect { x: f32, y: f32, w: f32, h: f32 }

#[inline(always)]
fn build_actor_recursive(
    actor: &actors::Actor,
    parent: SmRect,
    m: &Metrics,
    fonts: &std::collections::HashMap<&'static str, msdf::Font>,
    base_z: i16,
    order_counter: &mut u32,
    out: &mut Vec<RenderObject>,
    total_elapsed: f32,
) {
    match actor {
        actors::Actor::Sprite {
            align, offset, size, source, tint, z,
            cell, grid, uv_rect, visible, flip_x, flip_y,
            cropleft, cropright, croptop, cropbottom, blend,
            rot_z_deg, texcoordvelocity,
        } => {
            if !*visible { return; }
            let rect = place_rect(parent, *align, *offset, *size);
            let before = out.len();
            push_sprite(
                out, rect, m, *source, *tint, *uv_rect, *cell, *grid,
                *flip_x, *flip_y, *cropleft, *cropright, *croptop, *cropbottom, *blend,
                *rot_z_deg, *texcoordvelocity, total_elapsed,
            );
            let layer = base_z.saturating_add(*z);
            for i in before..out.len() {
                out[i].z = layer;
                out[i].order = { let o = *order_counter; *order_counter += 1; o };
            }
        }
        actors::Actor::Text {
            align, offset, px, color, font, content, align_text, z, scale,
            fit_width, fit_height, blend,
        } => {
            if let Some(fm) = fonts.get(font) {
                // Base metrics at requested px (before any zoom/fit)
                let measured_w = fm.measure_line_width(content, *px);
                let (asc, desc) = line_extents_px(fm, content, *px);
                let line_h_px   = asc + desc;

                // Fit scalar: prefer explicit width, else height; uniform to preserve aspect.
                let fit_s = if let Some(w_target) = *fit_width {
                    if measured_w > 0.0 { (w_target / measured_w).max(0.0) } else { 1.0 }
                } else if let Some(h_target) = *fit_height {
                    if line_h_px > 0.0 { (h_target / line_h_px).max(0.0) } else { 1.0 }
                } else {
                    1.0
                };

                // Final per-axis scale = fit * zoomx/zoomy
                let sx = (scale[0].max(0.0)) * fit_s;
                let sy = (scale[1].max(0.0)) * fit_s;

                // Compute baseline origin using scaled box (SM semantics)
                let origin = place_text_baseline(
                    parent, *align, *offset, *align_text,
                    measured_w, asc, desc, sx, sy, m
                );

                let layer = base_z.saturating_add(*z);

                for g in msdf::layout_line(fm, content, *px, origin) {
                    // Scale glyph position about the origin (baseline pivot)
                    let cx = origin.x + (g.center.x - origin.x) * sx;
                    let cy = origin.y + (g.center.y - origin.y) * sy;

                    // Scale glyph size
                    let size_x = g.size.x * sx;
                    let size_y = g.size.y * sy;

                    let t = Matrix4::from_translation(Vector3::new(cx, cy, 0.0))
                          * Matrix4::from_nonuniform_scale(size_x, size_y, 1.0);

                    out.push(RenderObject {
                        object_type: renderer::ObjectType::MsdfGlyph {
                            texture_id: fm.atlas_tex_key,
                            uv_scale: g.uv_scale,
                            uv_offset: g.uv_offset,
                            color: *color,
                            px_range: fm.px_range,
                        },
                        transform: t,
                        // NEW: use text actor's blend (matches SM)
                        blend: *blend,
                        z: layer,
                        order: { let o = *order_counter; *order_counter += 1; o },
                    });
                }
            }
        }
        actors::Actor::Frame {
            align, offset, size, children, background, z,
        } => {
            let rect = place_rect(parent, *align, *offset, *size);
            let layer = base_z.saturating_add(*z);
            if let Some(bg) = background {
                match bg {
                    actors::Background::Color(c) => {
                        let before = out.len();
                        push_sprite(
                            out, rect, m, actors::SpriteSource::Solid, *c,
                            None, None, None, false, false,
                            0.0, 0.0, 0.0, 0.0, BlendMode::Alpha, 0.0,
                            None, 0.0,
                        );
                        for i in before..out.len() {
                            out[i].z = layer;
                            out[i].order = { let o = *order_counter; *order_counter += 1; o };
                        }
                    }
                    actors::Background::Texture(tex) => {
                        let before = out.len();
                        push_sprite(
                            out, rect, m, actors::SpriteSource::Texture(*tex), [1.0; 4],
                            None, None, None, false, false,
                            0.0, 0.0, 0.0, 0.0, BlendMode::Alpha, 0.0,
                            None, 0.0,
                        );
                        for i in before..out.len() {
                            out[i].z = layer;
                            out[i].order = { let o = *order_counter; *order_counter += 1; o };
                        }
                    }
                }
            }
            for child in children {
                build_actor_recursive(child, rect, m, fonts, layer, order_counter, out, total_elapsed);
            }
        }
    }
}

/* ======================= LAYOUT HELPERS ======================= */

/// Parses sheet dimensions from a filename like "name_4x4.png" -> (4, 4).
/// Returns (1, 1) on failure to parse.
#[inline(always)]
fn parse_sheet_dims_from_filename(filename: &str) -> (u32, u32) {
    #[inline(always)]
    fn parse_u32_digits(bs: &[u8]) -> Option<u32> {
        if bs.is_empty() { return None; }
        let mut acc: u32 = 0;
        for &b in bs {
            if !(b'0'..=b'9').contains(&b) { return None; }
            let d = (b - b'0') as u32;
            acc = acc.checked_mul(10)?.checked_add(d)?;
        }
        Some(acc)
    }

    let bytes = filename.as_bytes();

    // strip extension
    let end = bytes.iter().rposition(|&b| b == b'.').map(|i| i).unwrap_or(bytes.len());
    // find last '_' before extension
    let us = bytes[..end].iter().rposition(|&b| b == b'_');
    let Some(us_i) = us else { return (1, 1) };

    let dims = &bytes[us_i + 1..end];
    // expect "<cols>x<rows>"
    let x_pos = dims.iter().position(|&b| b == b'x' || b == b'X');
    let Some(xi) = x_pos else { return (1, 1) };

    let w = parse_u32_digits(&dims[..xi]).unwrap_or(1);
    let h = parse_u32_digits(&dims[xi + 1..]).unwrap_or(1);

    if w == 0 || h == 0 { (1, 1) } else { (w, h) }
}

#[inline(always)]
fn place_rect(parent: SmRect, align: [f32; 2], offset: [f32; 2], size: [SizeSpec; 2]) -> SmRect {
    // StepMania semantics:
    // - `offset` (xy) is in the parent's local top-left space.
    // - `align` only affects the pivot inside *this* rect.
    let w = match size[0] {
        SizeSpec::Px(w) => w,
        SizeSpec::Fill  => parent.w,
    };
    let h = match size[1] {
        SizeSpec::Px(h) => h,
        SizeSpec::Fill  => parent.h,
    };

    // Parent reference is ALWAYS its top-left.
    let rx = parent.x;
    let ry = parent.y;

    // Actor's internal pivot from align (0..1 inside its own rect).
    let ax = align[0];
    let ay = align[1];

    SmRect {
        // Put the actor's pivot at (rx + offset.x, ry + offset.y)
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
fn rect_transform_with_rot(rect: SmRect, m:&Metrics, rot_z_deg:f32) -> Matrix4<f32> {
    let (center, size) = sm_rect_to_world_center_size(rect, m);
    Matrix4::from_translation(Vector3::new(center.x, center.y, 0.0))
        * Matrix4::from_angle_z(Deg(rot_z_deg))
        * Matrix4::from_nonuniform_scale(size.x, size.y, 1.0)
}

#[inline(always)]
fn calculate_uvs(
    texture: &'static str,
    uv_rect: Option<[f32; 4]>,
    cell: Option<(u32, u32)>,
    grid: Option<(u32, u32)>,
    flip_x: bool,
    flip_y: bool,
    cl: f32, cr: f32, ct: f32, cb: f32, // Expects pre-clamped fractions
    texcoordvelocity: Option<[f32; 2]>,
    total_elapsed: f32,
) -> ([f32; 2], [f32; 2]) {
    // 1) Determine base UV subrect (from explicit rect, cell, or full texture)
    let (mut uv_scale, mut uv_offset) = if let Some([u0, v0, u1, v1]) = uv_rect {
        let du = (u1 - u0).abs().max(1e-6);
        let dv = (v1 - v0).abs().max(1e-6);
        ([du, dv], [u0.min(u1), v0.min(v1)])
    } else if let Some((cx, cy)) = cell {
        // support for setstate(linearIndex) via sentinel cy == u32::MAX
        let (gc, gr) = grid.unwrap_or_else(|| parse_sheet_dims_from_filename(texture));
        let cols = gc.max(1);
        let rows = gr.max(1);

        let (col, row) = if cy == u32::MAX {
            let idx = cx;
            (idx % cols, (idx / cols).min(rows.saturating_sub(1)))
        } else {
            (cx.min(cols.saturating_sub(1)), cy.min(rows.saturating_sub(1)))
        };

        let s = [1.0 / cols as f32, 1.0 / rows as f32];
        let o = [col as f32 * s[0], row as f32 * s[1]];
        (s, o)
    } else {
        ([1.0, 1.0], [0.0, 0.0])
    };

    // 2) Apply pre-clamped crop to the base UVs
    uv_offset[0] += uv_scale[0] * cl;
    uv_offset[1] += uv_scale[1] * ct;
    uv_scale[0] *= (1.0 - cl - cr).max(0.0);
    uv_scale[1] *= (1.0 - ct - cb).max(0.0);

    // 3) Apply flips
    if flip_x { uv_offset[0] += uv_scale[0]; uv_scale[0] = -uv_scale[0]; }
    if flip_y { uv_offset[1] += uv_scale[1]; uv_scale[1] = -uv_scale[1]; }
    
    // 4) Apply velocity (after all other calculations)
    if let Some(vel) = texcoordvelocity {
        uv_offset[0] += vel[0] * total_elapsed;
        uv_offset[1] += vel[1] * total_elapsed;
    }

    (uv_scale, uv_offset)
}

#[inline(always)]
fn push_sprite(
    out: &mut Vec<renderer::RenderObject>,
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
    rot_z_deg: f32,
    texcoordvelocity: Option<[f32; 2]>,
    total_elapsed: f32,
) {
    // Clamp crop values ONCE here.
    let (cl, cr, ct, cb) = clamp_crop_fractions(cropleft, cropright, croptop, cropbottom);

    // Apply crop to geometry first.
    let cropped_rect = apply_crop_to_rect(rect, cl, cr, ct, cb);
    if cropped_rect.w <= 0.0 || cropped_rect.h <= 0.0 { return; }

    // Unify: solids are sprites using the built-in "__white" texture.
    let (texture_id, uv_scale, uv_offset) = match source {
        actors::SpriteSource::Solid => ("__white", [1.0, 1.0], [0.0, 0.0]),
        actors::SpriteSource::Texture(texture) => {
            let (uv_scale, uv_offset) = calculate_uvs(
                texture, uv_rect, cell, grid, flip_x, flip_y, cl, cr, ct, cb,
                texcoordvelocity, total_elapsed
            );
            (texture, uv_scale, uv_offset)
        }
    };

    let transform = if rot_z_deg != 0.0 {
        rect_transform_with_rot(cropped_rect, m, rot_z_deg)
    } else {
        rect_transform(cropped_rect, m)
    };

    out.push(renderer::RenderObject {
        object_type: renderer::ObjectType::Sprite { texture_id, tint, uv_scale, uv_offset },
        transform,
        blend,
        z: 0,
        order: 0,
    });
}

#[inline(always)]
fn clamp_crop_fractions(l: f32, r: f32, t: f32, b: f32) -> (f32, f32, f32, f32) {
    // StepMania semantics: clamp each edge independently to [0,1].
    // If l+r >= 1 or t+b >= 1 the geometry collapses to zero on that axis.
    // Do NOT renormalize proportionally (that changes which side wins).
    (
        l.clamp(0.0, 1.0),
        r.clamp(0.0, 1.0),
        t.clamp(0.0, 1.0),
        b.clamp(0.0, 1.0),
    )
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
    actor_align: [f32; 2],          // halign/valign pivot inside the line box
    offset: [f32; 2],               // parent TL space
    talign: actors::TextAlign,      // horizalign: left/center/right
    measured_width_px: f32,         // at the provided px (pre-scale)
    asc_px: f32,                    // at the provided px (pre-scale)
    desc_px: f32,                   // at the provided px (pre-scale)
    sx: f32,                        // final X scale (fit * zoomx)
    sy: f32,                        // final Y scale (fit * zoomy)
    m: &Metrics,
) -> Vector2<f32> {
    // Parent reference is ALWAYS its top-left in SM.
    let rx = parent.x;
    let ry = parent.y;

    // Horizontal offset uses the *scaled* width so center/right anchor correctly under zoom.
    let width_scaled = measured_width_px * sx;
    let align_offset_x = match talign {
        actors::TextAlign::Left   => 0.0,
        actors::TextAlign::Center => -0.5 * width_scaled,
        actors::TextAlign::Right  => -width_scaled,
    };

    // Vertical: compute baseline so the chosen pivot inside the (scaled) line box stays put.
    let asc_s   = asc_px * sy;
    let line_hs = (asc_px + desc_px) * sy;
    let ay      = actor_align[1]; // 0=top, .5=middle, 1=bottom

    let text_top_sm_y = ry + offset[1] - ay * line_hs;
    let baseline_sm_y = text_top_sm_y + asc_s;

    // Convert SM top-left "px" to world
    let world_x = m.left + (rx + offset[0] + align_offset_x);
    let world_y = m.top  - baseline_sm_y;

    Vector2::new(world_x, world_y)
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
