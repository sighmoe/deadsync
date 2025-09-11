// FILE: /mnt/c/Users/PerfectTaste/Documents/GitHub/new-engine/src/ui/compose.rs

use crate::core::gfx as renderer;
use crate::core::gfx::{BlendMode, RenderList, RenderObject};
use crate::core::space::Metrics;
use crate::core::{assets, font};
use crate::ui::actors::{self, Actor, SizeSpec};
use cgmath::{Deg, Matrix4, Vector2, Vector3};

/* ======================= RENDERER SCREEN BUILDER ======================= */

#[inline(always)]
pub fn build_screen(
    actors: &[actors::Actor],
    clear_color: [f32; 4],
    m: &Metrics,
    fonts: &std::collections::HashMap<&'static str, font::Font>,
    total_elapsed: f32,
) -> RenderList {
    let mut objects = Vec::with_capacity(estimate_object_count(actors, fonts));
    let mut order_counter: u32 = 0;

    let root_rect = SmRect { x: 0.0, y: 0.0, w: m.right - m.left, h: m.top - m.bottom };
    let parent_z: i16 = 0;

    for actor in actors {
        build_actor_recursive(
            actor, root_rect, m, fonts, parent_z, &mut order_counter, &mut objects, total_elapsed,
        );
    }

    objects.sort_by_key(|o| (o.z, o.order));
    RenderList { clear_color, objects }
}

#[inline(always)]
fn estimate_object_count(
    actors: &[Actor],
    fonts: &std::collections::HashMap<&'static str, font::Font>,
) -> usize {
    let mut stack: Vec<&Actor> = Vec::with_capacity(actors.len());
    stack.extend(actors.iter());

    let mut total = 0usize;
    while let Some(a) = stack.pop() {
        match a {
            Actor::Sprite { visible, tint, .. } => {
                if *visible && tint[3] > 0.0 {
                    total += 1;
                }
            }
            Actor::Text { content, font, .. } => {
                if let Some(fm) = fonts.get(font) {
                    total += content.chars().filter(|&c| {
                        if c == '\n' { return false; }
                        let mapped = fm.glyph_map.contains_key(&c);
                        if c == ' ' && !mapped { return false; } // skip unmapped spaces
                        mapped || fm.default_glyph.is_some()
                    }).count();
                } else {
                    total += content.chars().filter(|&ch| ch != '\n').count();
                }
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
    fonts: &std::collections::HashMap<&'static str, font::Font>,
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
            fadeleft, faderight, fadetop, fadebottom,
            rot_z_deg, texcoordvelocity, animate, state_delay,
            scale,
        } => {
            if !*visible { return; }

            let (is_solid, texture_name) = match source {
                actors::SpriteSource::Solid => (true, "__white"),
                actors::SpriteSource::Texture(name) => (false, name.as_str()),
            };

            let mut chosen_cell = *cell;
            let mut chosen_grid = *grid;

            if !is_solid && uv_rect.is_none() {
                let (cols, rows) = grid.unwrap_or_else(|| font::parse_sheet_dims_from_filename(texture_name));
                let total = cols.saturating_mul(rows).max(1);

                let start_linear: u32 = match *cell {
                    Some((cx, cy)) if cy != u32::MAX => {
                        let cx = cx.min(cols.saturating_sub(1));
                        let cy = cy.min(rows.saturating_sub(1));
                        cy.saturating_mul(cols).saturating_add(cx)
                    }
                    Some((i, _)) => i,
                    None => 0,
                };

                if *animate && *state_delay > 0.0 && total > 1 {
                    let steps = (total_elapsed / *state_delay).floor().max(0.0) as u32;
                    let idx = (start_linear + (steps % total)) % total;
                    chosen_cell = Some((idx, u32::MAX));
                    chosen_grid = Some((cols, rows));
                } else if chosen_cell.is_none() && total > 1 {
                    chosen_cell = Some((0, u32::MAX));
                    chosen_grid = Some((cols, rows));
                }
            }

            let resolved_size = resolve_sprite_size_like_sm(
                *size, is_solid, texture_name, *uv_rect, chosen_cell, chosen_grid, *scale
            );

            let rect = place_rect(parent, *align, *offset, resolved_size);

            let before = out.len();
            push_sprite(
                out, rect, m, is_solid, texture_name, *tint, *uv_rect, chosen_cell, chosen_grid,
                *flip_x, *flip_y,
                *cropleft, *cropright, *croptop, *cropbottom,
                *fadeleft, *faderight, *fadetop, *fadebottom,
                *blend, *rot_z_deg, *texcoordvelocity, total_elapsed,
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
                let mut objects = layout_text(
                    fm, content, *px, *scale, *fit_width, *fit_height, 
                    parent, *align, *offset, *align_text, m
                );
                
                let layer = base_z.saturating_add(*z);
                for obj in &mut objects {
                    obj.z = layer;
                    obj.order = { let o = *order_counter; *order_counter += 1; o };
                    obj.blend = *blend;
                    if let renderer::ObjectType::Sprite { tint, .. } = &mut obj.object_type {
                        *tint = *color;
                    }
                }
                out.extend(objects);
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
                            out, rect, m,
                            true, "__white", *c,
                            None, None, None,
                            false, false,
                            0.0, 0.0, 0.0, 0.0,
                            0.0, 0.0, 0.0, 0.0,
                            BlendMode::Alpha,
                            0.0, None, total_elapsed,
                        );
                        for i in before..out.len() {
                            out[i].z = layer;
                            out[i].order = { let o = *order_counter; *order_counter += 1; o };
                        }
                    }
                    actors::Background::Texture(tex) => {
                        let before = out.len();
                        push_sprite(
                            out, rect, m,
                            false, tex, [1.0; 4],
                            None, None, None,
                            false, false,
                            0.0, 0.0, 0.0, 0.0,
                            0.0, 0.0, 0.0, 0.0,
                            BlendMode::Alpha,
                            0.0, None, total_elapsed,
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

#[inline(always)]
fn resolve_sprite_size_like_sm(
    size: [SizeSpec; 2],
    is_solid: bool,
    texture_name: &str,
    uv_rect: Option<[f32; 4]>,
    cell: Option<(u32, u32)>,
    grid: Option<(u32, u32)>,
    scale: [f32; 2],
) -> [SizeSpec; 2] {
    use SizeSpec::Px;

    #[inline(always)]
    fn native_dims(
        is_solid: bool, texture_name: &str, uv: Option<[f32; 4]>, cell: Option<(u32, u32)>, grid: Option<(u32, u32)>
    ) -> (f32, f32) {
        if is_solid { return (1.0, 1.0); }
        let Some(meta) = assets::texture_dims(texture_name) else { return (0.0, 0.0); };
        let (mut tw, mut th) = (meta.w as f32, meta.h as f32);
        if let Some([u0, v0, u1, v1]) = uv {
            tw *= (u1 - u0).abs().max(1e-6);
            th *= (v1 - v0).abs().max(1e-6);
        } else if cell.is_some() {
            let (gc, gr) = grid.unwrap_or_else(|| font::parse_sheet_dims_from_filename(texture_name));
            let cols = gc.max(1);
            let rows = gr.max(1);
            tw /= cols as f32;
            th /= rows as f32;
        }
        (tw, th)
    }

    let (nw, nh) = native_dims(is_solid, texture_name, uv_rect, cell, grid);
    let aspect = if nw > 0.0 && nh > 0.0 { nh / nw } else { 1.0 };

    match (size[0], size[1]) {
        (Px(w), Px(h)) if w == 0.0 && h == 0.0 => {
            [Px(nw * scale[0]), Px(nh * scale[1])]
        }
        (Px(w), Px(h)) if w > 0.0 && h == 0.0 => {
            [Px(w), Px(w * aspect)]
        }
        (Px(w), Px(h)) if w == 0.0 && h > 0.0 => {
            let inv_aspect = if aspect > 0.0 { 1.0 / aspect } else { 1.0 };
            [Px(h * inv_aspect), Px(h)]
        }
        _ => size,
    }
}

#[inline(always)]
fn place_rect(parent: SmRect, align: [f32; 2], offset: [f32; 2], size: [SizeSpec; 2]) -> SmRect {
    let w = match size[0] {
        SizeSpec::Px(w) => w,
        SizeSpec::Fill  => parent.w,
    };
    let h = match size[1] {
        SizeSpec::Px(h) => h,
        SizeSpec::Fill  => parent.h,
    };
    let rx = parent.x;
    let ry = parent.y;
    let ax = align[0];
    let ay = align[1];

    SmRect {
        x: rx + offset[0] - ax * w,
        y: ry + offset[1] - ay * h,
        w,
        h,
    }
}

#[inline(always)]
fn calculate_uvs(
    texture: &str,
    uv_rect: Option<[f32; 4]>,
    cell: Option<(u32, u32)>,
    grid: Option<(u32, u32)>,
    flip_x: bool,
    flip_y: bool,
    cl: f32, cr: f32, ct: f32, cb: f32,
    texcoordvelocity: Option<[f32; 2]>,
    total_elapsed: f32,
) -> ([f32; 2], [f32; 2]) {
    let (mut uv_scale, mut uv_offset) = if let Some([u0, v0, u1, v1]) = uv_rect {
        let du = (u1 - u0).abs().max(1e-6);
        let dv = (v1 - v0).abs().max(1e-6);
        ([du, dv], [u0.min(u1), v0.min(v1)])
    } else if let Some((cx, cy)) = cell {
        let (gc, gr) = grid.unwrap_or_else(|| font::parse_sheet_dims_from_filename(texture));
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

    uv_offset[0] += uv_scale[0] * cl;
    uv_offset[1] += uv_scale[1] * ct;
    uv_scale[0] *= (1.0 - cl - cr).max(0.0);
    uv_scale[1] *= (1.0 - ct - cb).max(0.0);

    if flip_x { uv_offset[0] += uv_scale[0]; uv_scale[0] = -uv_scale[0]; }
    if flip_y { uv_offset[1] += uv_scale[1]; uv_scale[1] = -uv_scale[1]; }
    
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
    is_solid: bool,
    texture_id: &str,
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
    fadeleft: f32,
    faderight: f32,
    fadetop: f32,
    fadebottom: f32,
    blend: BlendMode,
    rot_z_deg: f32,
    texcoordvelocity: Option<[f32; 2]>,
    total_elapsed: f32,
) {
    if tint[3] <= 0.0 { return; }

    let (cl, cr, ct, cb) = clamp_crop_fractions(cropleft, cropright, croptop, cropbottom);

    let (base_center, base_size) = sm_rect_to_world_center_size(rect, m);
    if base_size.x <= 0.0 || base_size.y <= 0.0 { return; }

    let sx_crop = (1.0 - cl - cr).max(0.0);
    let sy_crop = (1.0 - ct - cb).max(0.0);
    if sx_crop <= 0.0 || sy_crop <= 0.0 { return; }

    let center_x = base_center.x;
    let center_y = base_center.y;

    let size_x = base_size.x * sx_crop;
    let size_y = base_size.y * sy_crop;

    let (uv_scale, uv_offset) = if is_solid {
        ([1.0, 1.0], [0.0, 0.0])
    } else {
        calculate_uvs(
            texture_id, uv_rect, cell, grid,
            flip_x, flip_y,
            cl, cr, ct, cb,
            texcoordvelocity, total_elapsed,
        )
    };

    let fl = fadeleft.clamp(0.0, 1.0);
    let fr = faderight.clamp(0.0, 1.0);
    let ft = fadetop.clamp(0.0, 1.0);
    let fb = fadebottom.clamp(0.0, 1.0);

    let mut fl_eff = ((fl - cl).max(0.0) / sx_crop).clamp(0.0, 1.0);
    let mut fr_eff = ((fr - cr).max(0.0) / sx_crop).clamp(0.0, 1.0);
    let mut ft_eff = ((ft - ct).max(0.0) / sy_crop).clamp(0.0, 1.0);
    let mut fb_eff = ((fb - cb).max(0.0) / sy_crop).clamp(0.0, 1.0);

    if flip_x { std::mem::swap(&mut fl_eff, &mut fr_eff); }
    if flip_y { std::mem::swap(&mut ft_eff, &mut fb_eff); }

    let transform =
        Matrix4::from_translation(Vector3::new(center_x, center_y, 0.0)) *
        Matrix4::from_angle_z(Deg(rot_z_deg)) *
        Matrix4::from_nonuniform_scale(size_x, size_y, 1.0);

    let final_texture_id = if is_solid { "__white".to_string() } else { texture_id.to_string() };

    out.push(renderer::RenderObject {
        object_type: renderer::ObjectType::Sprite {
            texture_id: final_texture_id,
            tint,
            uv_scale,
            uv_offset,
            edge_fade: [fl_eff, fr_eff, ft_eff, fb_eff],
        },
        transform,
        blend,
        z: 0,
        order: 0,
    });
}

#[inline(always)]
fn clamp_crop_fractions(l: f32, r: f32, t: f32, b: f32) -> (f32, f32, f32, f32) {
    (
        l.clamp(0.0, 1.0),
        r.clamp(0.0, 1.0),
        t.clamp(0.0, 1.0),
        b.clamp(0.0, 1.0),
    )
}

#[inline(always)]
fn line_width_no_overlap_px(font: &font::Font, text: &str, scale_x: f32) -> i32 {
    // simulate the renderer with a "no-overlap" pen
    let mut pen = 0.0f32;
    let mut last_right = f32::NEG_INFINITY;

    for ch in text.chars() {
        let mapped = font.glyph_map.get(&ch);
        let g = match mapped.or(font.default_glyph.as_ref()) {
            Some(g) => g,
            None => continue, // completely unmapped and no default: skip
        };

        // StepMania parity for missing SPACE: advance only; no quad
        let should_draw_quad = !(ch == ' ' && mapped.is_none());

        if should_draw_quad {
            // ensure that the snapped left edge of this quad won't cross the previous right edge
            // pen must be large enough that round(pen + off) >= last_right
            let need_pen = (last_right - g.offset[0] * scale_x - 0.5).ceil();
            if pen < need_pen { pen = need_pen; }

            // snapped draw position, like the renderer
            let draw_x = (pen + g.offset[0] * scale_x).round();
            let right  = draw_x + g.size[0] * scale_x;
            if right > last_right { last_right = right; }
        }

        // logical advance (always applied)
        pen += g.advance * scale_x;
    }

    // true visible width is the max of where the pen ended and the furthest drawn pixel
    last_right.max(pen).round() as i32
}

#[inline(always)]
fn place_no_overlap_and_get_draw_x(
    pen_x: &mut f32,
    last_right: &mut f32,
    g: &font::Glyph,
    scale_x: f32,
) -> f32 {
    // bump pen so that after snapping, left edge >= last_right
    let need_pen = (*last_right - g.offset[0] * scale_x - 0.5).ceil();
    if *pen_x < need_pen { *pen_x = need_pen; }

    let draw_x = (*pen_x + g.offset[0] * scale_x).round();
    let right  = draw_x + g.size[0] * scale_x;
    if right > *last_right { *last_right = right; }
    draw_x
}

fn layout_text(
    font: &font::Font,
    text: &str,
    _px_size: f32, // intentionally unused; bitmap font is intrinsic + scale
    scale: [f32; 2],
    fit_width: Option<f32>,
    fit_height: Option<f32>,
    parent: SmRect,
    align: [f32; 2],
    offset: [f32; 2],
    text_align: actors::TextAlign,
    m: &Metrics,
) -> Vec<RenderObject> {
    if text.is_empty() { return vec![]; }

    // 1) split
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() { return vec![]; }

    // 2) measure *unscaled logical* widths (for fit calc, like SM)
    let logical_line_widths: Vec<f32> =
        lines.iter().map(|l| line_width_no_overlap_px(font, l, 1.0) as f32).collect();
    let max_logical_width = logical_line_widths.iter().fold(0.0f32, |a, &b| a.max(b));

    // Vertical metrics (SM: cap height = baseline - top from main page)
    let cap_height = if font.height > 0 { font.height as f32 } else { font.line_spacing as f32 };
    let num_lines = lines.len();
    let unscaled_block_height = if num_lines > 1 {
        cap_height + ((num_lines - 1) as f32 * font.line_spacing as f32)
    } else {
        cap_height
    };

    // 3) fit scale (uniform; min of width/height constraints)
    use std::f32::INFINITY;
    let s_w = fit_width.map_or(INFINITY, |w| if max_logical_width > 0.0 { w / max_logical_width } else { 1.0 });
    let s_h = fit_height.map_or(INFINITY, |h| if unscaled_block_height > 0.0 { h / unscaled_block_height } else { 1.0 });
    let fit_s = if s_w.is_infinite() && s_h.is_infinite() { 1.0 } else { s_w.min(s_h).max(0.0) };

    let final_scale_x = scale[0] * fit_s;
    let final_scale_y = scale[1] * fit_s;
    if final_scale_x.abs() < 1e-6 || final_scale_y.abs() < 1e-6 { return vec![]; }

    // 4) measure widths in *screen pixels* with cumulative (float) advance; snap once
    let line_widths_px: Vec<i32> =
        lines.iter().map(|l| line_width_no_overlap_px(font, l, final_scale_x)).collect();
    let max_line_width_px = *line_widths_px.iter().max().unwrap_or(&0);

    // 5) place the text block
    let scaled_block_width  = max_line_width_px as f32;
    let scaled_block_height = unscaled_block_height * final_scale_y;

    let block_top_sm_y  = parent.y + offset[1] - align[1] * scaled_block_height;
    let block_left_sm_x = parent.x + offset[0] - align[0] * scaled_block_width;

    // First baseline at cap height; pixel-aligned
    let mut current_baseline_sm_y = (block_top_sm_y + cap_height * final_scale_y).round();

    // 6) build render objects
    let mut objects = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        let line_w_px = line_widths_px[i] as f32;

        // Start pen based on actual pixel width of this line; pixel-aligned
        let pen_start_x = match text_align {
            actors::TextAlign::Left   => block_left_sm_x,
            actors::TextAlign::Center => block_left_sm_x + 0.5 * (scaled_block_width - line_w_px),
            actors::TextAlign::Right  => block_left_sm_x + (scaled_block_width - line_w_px),
        }.round();

        // Accumulate in float (SM behavior)
        let mut pen_x_sm = pen_start_x;
        let mut last_right_edge_sm = f32::NEG_INFINITY;

        for c in line.chars() {
            let mapped = font.glyph_map.get(&c);
            let glyph = match mapped.or(font.default_glyph.as_ref()) {
                Some(g) => g,
                None => continue,
            };

            let should_draw_quad = !(c == ' ' && mapped.is_none());

            if should_draw_quad {
                // sizes (unchanged)
                let quad_w = glyph.size[0] * final_scale_x;
                let quad_h = glyph.size[1] * final_scale_y;

                if quad_w.abs() >= 1e-6 && quad_h.abs() >= 1e-6 {
                    // NEW: get a non-overlapping, pixel-snapped x
                    let quad_x_sm = place_no_overlap_and_get_draw_x(
                        &mut pen_x_sm, &mut last_right_edge_sm, glyph, final_scale_x
                    );
                    let quad_y_sm = (current_baseline_sm_y + glyph.offset[1] * final_scale_y).round();

                    let center_x = m.left + quad_x_sm + quad_w * 0.5;
                    let center_y = m.top  - (quad_y_sm + quad_h * 0.5);

                    let transform = Matrix4::from_translation(Vector3::new(center_x, center_y, 0.0))
                                * Matrix4::from_nonuniform_scale(quad_w, quad_h, 1.0);

                    let (tex_w, tex_h) = assets::texture_dims(&glyph.texture_key)
                        .map_or((1.0, 1.0), |meta| (meta.w as f32, meta.h as f32));

                    let uv_scale = [
                        (glyph.tex_rect[2] - glyph.tex_rect[0]) / tex_w,
                        (glyph.tex_rect[3] - glyph.tex_rect[1]) / tex_h,
                    ];
                    let uv_offset = [
                        glyph.tex_rect[0] / tex_w,
                        glyph.tex_rect[1] / tex_h,
                    ];

                    objects.push(RenderObject {
                        object_type: renderer::ObjectType::Sprite {
                            texture_id: glyph.texture_key.clone(),
                            tint: [1.0; 4],
                            uv_scale,
                            uv_offset,
                            edge_fade: [0.0; 4],
                        },
                        transform,
                        blend: BlendMode::Alpha,
                        z: 0,
                        order: 0,
                    });
                }
            }

            // advance pen (always), like before
            pen_x_sm += glyph.advance * final_scale_x;
        }

        // Next line (pixel-aligned)
        current_baseline_sm_y += (font.line_spacing as f32 * final_scale_y).round();
    }

    objects
}

#[inline(always)]
fn measure_line_width_px(font: &font::Font, text: &str, scale_x: f32) -> i32 {
    let mut pen = 0.0f32;
    for ch in text.chars() {
        if let Some(g) = font.glyph_map.get(&ch).or(font.default_glyph.as_ref()) {
            pen += g.advance * scale_x; // accumulate in float
        }
    }
    pen.round() as i32 // snap once for layout math
}

#[inline(always)]
fn sm_rect_to_world_center_size(rect: SmRect, m: &Metrics) -> (Vector2<f32>, Vector2<f32>) {
    (
        Vector2::new(m.left + rect.x + 0.5 * rect.w, m.top - (rect.y + 0.5 * rect.h)),
        Vector2::new(rect.w, rect.h),
    )
}
