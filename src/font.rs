use crate::texture::{load_texture, TextureResource};
use crate::vulkan_base::VulkanBase;
use ash::{vk, Device};
use cgmath::{Matrix4, Vector3};
use configparser::ini::Ini;
use log::{debug, error, info, warn}; // Import info here
use std::collections::HashMap;
use std::error::Error;
use std::mem;
use std::path::Path;

use crate::PushConstantData; // Import from main

// --- Structs ---

#[derive(Debug, Clone, Copy)]
pub struct FontMetrics {
    pub baseline: f32,
    pub top: f32,
    pub line_spacing: f32,
    pub draw_extra_pixels_left: f32,
    pub draw_extra_pixels_right: f32,
    pub advance_extra_pixels: f32,
    pub cell_width: f32,
    pub cell_height: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct GlyphInfo {
    pub uv_offset: [f32; 2],
    pub uv_scale: [f32; 2],
    pub advance: f32,
    pub bearing_x: f32,
    pub bearing_y: f32,
    pub width_pixels: f32,
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
        self.texture.destroy(device);
    }

    pub fn measure_text(&self, text: &str) -> f32 {
        let mut width = 0.0;
        for char_code in text.chars() {
            if let Some(glyph) = self.glyphs.get(&char_code) {
                width += glyph.advance;
            } else if char_code == ' ' {
                width += self.space_width;
            } else if let Some(fallback) = self.glyphs.get(&'?') {
                width += fallback.advance;
            }
        }
        width
    }
}

// --- Font Loading (Full INI version with logging) ---
pub fn load_font(
    base: &VulkanBase,
    ini_path: &Path,
    texture_path: &Path,
) -> Result<Font, Box<dyn Error>> {
    log::info!("Loading font from INI: {:?}", ini_path);

    // --- 1. Parse INI ---
    let mut config = Ini::new();
    let map = config
        .load(ini_path)
        .map_err(|e| format!("Failed to load/parse INI file {:?}: {}", ini_path, e))?;
    log::debug!("INI Parsed: {:?}", map);

    // --- 2. Parse [common] Metrics ---
    let common = map.get("common").ok_or("Missing [common] section in INI")?;
    info!("--- Common Section Parsed ---"); // Log point 1
    dbg!(common);

    let baseline = common
        .get("baseline")
        .and_then(|opt_s| opt_s.as_deref())
        .and_then(|s| s.parse::<f32>().ok())
        .ok_or("Missing or invalid baseline in [common]")?;
    let top = common
        .get("top")
        .and_then(|opt_s| opt_s.as_deref())
        .and_then(|s| s.parse::<f32>().ok())
        .ok_or("Missing or invalid top in [common]")?;
    let line_spacing = common
        .get("linespacing")
        .and_then(|opt_s| opt_s.as_deref())
        .and_then(|s| s.parse::<f32>().ok())
        .ok_or("Missing or invalid linespacing in [common]")?;
    let draw_extra_pixels_left = common
        .get("drawextrapixelsleft")
        .and_then(|opt_s| opt_s.as_deref())
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.0);
    let draw_extra_pixels_right = common
        .get("drawextrapixelsright")
        .and_then(|opt_s| opt_s.as_deref())
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.0);
    let advance_extra_pixels = common
        .get("advanceextrapixels")
        .and_then(|opt_s| opt_s.as_deref())
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.0);

    info!(
        "Parsed Common Metrics: Baseline={}, Top={}, LineSpacing={}",
        baseline, top, line_spacing
    ); // Log point 2

    // --- 3. Load Texture ---
    let texture = load_texture(base, texture_path)?;
    let tex_width = texture.width as f32;
    let tex_height = texture.height as f32;
    info!("Font texture loaded: {}x{}", tex_width, tex_height); // Log point 3

    // --- 4. Determine Grid Size & Get Line Keys ---
    let main_section = map.get("main").ok_or("Missing [main] section in INI")?;
    let mut max_row: u32 = 0;
    let mut max_col: u32 = 0;

    // --- ROBUST LINE KEY PARSING ---
    let mut line_keys: Vec<usize> = Vec::new();
    info!("--- Parsing Line Keys ---");
    for key in main_section.keys() {
        let lower_key = key.to_lowercase();
        if lower_key.starts_with("line") {
            if let Some(num_part) = lower_key.splitn(2, 'e').nth(1) {
                if let Ok(num) = num_part.trim().parse::<usize>() {
                    info!("Found valid line key: '{}' -> Parsed number: {}", key, num);
                    line_keys.push(num);
                } else {
                    warn!(
                        "Could not parse number from line key: '{}' (part: '{}')",
                        key, num_part
                    );
                }
            } else {
                warn!("Could not extract number part from line key: '{}'", key);
            }
        }
    }
    // --- END ROBUST PARSING ---

    if line_keys.is_empty() {
        return Err("No valid 'Line X' entries found in [main] section".into());
    }
    line_keys.sort_unstable();
    info!("Found and sorted line keys: {:?}", line_keys);

    // Loop just to find max row/col using the *parsed* line_keys
    info!("--- Determining Grid Size (using parsed keys) ---");
    for &row_idx_usize in &line_keys {
        let original_key = main_section
            .keys()
            .find(|k| {
                k.to_lowercase()
                    .splitn(2, 'e')
                    .nth(1)
                    .and_then(|num_part| num_part.trim().parse::<usize>().ok())
                    == Some(row_idx_usize)
            })
            .ok_or_else(|| {
                format!(
                    "Internal error: Could not find original key for parsed line number {}",
                    row_idx_usize
                )
            })?;

        if let Some(line_str) = main_section.get(original_key).and_then(|opt| opt.as_ref()) {
            max_row = max_row.max(row_idx_usize as u32);
            let current_max_col = line_str.chars().count().saturating_sub(1) as u32;
            max_col = max_col.max(current_max_col);
            // info!("Checked line '{}': max_row={}, max_col={}", original_key, max_row, max_col); // Verbose
        } else {
            warn!("Could not read value for original key: {}", original_key);
        }
    }
    let num_rows = max_row + 1;
    let num_cols = max_col + 1;
    info!(
        "Determined font grid size: {} rows, {} cols",
        num_rows, num_cols
    ); // Log point 4

    // Calculate cell size
    let cell_width = (tex_width / num_cols as f32).floor();
    let cell_height = (tex_height / num_rows as f32).floor();
    info!("Calculated cell size: {}x{}", cell_width, cell_height); // Log point 5

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

    // --- Parse Width Map ---
    let mut width_map: HashMap<u32, f32> = HashMap::new();
    if let Some(root_section) = map.get("") {
        for (key, value_opt) in root_section {
            if let Ok(char_code_idx) = key.parse::<u32>() {
                if let Some(width_str) = value_opt {
                    if let Ok(width) = width_str.parse::<f32>() {
                        width_map.insert(char_code_idx, width);
                    } else {
                        warn!("Invalid width value for root key {}: {}", key, width_str);
                    }
                }
            }
        }
    }
    info!("--- Width Map Contents ---"); // Log point 6
    dbg!(&width_map);
    info!("--- End Width Map ---");

    // --- 5. Build Glyph Map ---
    let mut glyphs: HashMap<char, GlyphInfo> = HashMap::new();
    info!("--- Building Glyph Map ---"); // Log point 7
    for &row_idx_usize in &line_keys {
        let row_idx = row_idx_usize as u32;
        let original_key = main_section
            .keys()
            .find(|k| {
                k.to_lowercase()
                    .splitn(2, 'e')
                    .nth(1)
                    .and_then(|num_part| num_part.trim().parse::<usize>().ok())
                    == Some(row_idx_usize)
            })
            .ok_or_else(|| {
                format!(
                    "Internal error: Could not find original key for parsed line number {}",
                    row_idx_usize
                )
            })?; // Need original key again for lookup

        if let Some(line_str) = main_section.get(original_key).and_then(|opt| opt.as_ref()) {
            info!(
                "Processing Key: {}, Row Idx: {}, Chars: {}",
                original_key,
                row_idx,
                line_str.len()
            ); // Log point 8

            for (col_idx, char_code) in line_str.chars().enumerate() {
                if (char_code as u32) == 0 || char_code == '\u{200b}' {
                    continue;
                }

                let grid_pos = (row_idx, col_idx as u32);
                let char_unicode_idx = char_code as u32;

                let width_pixels = match width_map.get(&char_unicode_idx) {
                    Some(&w) => w,
                    None => {
                        warn!(
                            "Missing width for char '{}' (unicode {}) in INI. Using cell width {:.1}.",
                            char_code, char_unicode_idx, metrics.cell_width
                        );
                        48.0 // Fallback width
                    }
                };

                let u0 = grid_pos.1 as f32 * metrics.cell_width / tex_width;
                let v0 = grid_pos.0 as f32 * metrics.cell_height / tex_height;
                let u_width = metrics.cell_width / tex_width;
                let v_height = metrics.cell_height / tex_height;

                let advance = width_pixels + metrics.advance_extra_pixels;
                let bearing_x = metrics.draw_extra_pixels_left;
                let bearing_y = metrics.baseline - metrics.top;

                // Ensure all fields are assigned
                let glyph_info = GlyphInfo {
                    uv_offset: [u0, v0],
                    uv_scale: [u_width, v_height],
                    advance,
                    bearing_x,
                    bearing_y,
                    width_pixels,
                };

                info!(
                    "  -> Inserting char: '{}' (unicode: {}, grid_pos: {:?}, width: {}, advance: {})",
                    char_code, char_unicode_idx, grid_pos, width_pixels, advance
                ); // Log point 9
                glyphs.insert(char_code, glyph_info);
            }
        }
    }
    info!("--- Finished Building Glyph Map ---"); // Log point 10

    info!("--- Final Glyph Map Size ---"); // Log point 11
    let final_len_load = glyphs.len();
    dbg!(final_len_load);
    info!("--- End Final Glyph Map Size ---");

    // Debugging checks
    if !glyphs.contains_key(&'?') {
        warn!("CHECK: Fallback character '?' was NOT successfully added!");
    } else {
        info!("CHECK: Fallback character '?' was added.");
    }
    if !glyphs.contains_key(&'P') {
        warn!("CHECK: Character 'P' was NOT successfully added!");
    } else {
        info!("CHECK: Character 'P' was added.");
    }
    if !glyphs.contains_key(&' ') {
        warn!("CHECK: Space character ' ' was NOT successfully added!");
    } else {
        info!("CHECK: Space character ' ' was added.");
    }
    // --- End Debugging Checks ---

    // Calculate line height and space width
    let line_height = metrics.line_spacing;
    let space_width = glyphs.get(&' ').map_or_else(
        || {
            warn!("Calculating space_width: Space char ' ' not found! Using fallback.");
            glyphs.get(&'a').map_or(8.0, |g| g.advance)
        },
        |g| g.advance,
    );
    info!(
        "Calculated line_height: {}, space_width: {}",
        line_height, space_width
    ); // Log point 12

    // --- FINAL CHECKS MOVED HERE ---
    info!("--- Final Glyph Map Size (Before Return) ---");
    let final_len_before_return = glyphs.len(); // Get length again
    dbg!(final_len_before_return);
    if !glyphs.contains_key(&'?') {
        warn!("CHECK (Before Return): Fallback '?' was NOT successfully added!");
    } else {
        info!("CHECK (Before Return): Fallback '?' was added.");
    }
    if !glyphs.contains_key(&'P') {
        warn!("CHECK (Before Return): Character 'P' was NOT successfully added!");
    } else {
        info!("CHECK (Before Return): Character 'P' was added.");
    }
    if !glyphs.contains_key(&' ') {
        warn!("CHECK (Before Return): Space character ' ' was NOT successfully added!");
    } else {
        info!("CHECK (Before Return): Space character ' ' was added.");
    }
    // --- End Final Checks ---

    info!(
        "Font loading complete. {} glyphs mapped.",
        final_len_before_return
    ); // Log point 13

    Ok(Font {
        metrics,
        glyphs, // Return the potentially populated map
        texture,
        line_height,
        space_width,
    })
}

// --- Text Drawing Function ---
pub fn draw_text(
    device: &Device,
    cmd_buf: vk::CommandBuffer,
    pipeline_layout: vk::PipelineLayout,
    font: &Font, // Pass borrowed font
    text: &str,
    mut x: f32, // Make mutable for cursor updates
    mut y: f32, // Start Y (baseline of the first line)
    color: [f32; 4],
    index_count: u32, // Index count for the quad
) {
    // --- Log at start of draw_text ---
    //info!(
    //    "DRAW_TEXT START: Font Addr: {:p}, Glyphs len: {}, Text: '{}'",
    //    font,
    //    font.glyphs.len(),
    //    text
    //);

    if !font.glyphs.contains_key(&'?') {
        error!(
            "DRAW_TEXT CHECK: Fallback '?' MISSING from font.glyphs at call time! Map size: {}",
            font.glyphs.len()
        );
    }
    if !font.glyphs.contains_key(&'P') {
        error!(
            "DRAW_TEXT CHECK: Character 'P' MISSING from font.glyphs at call time! Map size: {}",
            font.glyphs.len()
        );
    }
    // --- End Checks ---

    let start_x = x;
    let quad_width = font.metrics.cell_width;
    let quad_height = font.metrics.cell_height;

    for char_code in text.chars() {
        match char_code {
            '\n' => {
                x = start_x;
                y += font.line_height;
            }
            ' ' => {
                x += font.space_width;
            }
            _ => {
                // Use font reference directly
                let glyph_lookup = font.glyphs.get(&char_code);

                let glyph = glyph_lookup.or_else(|| {
                    warn!(
                        "Character '{}' (unicode {}) not found in font map (size {}), using '?'.",
                        char_code,
                        char_code as u32,
                        font.glyphs.len()
                    );
                    font.glyphs.get(&'?') // Try getting '?'
                });

                if let Some(glyph_info) = glyph {
                    let quad_x = x + glyph_info.bearing_x;
                    let quad_y = y - glyph_info.bearing_y;
                    let model_matrix = Matrix4::from_translation(Vector3::new(quad_x, quad_y, 0.0))
                        * Matrix4::from_nonuniform_scale(quad_width, quad_height, 1.0);

                    // Ensure all fields are assigned
                    let push_data = PushConstantData {
                        model: model_matrix,
                        color,
                        uv_offset: glyph_info.uv_offset,
                        uv_scale: glyph_info.uv_scale,
                    };
                    // dbg!(char_code, &push_data); // Uncomment if needed for extreme debugging

                    unsafe {
                        let push_data_bytes = std::slice::from_raw_parts(
                            &push_data as *const _ as *const u8,
                            mem::size_of::<PushConstantData>(),
                        );
                        device.cmd_push_constants(
                            cmd_buf,
                            pipeline_layout,
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                            0,
                            push_data_bytes,
                        );
                        device.cmd_draw_indexed(cmd_buf, index_count, 1, 0, 0, 0);
                    }
                    x += glyph_info.advance;
                } else {
                    warn!(
                        "Fallback character '?' also not found in font map (size {}). Skipping char '{}'.",
                        font.glyphs.len(), char_code
                    );
                    x += font.space_width; // Advance by space width as a last resort
                }
            }
        }
    }
}
