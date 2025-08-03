use crate::graphics::renderer::DescriptorSetId;
use crate::graphics::texture::{load_texture, TextureResource};
use crate::graphics::vulkan::VulkanBase;
use ash::Device;
use log::{debug, error, info, warn};
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::Path;

// --- MSDF JSON Structures ---
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct MsdfAtlasMetrics {
    distance_range: f64,
    size: f64,
    width: u32,
    height: u32,
    y_origin: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct MsdfFontMetrics {
    em_size: f64,
    line_height: f64,
    ascender: f64,
    descender: f64,
    underline_y: Option<f64>,
    underline_thickness: Option<f64>,
}

#[derive(Deserialize, Debug, Clone, Copy)]
struct MsdfBounds {
    left: f64,
    bottom: f64,
    right: f64,
    top: f64,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct MsdfGlyph {
    unicode: u32,
    advance: f64,
    plane_bounds: Option<MsdfBounds>,
    atlas_bounds: Option<MsdfBounds>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct MsdfJsonFormat {
    atlas: MsdfAtlasMetrics,
    metrics: MsdfFontMetrics,
    glyphs: Vec<MsdfGlyph>,
}

// --- Internal Structs ---
#[derive(Debug, Clone, Copy)]
pub struct FontMetrics {
    pub em_size: f32,
    pub line_height: f32,
    pub ascender: f32,
    pub descender: f32,
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub msdf_pixel_range: f32,
    pub atlas_font_size_pixels: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct GlyphInfo {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    pub plane_left: f32,
    pub plane_bottom: f32,
    pub plane_right: f32,
    pub plane_top: f32,
    pub advance: f32,
}

pub struct LoadedFontData {
    pub metrics: FontMetrics,
    pub glyphs: HashMap<char, GlyphInfo>,
    pub texture: TextureResource,
    pub space_width: f32,
}

pub struct Font {
    pub metrics: FontMetrics,
    pub glyphs: HashMap<char, GlyphInfo>,
    pub texture: TextureResource,
    pub space_width: f32,
    pub descriptor_set_id: DescriptorSetId,
}

impl Font {
    pub fn destroy(&mut self, device: &Device) {
        log::debug!(
            "Destroying Font resources (Texture: {:?}, DescriptorSetId: {:?})",
            self.texture.image,
            self.descriptor_set_id
        );
        self.texture.destroy(device);
        log::debug!("Font resources destroyed.");
    }

    pub fn measure_text_normalized(&self, text: &str) -> f32 {
        let mut width = 0.0;
        for char_code in text.chars() {
            if let Some(glyph) = self.glyphs.get(&char_code) {
                width += glyph.advance;
            } else if char_code == ' ' {
                width += self.space_width;
            } else if char_code != '\n' {
                // Newlines don't add width
                let fallback_advance = self.glyphs.get(&'?').map_or_else(|| {
                    error!("Character '{}' and fallback '?' not found in MSDF font! Using space width (normalized).", char_code);
                    self.space_width
                }, |fallback_glyph| {
                    warn!("Character '{}' not found in MSDF font, using fallback '?' for width calculation (normalized).", char_code);
                    fallback_glyph.advance
                });
                width += fallback_advance;
            }
        }
        width
    }

    pub fn get_line_height_normalized(&self) -> f32 {
        self.metrics.line_height
    }
    pub fn get_ascender_normalized(&self) -> f32 {
        self.metrics.ascender
    }

    pub fn get_glyph(&self, char_code: char) -> Option<&GlyphInfo> {
        self.glyphs.get(&char_code).or_else(|| {
            if char_code != '?' {
                warn!("Character '{}' (unicode {}) not found in font map (size {}), trying fallback '?'.", char_code, char_code as u32, self.glyphs.len());
                self.glyphs.get(&'?')
            } else { None }
        })
    }
}

fn parse_msdf_json(json_path: &Path) -> Result<MsdfJsonFormat, Box<dyn Error>> {
    let json_string = fs::read_to_string(json_path)
        .map_err(|e| format!("Failed to read MSDF JSON file {:?}: {}", json_path, e))?;
    serde_json::from_str(&json_string)
        .map_err(|e| format!("Failed to parse MSDF JSON {:?}: {}", json_path, e).into())
}

fn process_font_metrics(
    msdf_data: &MsdfJsonFormat,
    atlas_texture_width: u32,
    atlas_texture_height: u32,
) -> FontMetrics {
    FontMetrics {
        em_size: msdf_data.metrics.em_size as f32,
        line_height: msdf_data.metrics.line_height as f32,
        ascender: msdf_data.metrics.ascender as f32,
        descender: msdf_data.metrics.descender as f32,
        atlas_width: atlas_texture_width,
        atlas_height: atlas_texture_height,
        msdf_pixel_range: msdf_data.atlas.distance_range as f32,
        atlas_font_size_pixels: msdf_data.atlas.size as f32,
    }
}

fn process_msdf_glyphs(
    msdf_glyphs: &[MsdfGlyph],
    font_metrics: &FontMetrics,
    atlas_y_origin_is_bottom: bool,
) -> (HashMap<char, GlyphInfo>, Option<f32>) {
    let mut glyphs_map: HashMap<char, GlyphInfo> = HashMap::new();
    let mut space_advance_normalized: Option<f32> = None;

    for msdf_glyph in msdf_glyphs {
        let char_code = match std::char::from_u32(msdf_glyph.unicode) {
            Some(c) => c,
            None => {
                warn!(
                    "LOAD_FONT: Invalid unicode scalar: {} in MSDF JSON. Skipping glyph.",
                    msdf_glyph.unicode
                );
                continue;
            }
        };

        if char_code == ' ' {
            space_advance_normalized = Some(msdf_glyph.advance as f32);
        }

        if let (Some(pb), Some(ab)) = (msdf_glyph.plane_bounds, msdf_glyph.atlas_bounds) {
            let (u0_tex, v0_tex, u1_tex, v1_tex) =
                calculate_glyph_uvs(ab, font_metrics, atlas_y_origin_is_bottom);

            if char_code == 'A' || glyphs_map.is_empty() {
                // Log for 'A' or the first glyph
                debug_glyph_processing(
                    char_code,
                    msdf_glyph,
                    ab,
                    atlas_y_origin_is_bottom,
                    u0_tex,
                    v0_tex,
                    u1_tex,
                    v1_tex,
                    pb,
                );
            }

            glyphs_map.insert(
                char_code,
                GlyphInfo {
                    u0: u0_tex,
                    v0: v0_tex,
                    u1: u1_tex,
                    v1: v1_tex,
                    plane_left: pb.left as f32,
                    plane_bottom: pb.bottom as f32,
                    plane_right: pb.right as f32,
                    plane_top: pb.top as f32,
                    advance: msdf_glyph.advance as f32,
                },
            );
        } else if char_code != ' ' {
            // Non-space characters without bounds are usually an issue
            debug!("LOAD_FONT: Glyph '{}' (unicode {}) has no plane_bounds or atlas_bounds. Advance (norm): {:.4}", char_code, msdf_glyph.unicode, msdf_glyph.advance);
        }
    }
    (glyphs_map, space_advance_normalized)
}

fn calculate_glyph_uvs(
    atlas_bounds: MsdfBounds,
    font_metrics: &FontMetrics,
    atlas_y_origin_is_bottom: bool,
) -> (f32, f32, f32, f32) {
    let u0_px = atlas_bounds.left as f32;
    let u1_px = atlas_bounds.right as f32;
    let v0_tex: f32;
    let v1_tex: f32;

    if atlas_y_origin_is_bottom {
        v0_tex = (font_metrics.atlas_height as f32 - atlas_bounds.top as f32)
            / font_metrics.atlas_height as f32;
        v1_tex = (font_metrics.atlas_height as f32 - atlas_bounds.bottom as f32)
            / font_metrics.atlas_height as f32;
    } else {
        v0_tex = atlas_bounds.top as f32 / font_metrics.atlas_height as f32;
        v1_tex = atlas_bounds.bottom as f32 / font_metrics.atlas_height as f32;
    }
    let u0_tex = u0_px / font_metrics.atlas_width as f32;
    let u1_tex = u1_px / font_metrics.atlas_width as f32;
    (u0_tex, v0_tex, u1_tex, v1_tex)
}

fn debug_glyph_processing(
    char_code: char,
    msdf_glyph: &MsdfGlyph,
    ab: MsdfBounds,
    atlas_y_origin_is_bottom: bool,
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
    pb: MsdfBounds,
) {
    debug!(
        "LOAD_FONT GLYPH ('{}', unicode {}):",
        char_code, msdf_glyph.unicode
    );
    debug!(
        "  Raw atlasBounds (JSON): L{:.1} T{:.1} R{:.1} B{:.1}",
        ab.left, ab.top, ab.right, ab.bottom
    );
    debug!("  atlas_y_origin_is_bottom: {}", atlas_y_origin_is_bottom);
    debug!(
        "  Computed Tex UVs: u0={:.4}, v0(top)={:.4} || u1={:.4}, v1(bottom)={:.4}",
        u0, v0, u1, v1
    );
    debug!(
        "  PlaneBounds (JSON, norm): L{:.4} B{:.4} R{:.4} T{:.4}",
        pb.left, pb.bottom, pb.right, pb.top
    );
    debug!("  Advance (JSON, norm): {:.4}", msdf_glyph.advance);
}

pub fn load_font(
    base: &VulkanBase,
    json_path: &Path,
    texture_path: &Path,
) -> Result<LoadedFontData, Box<dyn Error>> {
    info!(
        "--- MSDF Font Loading Started: {:?} ---",
        json_path.file_name().unwrap_or_default()
    );

    let msdf_data = parse_msdf_json(json_path)?;
    debug!(
        "LOAD_FONT: Raw MSDF JSON Parsed. Atlas: {:?}, Metrics: {:?}, Glyphs: {}",
        msdf_data.atlas,
        msdf_data.metrics,
        msdf_data.glyphs.len()
    );

    let texture = load_texture(base, texture_path)?;
    info!(
        "LOAD_FONT: MSDF Font texture loaded: {}x{} from {:?}",
        texture.width,
        texture.height,
        texture_path.file_name().unwrap_or_default()
    );

    if texture.width != msdf_data.atlas.width || texture.height != msdf_data.atlas.height {
        warn!("LOAD_FONT: Texture dimensions ({}x{}) mismatch JSON atlas ({}x{}). Using texture dimensions.", texture.width, texture.height, msdf_data.atlas.width, msdf_data.atlas.height);
    }

    let font_metrics = process_font_metrics(&msdf_data, texture.width, texture.height);
    info!("LOAD_FONT: Processed FontMetrics: em={:.2}, lineH={:.2}, asc={:.2}, desc={:.2}, atlas={}x{}, pxRange={:.2}, atlasGenSize={:.2}",
        font_metrics.em_size, font_metrics.line_height, font_metrics.ascender, font_metrics.descender,
        font_metrics.atlas_width, font_metrics.atlas_height, font_metrics.msdf_pixel_range, font_metrics.atlas_font_size_pixels);

    let atlas_y_origin_is_bottom = msdf_data
        .atlas
        .y_origin
        .as_deref()
        .unwrap_or("top")
        .eq_ignore_ascii_case("bottom");
    if atlas_y_origin_is_bottom {
        info!("LOAD_FONT: Atlas yOrigin is 'bottom'.");
    } else {
        info!("LOAD_FONT: Atlas yOrigin is 'top'.");
    }

    let (glyphs_map, space_advance_normalized_opt) =
        process_msdf_glyphs(&msdf_data.glyphs, &font_metrics, atlas_y_origin_is_bottom);
    info!("LOAD_FONT: Processed {} glyphs into map.", glyphs_map.len());

    let final_space_width_normalized = space_advance_normalized_opt.unwrap_or_else(|| {
        warn!("LOAD_FONT: Space char (' ') not found in MSDF JSON. Estimating space width (normalized).");
        glyphs_map.get(&'m').map_or(font_metrics.em_size / 3.0, |m_glyph| m_glyph.advance)
    });
    info!(
        "LOAD_FONT: Final space width (normalized): {:.4}",
        final_space_width_normalized
    );

    if !glyphs_map.contains_key(&'?') && !glyphs_map.is_empty() {
        warn!(
            "LOAD_FONT: Fallback char '?' not found. Unknown chars might be blank/use space width."
        );
    }

    info!(
        "--- MSDF Font Loading Complete: {:?} ---",
        json_path.file_name().unwrap_or_default()
    );
    Ok(LoadedFontData {
        metrics: font_metrics,
        glyphs: glyphs_map,
        texture,
        space_width: final_space_width_normalized,
    })
}
