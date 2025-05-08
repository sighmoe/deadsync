use crate::graphics::texture::{load_texture, TextureResource};
use crate::graphics::vulkan_base::VulkanBase;
use ash::Device;
use std::fs::File;
use std::io::{BufRead, BufReader};
use log::{error, info, warn, trace, debug};
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

// --- Structs (Data Representation) ---

#[derive(Debug, Clone, Copy)]
pub struct FontMetrics {
    pub baseline: f32,
    pub top: f32,
    pub line_spacing: f32,
    pub letter_spacing: f32,
    pub cell_width: f32,
    pub cell_height: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct GlyphInfo {
    pub u0: f32, pub v0: f32, pub u1: f32, pub v1: f32,
    pub advance: f32,
    pub bearing_x: f32, // Offset from cell left to visual glyph left
    pub bearing_y: f32, // Offset from baseline up to visual glyph top
    pub visual_width_pixels: f32, // The width of the ink/visual part
    pub visual_height_pixels: f32, // The height of the ink/visual part
}

pub struct Font {
    pub metrics: FontMetrics,
    pub glyphs: HashMap<char, GlyphInfo>,
    pub texture: TextureResource,
    pub line_height: f32,
    pub space_width: f32,
}

impl Font {
    pub fn destroy(&mut self, device: &Device) {
        log::debug!("Destroying Font resources (Texture: {:?})", self.texture.image);
        self.texture.destroy(device);
        log::debug!("Font resources destroyed.");
    }

    pub fn measure_text(&self, text: &str) -> f32 {
        let mut width = 0.0;
        for char_code in text.chars() {
            if let Some(glyph) = self.glyphs.get(&char_code) {
                width += glyph.advance;
            } else if char_code == ' ' {
                width += self.space_width;
            } else if char_code == '\n' {
            } else if let Some(fallback) = self.glyphs.get(&'?') {
                width += fallback.advance;
                warn!("Character '{}' not found in font, using fallback '?' for width calculation.", char_code);
            } else {
                width += self.space_width;
                error!("Character '{}' and fallback '?' not found in font! Using space width.", char_code);
            }
        }
        width
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
    ini_path: &Path,
    texture_path: &Path,
) -> Result<Font, Box<dyn Error>> {
    info!("--- Font Loading Started: {:?} ---", ini_path);

    let file = File::open(ini_path).map_err(|e| format!("Failed to open INI file {:?}: {}", ini_path, e))?;
    let reader = BufReader::new(file);

    let mut current_section: Option<String> = None;
    let mut common_metrics_map: HashMap<String, String> = HashMap::new();
    let mut main_lines_map: HashMap<usize, String> = HashMap::new();
    let mut global_index_char_widths: HashMap<usize, f32> = HashMap::new();

    debug!("--- INI Parsing Phase ---");
    for (line_num, line_result) in reader.lines().enumerate() {
        let line = line_result.map_err(|e| format!("Failed to read line {} from {:?}: {}", line_num + 1, ini_path, e))?.trim().to_string();
        trace!("Raw line {}: '{}'", line_num + 1, line);
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            current_section = Some(line[1..line.len() - 1].trim().to_lowercase());
            debug!("Switched to section: [{:?}]", current_section.as_deref().unwrap_or("root"));
            continue;
        }

        if let Some(eq_pos) = line.find('=') {
            let key_original_case = line[..eq_pos].trim();
            let key_lower_case = key_original_case.to_lowercase();
            let value_str = line[eq_pos + 1..].trim().to_string();
            trace!("  Key: '{}', Value: '{}', Section: {:?}", key_original_case, value_str, current_section.as_deref());

            match current_section.as_deref() {
                Some("common") => {
                    common_metrics_map.insert(key_lower_case, value_str);
                }
                Some("main") => {
                    let key_trimmed = key_original_case.trim();
                    if let Ok(global_char_idx) = key_trimmed.parse::<usize>() {
                        match value_str.parse::<f32>() {
                            Ok(width_val) => {
                                global_index_char_widths.insert(global_char_idx, width_val);
                                info!("SAVED INDEX WIDTH: Parsed [main] override: GlobalIndex {} = {} (from key '{}')", global_char_idx, width_val, key_original_case);
                            },
                            Err(e) => warn!("Line {}: Invalid f32 width for global index key '{}' in [main]: {} (Error: {})", line_num + 1, key_original_case, value_str, e),
                        }
                    } else if key_lower_case.starts_with("line") {
                        if let Some(num_part) = key_lower_case.strip_prefix("line") {
                            if let Ok(num) = num_part.trim().parse::<usize>() {
                                main_lines_map.insert(num, value_str.clone());
                                trace!("  Stored line {}: '{}'", num, value_str);
                            } else {
                                warn!("Line {}: Could not parse line number from key '{}' in [main]", line_num + 1, key_original_case);
                            }
                        }
                    } else {
                        warn!("Line {}: Unhandled key '{}' in [main] section.", line_num + 1, key_original_case);
                    }
                }
                None => { /* Root section, currently ignored */ }
                Some(other_section) => { /* Unknown section, ignored */ }
            }
        } else { warn!("Line {}: Malformed line (no '='): {}", line_num + 1, line); }
    }
    debug!(
        "INI Parsing Complete. Common keys: {}. Lines mapped: {}. Global Index Width Overrides: {}.",
        common_metrics_map.len(), main_lines_map.len(), global_index_char_widths.len()
    );
    if global_index_char_widths.is_empty() {
        warn!("NO GLOBAL INDEX WIDTH OVERRIDES WERE PARSED FROM [main] SECTION!");
    }


    debug!("--- Common Metrics Processing ---");
    let parse_f32_common = |key: &str, default: Option<f32>| -> Result<f32, String> {
        match common_metrics_map.get(key.to_lowercase().as_str()) {
            Some(s) => s.parse::<f32>().map_err(|e| format!("Invalid float for common key '{}': {} ({})", key, s, e)),
            None => default.ok_or_else(|| format!("Missing required common key '{}'", key)),
        }
    };
    let top_ini = parse_f32_common("Top", None)?;
    let baseline_ini = parse_f32_common("Baseline", None)?;
    let line_spacing_ini = parse_f32_common("LineSpacing", None)?;
    let letter_spacing_ini = parse_f32_common("LetterSpacing", Some(0.0))?;
    let default_width_from_common = parse_f32_common("DefaultWidth", None)?;
    debug!(
        "PARSED COMMON: Top={:.1}, Baseline={:.1}, LineSpacing={:.1}, LetterSpacing={:.1}, DefaultWidth={:.1}",
        top_ini, baseline_ini, line_spacing_ini, letter_spacing_ini, default_width_from_common
    );

    debug!("--- Texture & Grid Calculation ---");
    let texture = load_texture(base, texture_path)?;
    log::info!("Font texture loaded: {}x{}", texture.width, texture.height);
    if main_lines_map.is_empty() { return Err("No 'LineX' entries in [main] section".into()); }
    let mut line_keys: Vec<usize> = main_lines_map.keys().copied().collect();
    line_keys.sort_unstable();
    let mut max_row: u32 = 0;
    let mut max_col: u32 = 0;
    for &row_idx_usize in &line_keys {
        if let Some(line_str) = main_lines_map.get(&row_idx_usize) {
            max_row = max_row.max(row_idx_usize as u32);
            max_col = max_col.max(line_str.chars().count().saturating_sub(1) as u32);
        }
    }
    let num_rows = max_row + 1;
    let num_cols = max_col + 1;
    debug!("Determined font grid: {} rows, {} cols", num_rows, num_cols);
    let cell_w = texture.width as f32 / num_cols as f32;
    let cell_h = texture.height as f32 / num_rows as f32;
    debug!("Calculated cell size: {:.2}x{:.2}", cell_w, cell_h);

    let metrics = FontMetrics {
        top: top_ini, baseline: baseline_ini, line_spacing: line_spacing_ini,
        letter_spacing: letter_spacing_ini,
        cell_width: cell_w, cell_height: cell_h,
    };
    debug!("FontMetrics struct created: {:?}", metrics);

    let default_width_resolved = default_width_from_common;
    debug!("Using DefaultWidth (resolved from common): {:.1}", default_width_resolved);

    let mut glyphs: HashMap<char, GlyphInfo> = HashMap::new();
    let mut current_global_char_index: usize = 0;
    debug!("--- Building Glyph Map (Glyph by Glyph) ---");
    for line_key_idx in 0..=max_row as usize {
        if let Some(line_str) = main_lines_map.get(&line_key_idx) {
            let row_idx_in_atlas = line_key_idx as u32;
            for (col_idx_in_line, char_code) in line_str.chars().enumerate() {
                if (char_code as u32) == 0 || char_code == '\u{200b}' { continue; }

                let visual_width = global_index_char_widths
                    .get(&current_global_char_index)
                    .copied()
                    .unwrap_or(default_width_resolved);
                
                let advance = visual_width + metrics.letter_spacing;
                
                let u0 = col_idx_in_line as f32 * metrics.cell_width / texture.width as f32;
                let v0 = row_idx_in_atlas as f32 * metrics.cell_height / texture.height as f32;
                let u1 = u0 + metrics.cell_width / texture.width as f32;
                let v1 = v0 + metrics.cell_height / texture.height as f32;

                let bearing_x = (metrics.cell_width - visual_width) / 2.0;
                let bearing_y = metrics.baseline - metrics.top; // Ascent
                let visual_height = metrics.baseline - metrics.top; // Same as ascent here

                debug!(
                    "  Char: '{}' (atlas r:{},c:{}, global_idx:{})\n    VisualWidth: {:.1} (from map? {}, fallback? {})\n    LetterSpacing: {:.1}\n    => Advance: {:.1}\n    CellWidth: {:.1}, BearingX: {:.1}\n    Baseline: {:.1}, Top: {:.1} => BearingY (Ascent): {:.1}, VisualHeight: {:.1}",
                    char_code, row_idx_in_atlas, col_idx_in_line, current_global_char_index,
                    visual_width, global_index_char_widths.contains_key(&current_global_char_index), !global_index_char_widths.contains_key(&current_global_char_index),
                    metrics.letter_spacing,
                    advance,
                    metrics.cell_width, bearing_x,
                    metrics.baseline, metrics.top, bearing_y, visual_height
                );

                glyphs.insert(char_code, GlyphInfo {
                    u0, v0, u1, v1, advance, bearing_x, bearing_y,
                    visual_width_pixels: visual_width,
                    visual_height_pixels: visual_height,
                });
                current_global_char_index += 1;
            }
        }
    }
    info!("Finished Building Glyph Map. {} glyphs mapped. Max global index processed: {}", glyphs.len(), current_global_char_index.saturating_sub(1));

    debug!("--- Space Character Handling ---");
    if !glyphs.contains_key(&' ') {
        warn!("Font Check: Space char ' ' not in line defs. Creating default.");
        let space_visual_width = default_width_resolved / 2.0;
        let space_advance = space_visual_width + metrics.letter_spacing;
        glyphs.insert(' ', GlyphInfo {
            u0: 0.0, v0: 0.0, u1: 0.0, v1: 0.0, advance: space_advance, bearing_x: 0.0, bearing_y: 0.0,
            visual_width_pixels: space_visual_width, visual_height_pixels: 0.0,
        });
        debug!("CREATED DEFAULT SPACE: VisualWidth={:.1}, LetterSpacing={:.1}, Advance={:.1}", space_visual_width, metrics.letter_spacing, space_advance);
    } else {
        let space_glyph = glyphs.get(&' ').unwrap(); // Safe to unwrap as we know it's there
        debug!("Space char ' ' found in line defs. VisualWidth={:.1}, LS={:.1}, Advance={:.1}", space_glyph.visual_width_pixels, metrics.letter_spacing, space_glyph.advance);
    }

    let final_line_height = metrics.line_spacing;
    let final_space_width = glyphs.get(&' ').unwrap().advance; // Should always exist now
    info!("Final line_height: {:.1}, final_space_width: {:.1}", final_line_height, final_space_width);
    info!("--- Font Loading Complete: {:?} ---", ini_path);

    Ok(Font {
        metrics, glyphs, texture,
        line_height: final_line_height,
        space_width: final_space_width,
    })
}