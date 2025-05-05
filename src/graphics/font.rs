use crate::graphics::texture::{load_texture, TextureResource}; // Corrected path
use crate::graphics::vulkan_base::VulkanBase; // Corrected path
use ash::Device;                                                // Keep Device for destroy
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use log::{error, info, warn};
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

// --- Structs (Data Representation) ---

#[derive(Debug, Clone, Copy)]
pub struct FontMetrics {
    pub baseline: f32,
    pub top: f32,
    pub line_spacing: f32,
    pub draw_extra_pixels_left: f32,
    pub draw_extra_pixels_right: f32,
    pub advance_extra_pixels: f32,
    pub cell_width: f32, // Width of a character cell in the texture atlas
    pub cell_height: f32, // Height of a character cell in the texture atlas
}

#[derive(Debug, Clone, Copy)]
pub struct GlyphInfo {
    // Texture coordinates within the font atlas (Normalized 0.0 to 1.0)
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    // Metrics for layout (in pixels)
    pub advance: f32,     // How much to move the cursor horizontally
    pub bearing_x: f32, // Offset from cursor X to glyph's left edge
    pub bearing_y: f32, // Offset from baseline to glyph's top edge
    pub width_pixels: f32, // Actual pixel width of the glyph (from width map)
    pub height_pixels: f32, // Actual pixel height of the glyph (usually cell_height)
}

pub struct Font {
    pub metrics: FontMetrics,
    pub glyphs: HashMap<char, GlyphInfo>,
    pub texture: TextureResource, // Keep the texture resource handle
    pub line_height: f32,         // Calculated line height (usually metrics.line_spacing)
    pub space_width: f32,         // Width of a space character
}

impl Font {
    pub fn destroy(&mut self, device: &Device) {
         log::debug!("Destroying Font resources (Texture: {:?})", self.texture.image);
        self.texture.destroy(device);
         log::debug!("Font resources destroyed.");
    }

    /// Measures the pixel width of a given string using this font.
    pub fn measure_text(&self, text: &str) -> f32 {
        let mut width = 0.0;
        for char_code in text.chars() {
            if let Some(glyph) = self.glyphs.get(&char_code) {
                width += glyph.advance;
            } else if char_code == ' ' {
                width += self.space_width;
            } else if char_code == '\n' {
                // Newlines don't add width in this context
            } else if let Some(fallback) = self.glyphs.get(&'?') {
                // Use fallback '?' if available
                width += fallback.advance;
                 warn!("Character '{}' not found in font, using fallback '?' for width calculation.", char_code);
            } else {
                 // If even '?' is missing, use space width as a last resort
                 width += self.space_width;
                 error!("Character '{}' and fallback '?' not found in font! Using space width.", char_code);
            }
        }
        width
    }

     /// Gets glyph info for a character, falling back to '?' if necessary.
     pub fn get_glyph(&self, char_code: char) -> Option<&GlyphInfo> {
         self.glyphs.get(&char_code).or_else(|| {
             if char_code != '?' { // Avoid infinite loop if '?' is missing
                 warn!(
                     "Character '{}' (unicode {}) not found in font map (size {}), trying fallback '?'.",
                     char_code,
                     char_code as u32,
                     self.glyphs.len()
                 );
                 self.glyphs.get(&'?')
             } else {
                 None // Already tried '?' and it's not there
             }
         })
     }
}

// --- Font Loading (Remains complex due to INI format) ---
pub fn load_font(
    base: &VulkanBase, // Needed only to pass to load_texture
    ini_path: &Path,
    texture_path: &Path,
) -> Result<Font, Box<dyn Error>> {
    log::info!("Loading font from INI: {:?}", ini_path);
    log::debug!("Texture path: {:?}", texture_path);

    // --- 1. Parse INI ---
    let file = File::open(ini_path).map_err(|e| format!("Failed to open INI file {:?}: {}", ini_path, e))?;
    let reader = BufReader::new(file);

    let mut current_section: Option<String> = None; // None represents the root section
    let mut common_metrics_map: HashMap<String, String> = HashMap::new();
    let mut main_lines_map: HashMap<usize, String> = HashMap::new();
    let mut width_map: HashMap<u32, f32> = HashMap::new();

    log::debug!("Starting manual INI parsing...");
    for (line_num, line_result) in reader.lines().enumerate() {
        let line = line_result.map_err(|e| format!("Failed to read line {} from {:?}: {}", line_num + 1, ini_path, e))?;
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }

        // Check for section headers
        if line.starts_with('[') && line.ends_with(']') {
            let section_name = line[1..line.len() - 1].trim().to_lowercase();
            current_section = Some(section_name);
            log::trace!("Line {}: Switched to section: [{:?}]", line_num + 1, current_section.as_deref().unwrap_or("root"));
            continue;
        }

        // Parse key-value pairs
        if let Some(eq_pos) = line.find('=') {
            let key_original_case = line[..eq_pos].trim();
            let key_lower_case = key_original_case.to_lowercase();
            let value = line[eq_pos + 1..].trim().to_string();

            match current_section.as_deref() {
                Some("common") => {
                    common_metrics_map.insert(key_lower_case, value);
                }
                Some("main") => {
                    if key_lower_case.starts_with("line") {
                        if let Some(num_part) = key_lower_case.strip_prefix("line") {
                            if let Ok(num) = num_part.trim().parse::<usize>() {
                                main_lines_map.insert(num, value); // value retains original case if needed by glyph map later
                            } else {
                                warn!("Line {}: Could not parse line number from key '{}' in [main]", line_num + 1, key_original_case);
                            }
                        }
                    } else {
                        warn!("Line {}: Unexpected key '{}' in [main] section", line_num + 1, key_original_case);
                    }
                }
                None => { // Root section for widths
                    if let Ok(char_code_idx) = key_original_case.parse::<u32>() {
                        if let Ok(width) = value.parse::<f32>() {
                            width_map.insert(char_code_idx, width);
                        } else {
                            warn!("Line {}: Invalid width value for root key '{}': {}", line_num + 1, key_original_case, value);
                        }
                    } // Ignore non-numeric root keys silently
                }
                Some(other_section) => {
                    log::trace!("Line {}: Ignoring key '{}' in unknown section '{}'", line_num + 1, key_original_case, other_section);
                }
            }
        } else {
            warn!("Line {}: Malformed line in INI file (missing '='): {}", line_num + 1, line);
        }
    }
    log::debug!("Finished manual INI parsing: {} common, {} lines, {} widths", common_metrics_map.len(), main_lines_map.len(), width_map.len());

    // Helper closure to parse f32 from the common metrics map
    let parse_f32 = |map: &HashMap<String, String>, key: &str, default: Option<f32>| -> Result<f32, String> {
        match map.get(key) {
            Some(s) => s.parse::<f32>().map_err(|e| format!("Invalid float value for '{}': {} ({})", key, s, e)),
            None => default.ok_or_else(|| format!("Missing required key '{}' in [common] section", key)),
        }
    };

    // --- 2. Parse [common] Metrics ---
    log::debug!("Parsing common metrics from manually parsed data...");

    let baseline = parse_f32(&common_metrics_map, "baseline", None).map_err(|e| Box::<dyn Error>::from(e))?;
    let top = parse_f32(&common_metrics_map, "top", None).map_err(|e| Box::<dyn Error>::from(e))?;
    let line_spacing = parse_f32(&common_metrics_map, "linespacing", None).map_err(|e| Box::<dyn Error>::from(e))?;

    // Optional metrics with defaults
    let draw_extra_pixels_left = parse_f32(&common_metrics_map, "drawextrapixelsleft", Some(0.0))?;
    let draw_extra_pixels_right = parse_f32(&common_metrics_map, "drawextrapixelsright", Some(0.0))?;
    let advance_extra_pixels = parse_f32(&common_metrics_map, "advanceextrapixels", Some(0.0))?;

    log::info!(
        "Parsed Common Metrics: Baseline={}, Top={}, LineSpacing={}",
        baseline, top, line_spacing
    );

    // --- 3. Load Texture ---
    let texture = load_texture(base, texture_path)?;
    let tex_width = texture.width as f32;
    let tex_height = texture.height as f32;
    log::info!("Font texture loaded: {}x{}", tex_width, tex_height);

    // --- 4. Determine Grid Size & Get Line Keys ---
    if main_lines_map.is_empty() {
        return Err("No 'LineX' entries found in [main] section".into());
    }

    let mut line_keys: Vec<usize> = main_lines_map.keys().copied().collect();
    line_keys.sort_unstable();
    log::debug!("Found and sorted line keys: {:?}", line_keys);

    let mut max_row: u32 = 0;
    let mut max_col: u32 = 0;

    log::debug!("Determining grid size from line content...");
    for &row_idx_usize in &line_keys {
        if let Some(line_str) = main_lines_map.get(&row_idx_usize) {
            max_row = max_row.max(row_idx_usize as u32);
            // Number of columns is number of characters - 1 (0-based index)
            let current_max_col = line_str.chars().count().saturating_sub(1) as u32;
            max_col = max_col.max(current_max_col);
        }
    }
    let num_rows = max_row + 1;
    let num_cols = max_col + 1;
    log::info!("Determined font grid size: {} rows, {} cols", num_rows, num_cols);

    // Calculate cell size (can be non-integer)
    let cell_width = tex_width / num_cols as f32;
    let cell_height = tex_height / num_rows as f32;
    log::info!("Calculated cell size: {}x{}", cell_width, cell_height);

    let metrics = FontMetrics {
        baseline,
        top,
        line_spacing,
        draw_extra_pixels_left,
        draw_extra_pixels_right,
        advance_extra_pixels,
        cell_width,
        cell_height,
    };

    // --- Parse Width Map (from root section) ---
    // The `width_map` is already populated by the manual parser above.

    // --- 5. Build Glyph Map ---
    let mut glyphs: HashMap<char, GlyphInfo> = HashMap::new();
    log::info!("Building Glyph Map...");
    for &row_idx_usize in &line_keys {
        let row_idx = row_idx_usize as u32;
        if let Some(line_str) = main_lines_map.get(&row_idx_usize) {
             log::trace!("Processing Line {}: '{}'", row_idx_usize, line_str);

            for (col_idx, char_code) in line_str.chars().enumerate() {
                // Skip null char or zero-width space sometimes used as placeholders
                if (char_code as u32) == 0 || char_code == '\u{200b}' {
                    continue;
                }

                let grid_pos = (row_idx, col_idx as u32);
                let char_unicode_idx = char_code as u32;

                // Get width from map, fall back if missing
                let width_pixels = match width_map.get(&char_unicode_idx) {
                    Some(&w) => w,
                    None => {
                        // Don't warn for space, it often doesn't have an explicit width entry
                        if char_code != ' ' {
                             warn!(
                                "Missing width for char '{}' (unicode {}) in INI width map. Using cell width {:.1} as fallback.",
                                char_code, char_unicode_idx, metrics.cell_width
                            );
                        }
                         // Use cell width as fallback if no explicit width found
                         //metrics.cell_width // <-- CORRECTED Fallback
                         56.0
                    }
                };

                // Calculate UV coordinates (normalized 0.0 to 1.0)
                let u0 = grid_pos.1 as f32 * metrics.cell_width / tex_width;
                let v0 = grid_pos.0 as f32 * metrics.cell_height / tex_height;
                // Use cell size for UV scale, actual glyph might be smaller
                let u1 = u0 + metrics.cell_width / tex_width;
                let v1 = v0 + metrics.cell_height / tex_height;


                // Calculate layout metrics
                let advance = width_pixels + metrics.advance_extra_pixels;
                // Bearing X: Offset from cursor to left edge, considering draw extra pixels
                let bearing_x = metrics.draw_extra_pixels_left;
                 // Bearing Y: Offset from baseline to top edge. Top edge is baseline - bearing_y.
                 // The font format seems inconsistent here. Sometimes 'top' is distance from
                 // top texture edge to top of highest glyph? Let's assume bearing_y needs to place
                 // the glyph relative to the baseline provided.
                 // A common definition is baseline - ascent. Let's try baseline - top.
                let bearing_y = metrics.baseline - metrics.top; // Positive value means glyph top is above baseline

                // Height in pixels is typically the cell height for rendering quads
                let height_pixels = metrics.cell_height;

                let glyph_info = GlyphInfo {
                    u0, v0, u1, v1,
                    advance,
                    bearing_x,
                    bearing_y,
                    width_pixels,
                    height_pixels,
                };

                // log::trace!(
                //     "  -> Inserting char: '{}' (unicode: {}, grid: {:?}, width: {}, advance: {}, uv: [{:.3},{:.3} .. {:.3},{:.3}])",
                //     char_code, char_unicode_idx, grid_pos, width_pixels, advance, u0,v0,u1,v1
                // );
                glyphs.insert(char_code, glyph_info);
            }
        }
    }
    log::info!("Finished Building Glyph Map. {} glyphs mapped.", glyphs.len());

    // Debugging checks
    if !glyphs.contains_key(&'?') {
        warn!("Font Check: Fallback character '?' was NOT found in the loaded glyphs!");
    } else {
        info!("Font Check: Fallback character '?' was loaded successfully.");
    }
    if !glyphs.contains_key(&' ') {
         // Add a default space if it wasn't in the INI
         if let Some(a_glyph) = glyphs.get(&'a').or_else(|| glyphs.get(&'A')) {
             warn!("Font Check: Space character ' ' was NOT found. Creating default based on 'a'/'A'.");
             let space_width = a_glyph.advance * 0.5; // Estimate space width
             glyphs.insert(' ', GlyphInfo {
                 u0: 0.0, v0: 0.0, u1: 0.0, v1: 0.0, // No visual representation
                 advance: space_width,
                 bearing_x: 0.0, bearing_y: 0.0,
                 width_pixels: 0.0, height_pixels: 0.0,
             });
         } else {
            error!("Font Check: Space character ' ' was NOT found, and could not create default!");
         }

    } else {
        info!("Font Check: Space character ' ' was loaded successfully.");
    }

    // --- Final Calculations ---
    let line_height = metrics.line_spacing;
    let space_width = glyphs.get(&' ').map_or_else(
        || {
            error!("Could not determine space width! Using fallback value 8.0.");
            8.0 // Last resort fallback
        },
        |g| g.advance,
    );
    log::info!("Calculated line_height: {}, space_width: {}", line_height, space_width);

    log::info!("Font loading complete for {:?}.", ini_path);

    Ok(Font {
        metrics,
        glyphs,
        texture,
        line_height,
        space_width,
    })
}
