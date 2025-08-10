use cgmath::Vector2;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone)]
pub struct Font {
    pub atlas_tex_key: &'static str,
    pub atlas_w: f32,
    pub atlas_h: f32,
    pub line_h: f32,
    pub px_range: f32,
    pub glyphs: HashMap<char, Glyph>,
    pub space_advance: f32,
}

#[derive(Clone)]
pub struct Glyph {
    // Atlas rect in pixels (we store top-left origin for Y to match v=0 at top)
    pub atlas_x: f32,
    pub atlas_y: f32,
    pub atlas_w: f32,
    pub atlas_h: f32,
    // Layout metrics in “font units”; scaled by (pixel_height / line_h)
    pub xoff: f32,
    pub yoff: f32,   // positive is up in source; we flip to down in layout
    pub xadv: f32,
    pub plane_w: f32,
    pub plane_h: f32,
}

/* ======================= msdf-atlas-gen JSON ======================= */

#[derive(Deserialize)]
struct MsdfRoot {
    atlas: MsdfAtlas,
    metrics: MsdfMetrics,
    glyphs: Vec<MsdfGlyph>,
    // kerning optional/ignored
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MsdfAtlas {
    #[allow(dead_code)]
    r#type: Option<String>,
    distance_range: Option<f32>,
    // size can be scalar or pair/object; width/height may also be present
    #[serde(default)]
    size: Option<MsdfSize>,
    #[serde(default)]
    width: Option<f32>,
    #[serde(default)]
    height: Option<f32>,
    #[serde(default)]
    y_origin: Option<String>, // "bottom" (default in msdf-atlas-gen) or "top"
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MsdfMetrics {
    line_height: f32,
    #[allow(dead_code)]
    em_size: Option<f32>,
    #[allow(dead_code)]
    ascender: Option<f32>,
    #[allow(dead_code)]
    descender: Option<f32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MsdfGlyph {
    unicode: Option<u32>,
    advance: f32,
    plane_bounds: Option<MsdfBounds>,
    atlas_bounds: Option<MsdfBounds>,
}

#[derive(Deserialize, Clone)]
struct MsdfBounds {
    left: f32,
    bottom: f32,
    right: f32,
    top: f32,
}

// size can be a scalar (square), an array [w,h], or an object {x,y}/{width,height}
#[derive(Deserialize)]
#[serde(untagged)]
enum MsdfSize {
    Scalar(f32),
    Pair([f32; 2]),
    XY { x: f32, y: f32 },
    WH { width: f32, height: f32 },
}

impl MsdfAtlas {
    fn dims(&self) -> (f32, f32) {
        if let (Some(w), Some(h)) = (self.width, self.height) {
            return (w, h);
        }
        if let Some(size) = &self.size {
            return match size {
                MsdfSize::Scalar(s) => (*s, *s),
                MsdfSize::Pair([w, h]) => (*w, *h),
                MsdfSize::XY { x, y } => (*x, *y),
                MsdfSize::WH { width, height } => (*width, *height),
            };
        }
        (0.0, 0.0)
    }
    fn y_origin_bottom(&self) -> bool {
        // default is "bottom" if missing
        !matches!(self.y_origin.as_deref(), Some("top"))
    }
}

/* ======================= Loader & Layout ======================= */

/// Load font from **msdf-atlas-gen** JSON only.
/// `atlas_tex_key` must match the texture key you inserted in the texture manager.
/// `px_range_hint` is used if the JSON doesn't specify `distanceRange`.
pub fn load_font(json_bytes: &[u8], atlas_tex_key: &'static str, px_range_hint: f32) -> Font {
    let f: MsdfRoot = serde_json::from_slice(json_bytes)
        .expect("msdf-atlas-gen JSON");

    let (atlas_w, atlas_h) = f.atlas.dims();
    let y_bottom = f.atlas.y_origin_bottom();

    let mut glyphs = HashMap::new();

    for g in &f.glyphs {
        let Some(code) = g.unicode else { continue; };
        let Some(ch) = std::char::from_u32(code) else { continue; };

        // Layout metrics from planeBounds (font units, Y up in data)
        let (xoff, yoff, plane_w, plane_h) = if let Some(pb) = &g.plane_bounds {
            let w = (pb.right - pb.left).abs();
            let h = (pb.top - pb.bottom).abs();
            // Our layout uses +Y down from baseline; flip sign
            (pb.left, -pb.top, w, h)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };

        // Atlas rectangle in pixels.
        // msdf-atlas-gen gives bottom/top depending on yOrigin.
        let (ax, ay, aw, ah) = if let Some(ab) = &g.atlas_bounds {
            let w = (ab.right - ab.left).abs();
            let h = (ab.top - ab.bottom).abs();

            // Store atlas_y as top-left Y to match our unit quad UVs (v=0 is top)
            let y_top_left = if y_bottom {
                // bottom-origin: ab.top is measured from bottom -> convert
                atlas_h - ab.top
            } else {
                // top-origin: ab.top already measured from top
                ab.top
            };

            (ab.left, y_top_left, w, h)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };

        glyphs.insert(ch, Glyph {
            atlas_x: ax, atlas_y: ay, atlas_w: aw, atlas_h: ah,
            xoff, yoff, xadv: g.advance,
            plane_w, plane_h,
        });
    }

    let space_advance = glyphs.get(&' ').map(|g| g.xadv).unwrap_or(0.5);

    Font {
        atlas_tex_key,
        atlas_w,
        atlas_h,
        line_h: f.metrics.line_height,
        px_range: f.atlas.distance_range.unwrap_or(px_range_hint),
        glyphs,
        space_advance,
    }
}

/// A positioned, sized glyph quad plus its atlas UV mapping.
pub struct LaidGlyph {
    pub center: Vector2<f32>,
    pub size: Vector2<f32>,
    pub uv_scale: [f32; 2],
    pub uv_offset: [f32; 2],
}

/// Layout a single-line string to glyph quads (MSDF atlas subrect + world size/position).
/// Missing glyphs advance by `space_advance` so text still flows.
pub fn layout_line(font: &Font, text: &str, pixel_height: f32, origin: Vector2<f32>) -> Vec<LaidGlyph> {
    let scale = if font.line_h != 0.0 { pixel_height / font.line_h } else { 1.0 };
    let mut pen_x = origin.x;
    let baseline_y = origin.y;

    let mut out = Vec::with_capacity(text.len());

    for ch in text.chars() {
        if ch == '\n' {
            continue; // single-line layout
        }

        // Fetch glyph; if missing, advance by space and continue.
        let Some(g) = font.glyphs.get(&ch) else {
            pen_x += font.space_advance * scale;
            continue;
        };

        let w = g.plane_w * scale;
        let h = g.plane_h * scale;

        // baseline-left origin: center the quad
        let cx = pen_x + (g.xoff * scale) + w * 0.5;
        let cy = baseline_y - (g.yoff * scale) - h * 0.5;

        let uv_scale = if font.atlas_w > 0.0 && font.atlas_h > 0.0 {
            [g.atlas_w / font.atlas_w, g.atlas_h / font.atlas_h]
        } else { [0.0, 0.0] };

        let uv_offset = if font.atlas_w > 0.0 && font.atlas_h > 0.0 {
            [g.atlas_x / font.atlas_w, g.atlas_y / font.atlas_h]
        } else { [0.0, 0.0] };

        pen_x += g.xadv * scale;

        // Skip zero-area glyphs (just advance)
        if w <= 0.0 || h <= 0.0 || uv_scale[0] <= 0.0 || uv_scale[1] <= 0.0 {
            continue;
        }

        out.push(LaidGlyph {
            center: Vector2::new(cx, cy),
            size: Vector2::new(w, h),
            uv_scale,
            uv_offset,
        });
    }

    out
}
