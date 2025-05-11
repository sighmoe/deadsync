// src/graphics/font.rs
use crate::graphics::renderer::DescriptorSetId;
use crate::graphics::texture::{load_texture, TextureResource};
use crate::graphics::vulkan_base::VulkanBase;
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
    size: f64, // This is the nominal font size the atlas was generated for (e.g., from -size param)
    width: u32,
    height: u32,
    y_origin: Option<String>, // "top" or "bottom"
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct MsdfFontMetrics {
    em_size: f64, // Often 1.0 if font metrics are normalized, or can match atlas.size
    line_height: f64,
    ascender: f64,
    descender: f64,
    underline_y: Option<f64>,         // Optional fields
    underline_thickness: Option<f64>, // Optional fields
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
    // kerning: Option<Vec<MsdfKerningPair>>, // If your generator produces kerning
}

// --- Internal Structs (Adapted for MSDF) ---

#[derive(Debug, Clone, Copy)]
pub struct FontMetrics {
    pub em_size: f32, // Font size the MSDF was generated for (often 1.0 if metrics are normalized)
    pub line_height: f32, // Distance between baselines (often normalized)
    pub ascender: f32, // Max height above baseline (often normalized)
    pub descender: f32, // Max depth below baseline (often normalized, usually negative)
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub msdf_pixel_range: f32,       // pxRange used during generation
    pub atlas_font_size_pixels: f32, // The 'size' parameter used for msdf-atlas-gen (e.g., 128)
}

#[derive(Debug, Clone, Copy)]
pub struct GlyphInfo {
    // Texture coordinates in the atlas (0.0 to 1.0 range)
    // v0 = top V coordinate, v1 = bottom V coordinate for standard quad UVs (Y=0 top, Y=1 bottom)
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,

    // Quad plane bounds (relative to cursor_x, cursor_y on baseline, often normalized)
    pub plane_left: f32,
    pub plane_bottom: f32,
    pub plane_right: f32,
    pub plane_top: f32,

    // Horizontal advance to the next character (often normalized)
    pub advance: f32,
}

pub struct LoadedFontData {
    pub metrics: FontMetrics,
    pub glyphs: HashMap<char, GlyphInfo>,
    pub texture: TextureResource,
    pub space_width: f32, // Advance for space character (often normalized)
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

    // Measures text width based on normalized advances. Caller needs to scale by desired pixel size.
    pub fn measure_text_normalized(&self, text: &str) -> f32 {
        let mut width = 0.0;
        for char_code in text.chars() {
            if let Some(glyph) = self.glyphs.get(&char_code) {
                width += glyph.advance;
            } else if char_code == ' ' {
                width += self.space_width;
            } else if char_code == '\n' {
                // Newlines don't add width
            } else if let Some(fallback) = self.glyphs.get(&'?') {
                width += fallback.advance;
                warn!("Character '{}' not found in MSDF font, using fallback '?' for width calculation (normalized).", char_code);
            } else {
                width += self.space_width;
                error!("Character '{}' and fallback '?' not found in MSDF font! Using space width (normalized).", char_code);
            }
        }
        width
    }

    // To get pixel width: measure_text_normalized(text) * text_scale
    // where text_scale is your desired pixel size if emSize is 1.0,
    // or text_scale is (desired_pixel_size / font_metrics.em_size) if emSize is not 1.0.

    pub fn get_line_height_normalized(&self) -> f32 {
        self.metrics.line_height
    }

    pub fn get_ascender_normalized(&self) -> f32 {
        self.metrics.ascender
    }

    pub fn get_glyph(&self, char_code: char) -> Option<&GlyphInfo> {
        self.glyphs.get(&char_code).or_else(|| {
            if char_code != '?' {
                warn!(
                    "Character '{}' (unicode {}) not found in font map (size {}), trying fallback '?'.",
                    char_code, char_code as u32, self.glyphs.len()
                );
                self.glyphs.get(&'?')
            } else { None }
        })
    }
}

pub fn load_font(
    base: &VulkanBase,
    json_path: &Path,
    texture_path: &Path,
) -> Result<LoadedFontData, Box<dyn Error>> {
    info!("--- MSDF Font Loading Started: {:?} ---", json_path);

    let json_string = fs::read_to_string(json_path)
        .map_err(|e| format!("Failed to read MSDF JSON file {:?}: {}", json_path, e))?;
    let msdf_data: MsdfJsonFormat = serde_json::from_str(&json_string)
        .map_err(|e| format!("Failed to parse MSDF JSON {:?}: {}", json_path, e))?;

    debug!("LOAD_FONT: Raw MSDF JSON Parsed. Atlas section: {:?}, Metrics section: {:?}, Glyphs found: {}", 
           msdf_data.atlas, msdf_data.metrics, msdf_data.glyphs.len());

    let texture = load_texture(base, texture_path)?;
    info!(
        "LOAD_FONT: MSDF Font texture loaded: {}x{} from {:?}",
        texture.width, texture.height, texture_path
    );

    if texture.width != msdf_data.atlas.width || texture.height != msdf_data.atlas.height {
        warn!(
            "LOAD_FONT: Texture dimensions ({}x{}) mismatch JSON atlas dimensions ({}x{}). Using texture dimensions from loaded image.",
            texture.width, texture.height, msdf_data.atlas.width, msdf_data.atlas.height
        );
    }

    let font_metrics = FontMetrics {
        em_size: msdf_data.metrics.em_size as f32, // Usually 1.0 if metrics are normalized
        line_height: msdf_data.metrics.line_height as f32,
        ascender: msdf_data.metrics.ascender as f32,
        descender: msdf_data.metrics.descender as f32,
        atlas_width: texture.width,
        atlas_height: texture.height,
        msdf_pixel_range: msdf_data.atlas.distance_range as f32,
        atlas_font_size_pixels: msdf_data.atlas.size as f32, // The -size param value from msdf-atlas-gen
    };
    info!("LOAD_FONT: Processed FontMetrics: em_size(normalized)={:.2}, line_height(norm)={:.2}, ascender(norm)={:.2}, descender(norm)={:.2}, atlas_width={}, atlas_height={}, MSDF_PIXEL_RANGE={:.2}, AtlasGenSizePx={:.2}",
        font_metrics.em_size, font_metrics.line_height, font_metrics.ascender, font_metrics.descender,
        font_metrics.atlas_width, font_metrics.atlas_height, font_metrics.msdf_pixel_range, font_metrics.atlas_font_size_pixels);

    let mut glyphs_map: HashMap<char, GlyphInfo> = HashMap::new();
    let mut space_advance_pixels_normalized: Option<f32> = None;

    let atlas_y_origin_is_bottom = msdf_data
        .atlas
        .y_origin
        .as_deref()
        .unwrap_or("top")
        .to_lowercase()
        == "bottom";
    if atlas_y_origin_is_bottom {
        info!("LOAD_FONT: Atlas yOrigin is 'bottom' (from JSON). Will adjust V coordinates for top-down texture sampling.");
    } else {
        info!("LOAD_FONT: Atlas yOrigin is 'top' (or not specified, default assumed 'top').");
    }

    for msdf_glyph in msdf_data.glyphs {
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
            space_advance_pixels_normalized = Some(msdf_glyph.advance as f32);
        }

        if let (Some(pb), Some(ab)) = (msdf_glyph.plane_bounds, msdf_glyph.atlas_bounds) {
            let u0_px = ab.left as f32;
            let u1_px = ab.right as f32;

            let v0_tex: f32; // V-coordinate for the top edge of the glyph in the texture (0.0 at top of texture)
            let v1_tex: f32; // V-coordinate for the bottom edge of the glyph in the texture (1.0 at bottom of texture)

            if atlas_y_origin_is_bottom {
                // atlasBounds from JSON has Y=0 at the bottom of the atlas image.
                // ab.bottom is the lower Y pixel value (closer to Y=0 of atlas image).
                // ab.top is the higher Y pixel value.
                // Texture coordinates (and image crate) have Y=0 at the top of the image.
                // So, texture V for JSON's ab.top (higher Y value) = (atlas_height - ab.top) / atlas_height
                // And texture V for JSON's ab.bottom (lower Y value) = (atlas_height - ab.bottom) / atlas_height
                // v0_tex should be the smaller V value (top of glyph in texture space)
                // v1_tex should be the larger V value (bottom of glyph in texture space)
                v0_tex = (font_metrics.atlas_height as f32 - ab.top as f32)
                    / font_metrics.atlas_height as f32;
                v1_tex = (font_metrics.atlas_height as f32 - ab.bottom as f32)
                    / font_metrics.atlas_height as f32;
            } else {
                // atlasBounds from JSON has Y=0 at the top of the atlas image.
                // ab.top is the smaller Y pixel value.
                // ab.bottom is the larger Y pixel value.
                // This directly maps to texture V coordinates.
                v0_tex = ab.top as f32 / font_metrics.atlas_height as f32;
                v1_tex = ab.bottom as f32 / font_metrics.atlas_height as f32;
            }

            let u0_tex = u0_px / font_metrics.atlas_width as f32;
            let u1_tex = u1_px / font_metrics.atlas_width as f32;

            if char_code == 'A' || glyphs_map.is_empty() {
                // Log for 'A' and the very first glyph processed
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
                    u0_tex, v0_tex, u1_tex, v1_tex
                );
                debug!(
                    "  PlaneBounds (JSON, norm): L{:.4} B{:.4} R{:.4} T{:.4}",
                    pb.left, pb.bottom, pb.right, pb.top
                );
                debug!("  Advance (JSON, norm): {:.4}", msdf_glyph.advance);
            }

            let glyph_info = GlyphInfo {
                u0: u0_tex,
                v0: v0_tex, // Top V for sampling rect
                u1: u1_tex,
                v1: v1_tex, // Bottom V for sampling rect
                plane_left: pb.left as f32,
                plane_bottom: pb.bottom as f32,
                plane_right: pb.right as f32,
                plane_top: pb.top as f32,
                advance: msdf_glyph.advance as f32,
            };
            glyphs_map.insert(char_code, glyph_info);
        } else if char_code != ' ' {
            debug!("LOAD_FONT: Glyph '{}' (unicode {}) has no plane_bounds or atlas_bounds. Advance (norm): {:.4}", char_code, msdf_glyph.unicode, msdf_glyph.advance);
        }
    }
    info!("LOAD_FONT: Processed {} glyphs into map.", glyphs_map.len());

    let final_space_width_normalized = match space_advance_pixels_normalized {
        Some(sw) => sw,
        None => {
            warn!("LOAD_FONT: Space character (' ') not found or no advance in MSDF JSON. Estimating space width (normalized).");
            if let Some(m_glyph) = glyphs_map.get(&'m') {
                m_glyph.advance
            } else {
                // Use a common typographic heuristic if 'm' is also missing
                font_metrics.em_size / 3.0 // em_size is usually 1.0 here
            }
        }
    };
    info!(
        "LOAD_FONT: Final space width (normalized): {:.4}",
        final_space_width_normalized
    );

    if !glyphs_map.contains_key(&'?') && !glyphs_map.is_empty() {
        // Added !glyphs_map.is_empty() to avoid warning if font is totally empty
        warn!("LOAD_FONT: Fallback character '?' not found in MSDF font. Text rendering for unknown characters might be blank or use space width.");
    }

    info!("--- MSDF Font Loading Complete: {:?} ---", json_path);
    Ok(LoadedFontData {
        metrics: font_metrics,
        glyphs: glyphs_map,
        texture,
        space_width: final_space_width_normalized,
    })
}
