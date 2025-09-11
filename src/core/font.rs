// FILE: src/core/font.rs

//! StepMania bitmap font parser (Rust port — dependency-light, functional/procedural)
//! - SM-parity defaults for metrics and width handling (fixes tight/overlapping glyphs)
//! - Supports LINE, MAP U+XXXX / "..." (Unicode, ASCII, CP1252, numbers)
//! - SM extra-pixels quirk (+1/+1, left forced even) to avoid stroke clipping
//! - Canonical texture keys (assets-relative, forward slashes) so lookups match
//! - Parses "(res WxH)" from sheet filenames and scales INI-authored metrics like StepMania
//! - No regex/glob/configparser/once_cell; pure std + image + log
//! - VERBOSE TRACE logging for troubleshooting: enable with RUST_LOG=new_engine::core::font=trace

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use image;
use log::{debug, info, warn, trace};

use crate::core::assets;

const FONT_DEFAULT_CHAR: char = '\u{F8FF}'; // SM default glyph (private use)

/* ======================= TYPES ======================= */

#[derive(Debug, Clone)]
pub struct Glyph {
    pub texture_key: String,
    pub tex_rect: [f32; 4],   // px: [x0, y0, x1, y1]
    pub size: [f32; 2],       // px: [w, h]
    pub offset: [f32; 2],     // px: [x_off_from_pen, y_off_from_baseline]
    pub advance: f32,         // px: pen advance
}

#[derive(Debug, Clone)]
pub struct Font {
    pub glyph_map: HashMap<char, Glyph>,
    pub default_glyph: Option<Glyph>,
    pub line_spacing: i32, // from main/default page
    pub height: i32,       // baseline - top
}

pub struct FontLoadData {
    pub font: Font,
    pub required_textures: Vec<PathBuf>,
}

#[derive(Debug)]
struct FontPageSettings {
    draw_extra_pixels_left: i32,
    draw_extra_pixels_right: i32,
    add_to_all_widths: i32,
    scale_all_widths_by: f32,
    line_spacing: i32,         // -1 = “use frame height”
    top: i32,                  // -1 = “center – line_spacing/2”
    baseline: i32,             // -1 = “center + line_spacing/2”
    default_width: i32,        // -1 = “use frame width”
    advance_extra_pixels: i32, // SM default 1
    glyph_widths: HashMap<usize, i32>,
}

impl Default for FontPageSettings {
    #[inline(always)]
    fn default() -> Self {
        Self {
            draw_extra_pixels_left: 0,
            draw_extra_pixels_right: 0,
            add_to_all_widths: 0,
            scale_all_widths_by: 1.0,
            line_spacing: -1,
            top: -1,
            baseline: -1,
            default_width: -1,
            advance_extra_pixels: 1, // SM default
            glyph_widths: HashMap::new(),
        }
    }
}

/* ======================= SMALL PARSERS (NO REGEX) ======================= */

#[inline(always)]
fn strip_bom(mut s: String) -> String {
    if s.starts_with('\u{FEFF}') { s.drain(..1); }
    s
}

#[inline(always)]
fn is_full_line_comment(s: &str) -> bool {
    let t = s.trim_start();
    t.starts_with(';') || t.starts_with('#') || t.starts_with("//")
}

#[inline(always)]
fn as_lower(s: &str) -> String { s.to_ascii_lowercase() }

/// Parse `[Section]` lines (returns section name) — whitespace tolerant.
#[inline(always)]
fn parse_section_header(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.len() >= 2 && t.as_bytes()[0] == b'[' && t.as_bytes()[t.len().saturating_sub(1)] == b']' {
        let name = &t[1..t.len() - 1];
        Some(name.trim().to_string())
    } else {
        None
    }
}

/// Parse `key=value` (trimmed key & value). Returns (key_lower, value_string).
#[inline(always)]
fn parse_kv_trimmed(raw: &str) -> Option<(String, String)> {
    let mut split = raw.splitn(2, '=');
    let k = split.next()?.trim();
    let v = split.next()?.trim();
    if k.is_empty() { return None; }
    Some((as_lower(k), v.to_string()))
}

/// Parse LINE row with *raw* RHS preserved (no trim). Case-insensitive `line`.
#[inline(always)]
fn parse_line_entry_raw(raw: &str) -> Option<(u32, &str)> {
    let eq = raw.find('=')?;
    let (lhs, rhs) = raw.split_at(eq);
    let rhs = &rhs[1..]; // skip '='
    let lhs = lhs.trim_start();

    // Expect: optional spaces, then "line" (any case), spaces, then digits
    let mut it = lhs.chars().peekable();
    while matches!(it.peek(), Some(c) if c.is_whitespace()) { it.next(); }
    let mut buf = String::new();
    for _ in 0..4 { buf.push(it.next()?); }
    if buf.to_ascii_lowercase() != "line" { return None; }
    while matches!(it.peek(), Some(c) if c.is_whitespace()) { it.next(); }
    let num_str: String = it.collect();
    let row: u32 = num_str.trim().parse().ok()?;
    Some((row, rhs))
}

/// FIRST `WxH` pair scanning left-to-right in a filename (ASCII only).
#[inline(always)]
pub fn parse_sheet_dims_from_filename(filename: &str) -> (u32, u32) {
    let bytes = filename.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;
    while i < len {
        if bytes[i] == b'x' || bytes[i] == b'X' {
            let mut l = i;
            while l > 0 && bytes[l - 1].is_ascii_digit() { l -= 1; }
            let mut r = i + 1;
            while r < len && bytes[r].is_ascii_digit() { r += 1; }
            if l < i && i + 1 < r {
                let w = std::str::from_utf8(&bytes[l..i]).ok()
                    .and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
                let h = std::str::from_utf8(&bytes[i + 1..r]).ok()
                    .and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
                if w > 0 && h > 0 { return (w, h); }
            }
        }
        i += 1;
    }
    (1, 1)
}

/// `[section]->{key->val}` (both section/key lowercased, value trimmed). Only std.
fn parse_ini_trimmed_map(text: &str) -> HashMap<String, HashMap<String, String>> {
    let mut out: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut section = String::from("common");
    for mut line in text.lines().map(|s| s.to_string()) {
        if line.ends_with('\r') { line.pop(); }
        if is_full_line_comment(&line) { continue; }
        if let Some(sec) = parse_section_header(&line) {
            section = as_lower(sec.trim());
            continue;
        }
        if line.trim().is_empty() { continue; }
        if let Some((k, v)) = parse_kv_trimmed(&line) {
            out.entry(section.clone()).or_default().insert(k, v);
        }
    }
    out
}

/// Harvest raw `line N=...` entries, keeping RHS verbatim (no trim).
fn harvest_raw_line_entries_from_text(text: &str) -> HashMap<(String, u32), String> {
    let mut out: HashMap<(String, u32), String> = HashMap::new();
    let mut section = String::from("common");
    for mut line in text.lines().map(|s| s.to_string()) {
        if line.ends_with('\r') { line.pop(); }
        if is_full_line_comment(&line) { continue; }
        if let Some(sec) = parse_section_header(&line) {
            section = as_lower(sec.trim());
            continue;
        }
        if let Some((row, rhs)) = parse_line_entry_raw(&line) {
            out.insert((section.clone(), row), rhs.to_string());
        }
    }
    out
}

/// Page name from filename stem: takes text inside first pair of `[...]`, else "main".
#[inline(always)]
fn get_page_name_from_path(path: &Path) -> String {
    let filename = path.file_stem().unwrap_or_default().to_string_lossy();
    if let (Some(s), Some(e)) = (filename.find('['), filename.find(']')) {
        if s < e { return filename[s + 1..e].to_string(); }
    }
    "main".to_string()
}

/// List PNG textures adjacent to INI where name starts with `prefix` (no glob).
fn list_texture_pages(font_dir: &Path, prefix: &str) -> std::io::Result<Vec<PathBuf>> {
    let mut v = Vec::new();
    for entry in fs::read_dir(font_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() { continue; }
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !name.to_ascii_lowercase().ends_with(".png") { continue; }
        if !name.starts_with(prefix) { continue; }
        if name.contains("-stroke") { continue; }
        v.push(path);
    }
    v.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
    Ok(v)
}

/// Parse `range <codeset> [#start-end]` from a key (key only).
#[inline(always)]
fn parse_range_key(key: &str) -> Option<(String, Option<(u32, u32)>)> {
    let k = key.trim_start();
    if !k.to_ascii_lowercase().starts_with("range ") { return None; }
    let rest = k[6..].trim_start();
    // codeset token ends at whitespace or '#'
    let mut cs_end = rest.len();
    for (i, ch) in rest.char_indices() {
        if ch.is_whitespace() || ch == '#' { cs_end = i; break; }
    }
    if cs_end == 0 { return None; }
    let codeset = &rest[..cs_end];
    let tail = rest[cs_end..].trim_start();
    if tail.is_empty() { return Some((codeset.to_string(), None)); }
    if !tail.starts_with('#') { return Some((codeset.to_string(), None)); }
    let tail = &tail[1..];
    let dash = tail.find('-')?;
    let (a, b) = tail.split_at(dash);
    let b = &b[1..];
    let start = u32::from_str_radix(a.trim(), 16).ok()?;
    let end   = u32::from_str_radix(b.trim(), 16).ok()?;
    if end < start { return None; }
    Some((codeset.to_string(), Some((start, end))))
}

/* ======================= LOG HELPERS ======================= */

#[inline(always)]
fn fmt_char(ch: char) -> String {
    match ch {
        ' '        => "SPACE (U+0020)".to_string(),
        '\u{00A0}' => "NBSP (U+00A0)".to_string(),
        '\n'       => "\\n (U+000A)".to_string(),
        '\r'       => "\\r (U+000D)".to_string(),
        '\t'       => "\\t (U+0009)".to_string(),
        _ if ch.is_control() => format!("U+{:04X}", ch as u32),
        _ => format!("'{}' (U+{:04X})", ch, ch as u32),
    }
}

/* ======================= STEPMania SHEET SCALE HELPERS ======================= */

#[inline(always)]
fn round_i(v: f32) -> i32 { v.round() as i32 }

/// Parse "(res WxH)" from a filename or path (case-insensitive). Returns sheet base res.
#[inline(always)]
fn parse_base_res_from_filename(path_or_name: &str) -> Option<(u32, u32)> {
    let s = path_or_name.to_ascii_lowercase();
    let bytes = s.as_bytes();
    let needle = b"(res";
    let mut i = 0usize;
    while i + needle.len() <= bytes.len() {
        if &bytes[i..i + needle.len()] == needle {
            // skip whitespace
            let mut k = i + needle.len();
            while k < bytes.len() && bytes[k].is_ascii_whitespace() { k += 1; }
            // parse W
            let mut w: u32 = 0;
            let mut h: u32 = 0;
            let mut have_w = false;
            while k < bytes.len() && bytes[k].is_ascii_digit() {
                have_w = true;
                w = w.saturating_mul(10) + (bytes[k] - b'0') as u32;
                k += 1;
            }
            while k < bytes.len() && bytes[k].is_ascii_whitespace() { k += 1; }
            // expect 'x'
            if k >= bytes.len() || bytes[k] != b'x' { i += 1; continue; }
            k += 1;
            while k < bytes.len() && bytes[k].is_ascii_whitespace() { k += 1; }
            // parse H
            let mut have_h = false;
            while k < bytes.len() && bytes[k].is_ascii_digit() {
                have_h = true;
                h = h.saturating_mul(10) + (bytes[k] - b'0') as u32;
                k += 1;
            }
            while k < bytes.len() && bytes[k].is_ascii_whitespace() { k += 1; }
            if have_w && have_h && k < bytes.len() && bytes[k] == b')' && w > 0 && h > 0 {
                return Some((w, h));
            }
        }
        i += 1;
    }
    None
}

/// Compute StepMania sheet scaling factors from real texture size and "(res WxH)".
#[inline(always)]
fn compute_font_sheet_scale(texture_key: &str, tex_w: u32, tex_h: u32) -> (f32, f32) {
    let (cols, rows) = parse_sheet_dims_from_filename(texture_key);
    let cols = cols.max(1);
    let rows = rows.max(1);

    let frame_w = (tex_w / cols) as f32;
    let frame_h = (tex_h / rows) as f32;

    if let Some((base_w, base_h)) = parse_base_res_from_filename(texture_key) {
        let base_cell_w = (base_w as f32) / (cols as f32);
        let base_cell_h = (base_h as f32) / (rows as f32);
        let sx = if base_cell_w > 0.0 { frame_w / base_cell_w } else { 1.0 };
        let sy = if base_cell_h > 0.0 { frame_h / base_cell_h } else { 1.0 };
        (sx, sy)
    } else {
        (1.0, 1.0)
    }
}

/// Scale authored metrics into texture pixels (SM parity), then apply ScaleAllWidthsBy.
#[inline(always)]
fn apply_stepmania_metric_scaling(settings: &mut FontPageSettings, sx: f32, sy: f32) {
    // horizontals to texture px
    if settings.default_width != -1 {
        settings.default_width = round_i(settings.default_width as f32 * sx);
    }
    settings.add_to_all_widths     = round_i(settings.add_to_all_widths as f32 * sx);
    settings.advance_extra_pixels  = round_i(settings.advance_extra_pixels as f32 * sx);
    settings.draw_extra_pixels_left  = round_i(settings.draw_extra_pixels_left as f32 * sx);
    settings.draw_extra_pixels_right = round_i(settings.draw_extra_pixels_right as f32 * sx);
    for w in settings.glyph_widths.values_mut() {
        *w = round_i(*w as f32 * sx);
    }

    // verticals to texture px
    if settings.line_spacing != -1 {
        settings.line_spacing = round_i(settings.line_spacing as f32 * sy);
    }
    if settings.top != -1 {
        settings.top = round_i(settings.top as f32 * sy);
    }
    if settings.baseline != -1 {
        settings.baseline = round_i(settings.baseline as f32 * sy);
    }

    // author width scaling applied after base conversion
    if (settings.scale_all_widths_by - 1.0).abs() > f32::EPSILON {
        let s = settings.scale_all_widths_by;
        if settings.default_width != -1 {
            settings.default_width = round_i(settings.default_width as f32 * s);
        }
        settings.add_to_all_widths    = round_i(settings.add_to_all_widths as f32 * s);
        settings.advance_extra_pixels = round_i(settings.advance_extra_pixels as f32 * s);
        settings.draw_extra_pixels_left  = round_i(settings.draw_extra_pixels_left as f32 * s);
        settings.draw_extra_pixels_right = round_i(settings.draw_extra_pixels_right as f32 * s);
        for w in settings.glyph_widths.values_mut() {
            *w = round_i(*w as f32 * s);
        }
    }
}

/* ======================= RANGE APPLY ======================= */

#[inline(always)]
fn apply_range_mapping(
    map: &mut HashMap<char, usize>,
    codeset: &str,
    hex_range: Option<(u32, u32)>,
    first_frame: usize,
) {
    match codeset.to_ascii_lowercase().as_str() {
        "unicode" => {
            if let Some((start, end)) = hex_range {
                let count = end - start + 1;
                for i in 0..count {
                    if let Some(ch) = char::from_u32(start + i) {
                        map.insert(ch, first_frame + i as usize);
                    }
                }
            } else {
                warn!("range Unicode without #start-end ignored");
            }
        }
        "ascii" => {
            let (start, end) = hex_range.unwrap_or((0, 0x7F));
            let mut ff = first_frame;
            for cp in start..=end {
                if let Some(ch) = char::from_u32(cp) { map.insert(ch, ff); }
                ff += 1;
            }
        }
        "cp1252" => {
            let (start, end) = hex_range.unwrap_or((0, 0xFF));
            let mut ff = first_frame;
            for cp in start..=end {
                if let Some(ch) = char::from_u32(cp) { map.insert(ch, ff); }
                ff += 1;
            }
        }
        "numbers" => {
            let numbers_map: &[char] = &[
                '0','1','2','3','4','5','6','7','8','9',
                '.',':','-','+','/','x','%',' ',
            ];
            let (start, end) = hex_range.unwrap_or((0, (numbers_map.len() as u32) - 1));
            let mut ff = first_frame;
            for idx in start..=end {
                let i = idx as usize;
                if i < numbers_map.len() {
                    map.insert(numbers_map[i], ff);
                    ff += 1;
                }
            }
        }
        other => warn!("Unsupported codeset '{}' in RANGE; skipping.", other),
    }
}

/* ======================= PARSE ======================= */

pub fn parse(ini_path_str: &str) -> Result<FontLoadData, Box<dyn std::error::Error>> {
    let ini_path = Path::new(ini_path_str);
    let font_dir = ini_path.parent().ok_or("Could not find font directory")?;

    let mut ini_text = fs::read_to_string(ini_path_str)?;
    ini_text = strip_bom(ini_text);

    // Case-insensitive section/key map (values trimmed)
    let ini_map_lower = parse_ini_trimmed_map(&ini_text);

    // Raw `line N=...` values with leading spaces preserved
    let raw_line_map = harvest_raw_line_entries_from_text(&ini_text);

    // Gather texture pages (ignore -stroke)
    let prefix = ini_path.file_stem().unwrap().to_str().unwrap();
    let mut texture_paths = list_texture_pages(font_dir, prefix)?;
    if texture_paths.is_empty() {
        return Err(format!("No texture pages found for font '{}'", ini_path_str).into());
    }

    let mut required_textures = Vec::new();
    let mut all_glyphs: HashMap<char, Glyph> = HashMap::new();
    let mut default_page_metrics = (0, 0); // (height, line_spacing)

    for (page_idx, tex_path) in texture_paths.iter().enumerate() {
        let page_name = get_page_name_from_path(tex_path);
        let tex_dims = image::image_dimensions(tex_path)?;
        let texture_key = assets::canonical_texture_key(tex_path);

        required_textures.push(tex_path.to_path_buf());

        let (num_frames_wide, num_frames_high) = parse_sheet_dims_from_filename(&texture_key);
        let total_frames = (num_frames_wide * num_frames_high) as usize;

        // --- SM parity: integer cell size for metrics *and* UVs ---
        let frame_w_i = (tex_dims.0 / num_frames_wide) as i32;
        let frame_h_i = (tex_dims.1 / num_frames_high) as i32;
        let frame_w_f = frame_w_i as f32;
        let frame_h_f = frame_h_i as f32;

        // NEW: compute StepMania per-sheet scale from "(res WxH)"
        let (sx, sy) = compute_font_sheet_scale(&texture_key, tex_dims.0, tex_dims.1);

        info!(
            "  Page '{}', Texture: '{}', Grid: {}x{} (frame {}x{} px, scale {:.3} x {:.3})",
            page_name, texture_key, num_frames_wide, num_frames_high, frame_w_i, frame_h_i, sx, sy
        );

        // ------------ Settings (SM defaults honored) -------------
        let mut settings = FontPageSettings::default();

        let mut sections_to_check = vec!["common".to_string(), page_name.clone()];
        if page_name == "main" {
            sections_to_check.push("char widths".to_string()); // lowercased
        }

        for section in &sections_to_check {
            if let Some(map) = ini_map_lower.get(section) {
                let mut get_int = |k: &str| -> Option<i32> { map.get(k).and_then(|s| s.parse().ok()) };
                let mut get_f32 = |k: &str| -> Option<f32> { map.get(k).and_then(|s| s.parse().ok()) };

                if let Some(n) = get_int("drawextrapixelsleft")   { settings.draw_extra_pixels_left = n; }
                if let Some(n) = get_int("drawextrapixelsright")  { settings.draw_extra_pixels_right = n; }
                if let Some(n) = get_int("addtoallwidths")        { settings.add_to_all_widths = n; }
                if let Some(n) = get_f32("scaleallwidthsby")      { settings.scale_all_widths_by = n; }
                if let Some(n) = get_int("linespacing")           { settings.line_spacing = n; }
                if let Some(n) = get_int("top")                   { settings.top = n; }
                if let Some(n) = get_int("baseline")              { settings.baseline = n; }
                if let Some(n) = get_int("defaultwidth")          { settings.default_width = n; }
                if let Some(n) = get_int("advanceextrapixels")    { settings.advance_extra_pixels = n; }

                // Numeric keys are per-frame width overrides
                for (key, val) in map {
                    if let Ok(frame_idx) = key.parse::<usize>() {
                        if let Ok(w) = val.parse::<i32>() {
                            settings.glyph_widths.insert(frame_idx, w);
                        }
                    }
                }
            }
        }

        // --- NEW: scale all authored metrics to texture pixels (SM parity) ---
        apply_stepmania_metric_scaling(&mut settings, sx, sy);

        // Trace page settings and grid
        trace!(
            "Page '{}' settings(px): draw_extra_pixels L={} R={}, add_to_all_widths={}, scale_all_widths_by={:.3}, \
             line_spacing={}, top={}, baseline={}, default_width={}, advance_extra_pixels={}",
            page_name,
            settings.draw_extra_pixels_left,
            settings.draw_extra_pixels_right,
            settings.add_to_all_widths,
            settings.scale_all_widths_by,
            settings.line_spacing,
            settings.top,
            settings.baseline,
            settings.default_width,
            settings.advance_extra_pixels,
        );
        trace!(
            "Page '{}' frames: {}x{} (frame_w={} frame_h={}), total_frames={}",
            page_name, num_frames_wide, num_frames_high, frame_w_i, frame_h_i, total_frames
        );

        // ------------- Vertical metrics --------------
        let line_spacing = if settings.line_spacing != -1 {
            settings.line_spacing
        } else {
            frame_h_i
        };

        let baseline = if settings.baseline != -1 {
            settings.baseline
        } else {
            (frame_h_f * 0.5 + line_spacing as f32 * 0.5).round() as i32
        };

        let top = if settings.top != -1 {
            settings.top
        } else {
            (frame_h_f * 0.5 - line_spacing as f32 * 0.5).round() as i32
        };

        let height = baseline - top;

        if page_idx == 0 || page_name == "main" {
            default_page_metrics = (height, line_spacing);
        }

        let vshift = -(baseline as f32);

        if line_spacing <= 0 || height <= 0 {
            warn!(
                "Page '{}' suspicious metrics: line_spacing={}, height={}, frame_h={}",
                page_name, line_spacing, height, frame_h_i
            );
        }

        // ------------- Build character -> frame mapping -----------
        let mut char_to_frame: HashMap<char, usize> = HashMap::new();

        for section_name in &sections_to_check {
            let sec_lc = section_name.to_string(); // already lowercased

            if let Some(map) = ini_map_lower.get(&sec_lc) {
                for (raw_key_lc, val_str) in map {
                    let key_lc = raw_key_lc.as_str();

                    // LINE row (prefer RAW, untrimmed value if available)
                    if key_lc.starts_with("line ") {
                        if let Ok(row) = key_lc[5..].trim().parse::<u32>() {
                            if row >= num_frames_high {
                                warn!("LINE {} out of bounds for grid {}x{}", row, num_frames_wide, num_frames_high);
                                continue;
                            }
                            let first_frame = row * num_frames_wide;
                            // Prefer the raw value (keeps leading spaces). Fall back to trimmed.
                            let line_val = if let Some(raw) = raw_line_map.get(&(sec_lc.clone(), row)) {
                                raw.as_str()
                            } else {
                                val_str.as_str()
                            };

                            for (i, ch) in line_val.chars().enumerate() {
                                if (i as u32) < num_frames_wide {
                                    let frame_idx = (first_frame as usize) + i;
                                    char_to_frame.insert(ch, frame_idx);
                                    trace!(
                                        "  [{}] LINE row {} col {} -> frame {} : {}",
                                        page_name, row, i, frame_idx, fmt_char(ch)
                                    );
                                } else {
                                    warn!("Too many chars on LINE {} ({} > cols {})", row, i + 1, num_frames_wide);
                                    break;
                                }
                            }
                        }
                        continue;
                    }

                    // MAP U+XXXX or map "X"
                    if key_lc.starts_with("map ") {
                        if let Ok(frame_index) = val_str.parse::<usize>() {
                            let spec = raw_key_lc[4..].trim(); // after "map "
                            if let Some(hex) = spec.strip_prefix("U+").or_else(|| spec.strip_prefix("u+")) {
                                if let Ok(cp) = u32::from_str_radix(hex, 16) {
                                    if let Some(ch) = char::from_u32(cp) {
                                        char_to_frame.insert(ch, frame_index);
                                        trace!(
                                            "  [{}] MAP {} -> frame {} : {}",
                                            page_name, &spec, frame_index, fmt_char(ch)
                                        );
                                    }
                                }
                            } else if spec.starts_with('"') && spec.ends_with('"') && spec.len() >= 2 {
                                let payload = &spec[1..spec.len() - 1];
                                for ch in payload.chars() {
                                    char_to_frame.insert(ch, frame_index);
                                    trace!(
                                        "  [{}] MAP \"{}\" -> frame {} : {}",
                                        page_name, payload, frame_index, fmt_char(ch)
                                    );
                                }
                            } else {
                                // keep strict behavior; require quoted strings or U+XXXX
                                warn!("Unsupported MAP alias key '{}'", spec);
                            }
                        }
                        continue;
                    }

                    // RANGE codeset
                    if key_lc.starts_with("range ") {
                        if let Ok(first_frame) = val_str.parse::<usize>() {
                            if let Some((codeset, hex)) = parse_range_key(raw_key_lc) {
                                trace!(
                                    "  [{}] RANGE {:?} first_frame={}",
                                    page_name, raw_key_lc, first_frame
                                );
                                apply_range_mapping(&mut char_to_frame, &codeset, hex, first_frame);
                            } else {
                                warn!("Failed to parse RANGE key '{}'", raw_key_lc);
                            }
                        }
                        continue;
                    }
                }
            }
        }

        // If ' ' is mapped and NBSP isn't, map NBSP to the same frame.
        if let Some(&space_idx) = char_to_frame.get(&' ') {
            char_to_frame.entry('\u{00A0}').or_insert(space_idx);
        }

        // Warn for the default page if SPACE is missing
        let is_default_page = page_idx == 0 || page_name.eq_ignore_ascii_case("main");
        if is_default_page && !char_to_frame.contains_key(&' ') {
            warn!(
                "Font page '{}' has no mapping for SPACE (U+0020). \
                 (Check raw LINE values; leading spaces must be preserved.)",
                page_name
            );
        }

        // SM defaults if a non-common page has no explicit mapping
        if page_name != "common" && char_to_frame.is_empty() {
            match total_frames {
                128 => {
                    for (i, cp) in (0u32..=0x7F).enumerate() {
                        if let Some(ch) = char::from_u32(cp) {
                            char_to_frame.insert(ch, i);
                        }
                    }
                    debug!("Page '{}' defaulted to ASCII mapping (128 frames).", page_name);
                }
                256 => {
                    for (i, cp) in (0u32..=0xFF).enumerate() {
                        if let Some(ch) = char::from_u32(cp) {
                            char_to_frame.insert(ch, i);
                        }
                    }
                    debug!("Page '{}' defaulted to CP1252 mapping (256 frames).", page_name);
                }
                15 | 16 => {
                    let digits = "0123456789";
                    for (i, ch) in digits.chars().enumerate() {
                        if i < total_frames {
                            char_to_frame.insert(ch, i);
                        }
                    }
                    debug!("Page '{}' defaulted to simple numbers mapping ({} frames).", page_name, total_frames);
                }
                _ => {
                    debug!("Page '{}' has no explicit mapping; leaving empty.", page_name);
                }
            }
        }

        debug!(
            "Page '{}' mapped {} chars (frames={}).",
            page_name,
            char_to_frame.len(),
            total_frames
        );

        // ------------- GLYPHS (pure integer math like StepMania) -------------
        // SM quirk: +1/+1 extra pixels; left forced even
        let mut draw_left = settings.draw_extra_pixels_left + 1;
        let mut draw_right = settings.draw_extra_pixels_right + 1;
        if draw_left % 2 != 0 { draw_left += 1; }

        for i in 0..total_frames {
            // Base width (int), plus tweaks (already scaled to texture px)
            let mut base_w_px: i32 = if let Some(&w) = settings.glyph_widths.get(&i) {
                w
            } else if settings.default_width != -1 {
                settings.default_width
            } else {
                frame_w_i
            };
            base_w_px += settings.add_to_all_widths;
            base_w_px = ((base_w_px as f32) * settings.scale_all_widths_by).round() as i32;

            // Integer hadvance (SM)
            let hadvance_px: i32 = base_w_px + settings.advance_extra_pixels;
            let advance = hadvance_px as f32;

            // Integer chop and width fixup (odd chop -> widen by 1, make chop even)
            let mut width_i = base_w_px;
            let mut chop_i = frame_w_i - width_i;
            if chop_i < 0 { chop_i = 0; } // don’t allow negative pad
            if (chop_i & 1) != 0 {
                chop_i -= 1;
                width_i += 1;
            }

            // Integer padding capacity and extra pixel clamp
            let pad_i = (chop_i / 2).max(0);
            let extra_left_i  = draw_left.min(pad_i);
            let extra_right_i = draw_right.min(pad_i);

            // Final on-screen quad size (px)
            let glyph_size = [
                (width_i + extra_left_i + extra_right_i) as f32,
                frame_h_i as f32,
            ];
            // Offset from pen (px)
            let glyph_offset = [-(extra_left_i as f32), vshift];

            // Integer texel rect for this frame
            let col = (i as u32 % num_frames_wide) as i32;
            let row = (i as u32 / num_frames_wide) as i32;
            let tex_x_i = col * frame_w_i;
            let tex_y_i = row * frame_h_i;

            // Trim inside the frame by (pad - extra) on each side
            let left_trim  = (pad_i - extra_left_i).max(0);
            let right_trim = (pad_i - extra_right_i).max(0);

            let tex_rect = [
                (tex_x_i + left_trim) as f32,
                (tex_y_i) as f32,
                (tex_x_i + frame_w_i - right_trim) as f32,
                (tex_y_i + frame_h_i) as f32,
            ];

            let glyph = Glyph {
                texture_key: texture_key.clone(),
                tex_rect,
                size: glyph_size,
                offset: glyph_offset,
                advance,
            };

            for (&ch, &frame_idx) in &char_to_frame {
                if frame_idx == i {
                    if advance <= 0.5 {
                        debug!(
                            "Glyph '{}' on page '{}' has very small advance ({:.2})",
                            ch, page_name, advance
                        );
                    }
                    trace!(
                        "  [{}] GLYPH {} -> frame {} | base_w={} hadv={} chop={} extraL={} extraR={} \
                         size=[{:.0}x{:.0}] offset=[{:.0},{:.0}] vshift={:.0} \
                         tex_rect=[{:.0},{:.0},{:.0},{:.0}]",
                        page_name,
                        fmt_char(ch),
                        i,
                        base_w_px,
                        hadvance_px,
                        chop_i,
                        extra_left_i,
                        extra_right_i,
                        glyph_size[0],
                        glyph_size[1],
                        glyph_offset[0],
                        glyph_offset[1],
                        vshift,
                        tex_rect[0],
                        tex_rect[1],
                        tex_rect[2],
                        tex_rect[3],
                    );
                    all_glyphs.insert(ch, glyph.clone());
                }
            }

            // Insert default glyph only from the first page's frame 0 (SM parity: main/default page)
            if page_idx == 0 && i == 0 {
                all_glyphs.entry(FONT_DEFAULT_CHAR).or_insert_with(|| glyph.clone());
            }
        }
    }

    let default_glyph = all_glyphs.get(&FONT_DEFAULT_CHAR).cloned();
    let font = Font {
        glyph_map: all_glyphs,
        default_glyph,
        height: default_page_metrics.0,
        line_spacing: default_page_metrics.1,
    };

    // Whole-font SPACE check + trace summary
    if !font.glyph_map.contains_key(&' ') {
        let adv = font.default_glyph.as_ref().map(|g| g.advance).unwrap_or(0.0);
        warn!(
            "Font '{}' is missing SPACE (U+0020). Falling back to default glyph (advance {:.1}px). \
             Consider adding a SPACE mapping in the INI.",
            ini_path_str, adv
        );
        if adv < 0.5 {
            warn!(
                "Default glyph advance for SPACE is extremely small ({:.2}); words may butt together.",
                adv
            );
        }
    } else if let Some(g) = font.glyph_map.get(&' ') {
        trace!(
            "SPACE metrics: advance={:.1} size=[{:.1}x{:.1}] offset=[{:.1},{:.1}]",
            g.advance, g.size[0], g.size[1], g.offset[0], g.offset[1]
        );
        debug!("SPACE mapped: advance {:.1}px (texture='{}')", g.advance, g.texture_key);
    }

    info!(
        "--- FINISHED Parsing font '{}' with {} glyphs and {} textures. ---\n",
        ini_path_str,
        font.glyph_map.len(),
        required_textures.len()
    );

    Ok(FontLoadData { font, required_textures })
}

/* ======================= API ======================= */

/// Functional helper (pure): compute total advance for a line.
#[inline(always)]
pub fn measure_line_width(font: &Font, text: &str) -> f32 {
    text.chars()
        .map(|c| font.glyph_map.get(&c).or(font.default_glyph.as_ref()).map_or(0.0, |g| g.advance))
        .sum()
}

impl Font {
    /// Wrapper retained for compatibility with existing call sites (e.g. `compose.rs`).
    #[inline(always)]
    pub fn measure_line_width(&self, text: &str) -> f32 {
        measure_line_width(self, text)
    }
}

/* ======================= LAYOUT HELPERS USED BY UI (unchanged) ======================= */

#[inline(always)]
fn line_width_no_overlap_px(font: &Font, text: &str, scale_x: f32) -> i32 {
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
            let need_pen = (last_right - g.offset[0] * scale_x - 0.5).ceil();
            if pen < need_pen { pen = need_pen; }

            let draw_x = (pen + g.offset[0] * scale_x).round();
            let right  = draw_x + g.size[0] * scale_x;
            if right > last_right { last_right = right; }
        }

        pen += g.advance * scale_x;
    }

    last_right.max(pen).round() as i32
}
