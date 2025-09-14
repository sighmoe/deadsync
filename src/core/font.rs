// FILE: src/core/font.rs

//! StepMania bitmap font parser (Rust port — dependency-light, functional/procedural)
//! - SM-parity defaults for metrics and width handling (fixes tight/overlapping glyphs)
//! - Supports LINE, MAP U+XXXX / "..." (Unicode, ASCII, CP1252, numbers)
//! - SM extra-pixels quirk (+1/+1, left forced even) to avoid stroke clipping
//! - Canonical texture keys (assets-relative, forward slashes) so lookups match
//! - Parses "(res WxH)" from sheet filenames and scales INI-authored metrics like StepMania
//! - Applies inverse draw scale so on-screen size matches StepMania's authored size
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
    pub tex_rect: [f32; 4],   // px: [x0, y0, x1, y1] (texture space)
    pub size: [f32; 2],       // draw units (SM authored units)
    pub offset: [f32; 2],     // draw units: [x_off_from_pen, y_off_from_baseline]
    pub advance: f32,         // draw units: pen advance
}

#[derive(Debug, Clone)]
pub struct Font {
    pub glyph_map: HashMap<char, Glyph>,
    pub default_glyph: Option<Glyph>,
    pub line_spacing: i32, // draw units (from main/default page)
    pub height: i32,       // draw units (baseline - top)
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
    advance_extra_pixels: i32, // SM default is 0
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
    if t.len() >= 2 && t.starts_with('[') && t.ends_with(']') {
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

/// Return the frame grid (cols, rows) from filename, ignoring any "(res WxH)" hints.
/// Strategy: collect all WxH pairs, drop those inside a "(res ...)" span (case-insensitive),
/// then pick the **last** remaining pair (matches common SM naming like "... 16x16.png").
#[inline(always)]
pub fn parse_sheet_dims_from_filename(filename: &str) -> (u32, u32) {
    let s = filename;
    let bytes = s.as_bytes();
    let n = bytes.len();

    // 1) Find spans covered by "(res ...)" to exclude their WxH.
    let lower = s.to_ascii_lowercase();
    let lb = lower.as_bytes();
    let mut res_spans: Vec<(usize, usize)> = Vec::new();
    let mut i = 0usize;
    while i < n {
        if lb[i] == b'(' && i + 4 <= n && &lb[i..i + 4] == b"(res" {
            // find closing ')'
            let mut j = i + 4;
            while j < n && lb[j] != b')' { j += 1; }
            if j < n && lb[j] == b')' {
                res_spans.push((i, j)); // inclusive
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    let in_res = |idx: usize| -> bool {
        for (a, b) in &res_spans {
            if idx >= *a && idx <= *b { return true; }
        }
        false
    };

    // 2) Collect all WxH candidates.
    let mut pairs: Vec<(usize, u32, u32)> = Vec::new(); // (pos, W, H)
    i = 0;
    while i < n {
        if bytes[i] == b'x' || bytes[i] == b'X' {
            // scan left for W
            let mut l = i;
            while l > 0 && bytes[l - 1].is_ascii_digit() { l -= 1; }
            // scan right for H
            let mut r = i + 1;
            while r < n && bytes[r].is_ascii_digit() { r += 1; }
            if l < i && i + 1 < r {
                if let (Ok(ws), Ok(hs)) = (
                    std::str::from_utf8(&bytes[l..i]),
                    std::str::from_utf8(&bytes[i + 1..r]),
                ) {
                    if let (Ok(w), Ok(h)) = (ws.parse::<u32>(), hs.parse::<u32>()) {
                        if w > 0 && h > 0 {
                            pairs.push((l, w, h));
                        }
                    }
                }
            }
        }
        i += 1;
    }

    // 3) Choose the last WxH not inside "(res ...)".
    for (pos, w, h) in pairs.into_iter().rev() {
        if !in_res(pos) { return (w, h); }
    }

    (1, 1)
}

#[inline(always)]
fn is_doubleres_in_name(name: &str) -> bool {
    name.to_ascii_lowercase().contains("doubleres")
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

/// Compute vertical metrics in texture px, then convert height & line_spacing to draw units.
/// Returns:
/// (line_spacing_tex, baseline_tex, top_tex, height_draw, line_spacing_draw, vshift_tex, vshift_draw)
#[inline(always)]
fn compute_vertical_metrics_draw(
    frame_h_i: i32,
    settings: &FontPageSettings,
    dy: f32,
) -> (i32, i32, i32, i32, i32, f32, f32) {
    let frame_h_f = frame_h_i as f32;

    let line_spacing_tex = if settings.line_spacing != -1 {
        settings.line_spacing
    } else {
        frame_h_i
    };

    let baseline_tex = if settings.baseline != -1 {
        settings.baseline
    } else {
        // SM uses int(center + lineSpacing/2); cast truncates toward zero.
        (frame_h_f * 0.5 + line_spacing_tex as f32 * 0.5) as i32
    };

    let top_tex = if settings.top != -1 {
        settings.top
    } else {
        // SM uses int(center - lineSpacing/2)
        (frame_h_f * 0.5 - line_spacing_tex as f32 * 0.5) as i32
    };

    let height_tex = baseline_tex - top_tex;

    // Convert stored font metrics to logical (draw) units
    let height_draw       = round_half_to_even_i32((height_tex as f32)       * dy);
    let line_spacing_draw = round_half_to_even_i32((line_spacing_tex as f32) * dy);

    // vshift (offset.y) stored per-glyph; keep both forms
    let vshift_tex  = -(baseline_tex as f32);
    let vshift_draw = vshift_tex * dy;

    trace!(
        "    VMetrics: tex(line_spacing={}, baseline={}, top={}, height={}) -> draw(height={}, line_spacing={}), vshift_tex={:.1}, vshift_draw={:.3}",
        line_spacing_tex, baseline_tex, top_tex, height_tex,
        height_draw, line_spacing_draw, vshift_tex, vshift_draw
    );

    (
        line_spacing_tex,
        baseline_tex,
        top_tex,
        height_draw,
        line_spacing_draw,
        vshift_tex,
        vshift_draw,
    )
}

/// Round-to-nearest with ties-to-even (banker's rounding), like C's lrint with FE_TONEAREST.
#[inline(always)]
fn round_half_to_even_i32(v: f32) -> i32 {
    if !v.is_finite() { return 0; }
    let floor = v.floor();
    let frac = v - floor;
    if frac < 0.5 {
        floor as i32
    } else if frac > 0.5 {
        (floor + 1.0) as i32
    } else {
        let f = floor as i32;
        if (f & 1) == 0 { f } else { f + 1 }
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

    let ini_map_lower = parse_ini_trimmed_map(&ini_text);
    let raw_line_map = harvest_raw_line_entries_from_text(&ini_text);

    let prefix = ini_path.file_stem().unwrap().to_str().unwrap();
    let mut texture_paths = list_texture_pages(font_dir, prefix)?;
    if texture_paths.is_empty() {
        return Err(format!("No texture pages found for font '{}'", ini_path_str).into());
    }

    let mut required_textures = Vec::new();
    let mut all_glyphs: HashMap<char, Glyph> = HashMap::new();
    let mut default_page_metrics = (0, 0);

    for (page_idx, tex_path) in texture_paths.iter().enumerate() {
        let page_name = get_page_name_from_path(tex_path);
        let tex_dims = image::image_dimensions(tex_path)?;
        let texture_key = assets::canonical_texture_key(tex_path);

        required_textures.push(tex_path.to_path_buf());

        let (num_frames_wide, num_frames_high) = parse_sheet_dims_from_filename(&texture_key);
        let has_doubleres = is_doubleres_in_name(&texture_key);
        let total_frames = (num_frames_wide * num_frames_high) as usize;

        let (base_tex_w, base_tex_h) = parse_base_res_from_filename(&texture_key)
            .unwrap_or((tex_dims.0, tex_dims.1));

        let mut authored_tex_w = base_tex_w;
        let mut authored_tex_h = base_tex_h;
        if has_doubleres {
            authored_tex_w = (authored_tex_w / 2).max(1);
            authored_tex_h = (authored_tex_h / 2).max(1);
        }

        let frame_w_i = (authored_tex_w / num_frames_wide) as i32;
        let frame_h_i = (authored_tex_h / num_frames_high) as i32;

        info!(
            "  Page '{}', Texture: '{}' -> Authored Grid: {}x{} (frame {}x{} px)",
            page_name, texture_key, num_frames_wide, num_frames_high, frame_w_i, frame_h_i
        );

        let mut settings = FontPageSettings::default();
        let mut sections_to_check = vec!["common".to_string(), page_name.clone()];
        if page_name == "main" {
            sections_to_check.push("char widths".to_string());
        }

        for section in &sections_to_check {
            if let Some(map) = ini_map_lower.get(section) {
                let get_int = |k: &str| -> Option<i32> { map.get(k).and_then(|s| s.parse().ok()) };
                let get_f32 = |k: &str| -> Option<f32> { map.get(k).and_then(|s| s.parse().ok()) };

                if let Some(n) = get_int("drawextrapixelsleft")   { settings.draw_extra_pixels_left = n; }
                if let Some(n) = get_int("drawextrapixelsright")  { settings.draw_extra_pixels_right = n; }
                if let Some(n) = get_int("addtoallwidths")        { settings.add_to_all_widths = n; }
                if let Some(n) = get_f32("scaleallwidthsby")      { settings.scale_all_widths_by = n; }
                if let Some(n) = get_int("linespacing")           { settings.line_spacing = n; }
                if let Some(n) = get_int("top")                   { settings.top = n; }
                if let Some(n) = get_int("baseline")              { settings.baseline = n; }
                if let Some(n) = get_int("defaultwidth")          { settings.default_width = n; }
                if let Some(n) = get_int("advanceextrapixels")    { settings.advance_extra_pixels = n; }

                for (key, val) in map {
                    if let Ok(frame_idx) = key.parse::<usize>() {
                        if let Ok(w) = val.parse::<i32>() {
                            settings.glyph_widths.insert(frame_idx, w);
                        }
                    }
                }
            }
        }
        
        trace!(
            "  [{}] settings(authored): draw_extra L={} R={}, add_to_all_widths={}, scale_all_widths_by={:.3}, \
             line_spacing={}, top={}, baseline={}, default_width={}, advance_extra_pixels={}",
            page_name, settings.draw_extra_pixels_left, settings.draw_extra_pixels_right,
            settings.add_to_all_widths, settings.scale_all_widths_by, settings.line_spacing,
            settings.top, settings.baseline, settings.default_width, settings.advance_extra_pixels
        );
        trace!(
            "  [{}] frames: {}x{} (frame_w={} frame_h={}), total_frames={}",
            page_name, num_frames_wide, num_frames_high, frame_w_i, frame_h_i, total_frames
        );

        let line_spacing_authored = if settings.line_spacing != -1 { settings.line_spacing } else { frame_h_i };
        let baseline_authored = if settings.baseline != -1 {
            settings.baseline
        } else {
            (frame_h_i as f32 * 0.5 + line_spacing_authored as f32 * 0.5) as i32
        };
        let top_authored = if settings.top != -1 {
            settings.top
        } else {
            (frame_h_i as f32 * 0.5 - line_spacing_authored as f32 * 0.5) as i32
        };
        let height_authored = baseline_authored - top_authored;
        let vshift_authored = -(baseline_authored as f32);

        if page_idx == 0 || page_name == "main" {
            default_page_metrics = (height_authored, line_spacing_authored);
        }
        
        trace!(
            "    VMetrics(authored): line_spacing={}, baseline={}, top={}, height={}, vshift={:.1}",
            line_spacing_authored, baseline_authored, top_authored, height_authored, vshift_authored
        );

        let mut char_to_frame: HashMap<char, usize> = HashMap::new();
        for section_name in &sections_to_check {
            let sec_lc = section_name.to_string();
            if let Some(map) = ini_map_lower.get(&sec_lc) {
                for (raw_key_lc, val_str) in map {
                    let key_lc = raw_key_lc.as_str();
                    if key_lc.starts_with("line ") {
                        if let Ok(row) = key_lc[5..].trim().parse::<u32>() {
                            if row >= num_frames_high { continue; }
                            let first_frame = row * num_frames_wide;
                            let line_val = raw_line_map.get(&(sec_lc.clone(), row)).map_or(val_str.as_str(), |s| s.as_str());
                            for (i, ch) in line_val.chars().enumerate() {
                                if (i as u32) < num_frames_wide {
                                    char_to_frame.insert(ch, (first_frame as usize) + i);
                                } else { break; }
                            }
                        }
                    } else if key_lc.starts_with("map ") {
                        if let Ok(frame_index) = val_str.parse::<usize>() {
                            let spec = raw_key_lc[4..].trim();
                            if let Some(hex) = spec.strip_prefix("U+").or_else(|| spec.strip_prefix("u+")) {
                                if let Ok(cp) = u32::from_str_radix(hex, 16) {
                                    if let Some(ch) = char::from_u32(cp) { char_to_frame.insert(ch, frame_index); }
                                }
                            } else if spec.starts_with('"') && spec.ends_with('"') && spec.len() >= 2 {
                                for ch in spec[1..spec.len() - 1].chars() { char_to_frame.insert(ch, frame_index); }
                            }
                        }
                    } else if key_lc.starts_with("range ") {
                        if let Ok(first_frame) = val_str.parse::<usize>() {
                            if let Some((codeset, hex)) = parse_range_key(raw_key_lc) {
                                apply_range_mapping(&mut char_to_frame, &codeset, hex, first_frame);
                            }
                        }
                    }
                }
            }
        }
        apply_space_nbsp_symmetry(&mut char_to_frame);
        
        if page_name != "common" && char_to_frame.is_empty() {
            match total_frames {
                128 => apply_range_mapping(&mut char_to_frame, "ascii", None, 0),
                256 => apply_range_mapping(&mut char_to_frame, "cp1252", None, 0),
                15 | 16 => apply_range_mapping(&mut char_to_frame, "numbers", None, 0),
                _ => {},
            }
        }
        
        debug!("Page '{}' mapped {} chars (frames={}).", page_name, char_to_frame.len(), total_frames);

        let mut draw_left = settings.draw_extra_pixels_left + 1;
        let mut draw_right = settings.draw_extra_pixels_right + 1;
        if draw_left % 2 != 0 { draw_left += 1; }

        for i in 0..total_frames {
            let base_w = if let Some(&w) = settings.glyph_widths.get(&i) {
                w
            } else if settings.default_width != -1 {
                settings.default_width
            } else {
                frame_w_i
            };
            let base_w = base_w + settings.add_to_all_widths;
            let base_w = round_half_to_even_i32((base_w as f32) * settings.scale_all_widths_by);
            
            // Per SM, advance is based on the width *before* the odd chop fix.
            let hadvance = base_w + settings.advance_extra_pixels;

            // This is the visual width, which may be modified by the odd chop quirk.
            let mut width_i = base_w;
            let mut chop_i = frame_w_i - width_i;
            if chop_i < 0 { chop_i = 0; }
            if (chop_i & 1) != 0 {
                chop_i -= 1;
                width_i += 1; // The "Odd Chop" quirk!
            }

            let pad_i = (chop_i / 2).max(0);
            let mut extra_left_i  = draw_left.min(pad_i);
            let mut extra_right_i = draw_right.min(pad_i);
            if width_i <= 0 {
                extra_left_i = 0;
                extra_right_i = 0;
            }

            let glyph_size   = [(width_i + extra_left_i + extra_right_i) as f32, frame_h_i as f32];
            let glyph_offset = [-(extra_left_i as f32), vshift_authored];
            let advance      = hadvance as f32;
            
            let actual_frame_w_i = (tex_dims.0 / num_frames_wide) as i32;
            let actual_frame_h_i = (tex_dims.1 / num_frames_high) as i32;
            let col_i = (i as u32 % num_frames_wide) as i32;
            let row_i = (i as u32 / num_frames_wide) as i32;

            let authored_to_actual_ratio = if frame_w_i > 0 {
                actual_frame_w_i as f32 / frame_w_i as f32
            } else { 1.0 };
            
            // This logic now precisely mirrors the C++ code's integer math steps.
            let tex_chop_off_i = (chop_i as f32 * authored_to_actual_ratio).round() as i32;
            let tex_extra_left_i = (extra_left_i as f32 * authored_to_actual_ratio).round() as i32;
            let tex_extra_right_i = (extra_right_i as f32 * authored_to_actual_ratio).round() as i32;
            
            let left_padding = tex_chop_off_i / 2;
            let right_padding = tex_chop_off_i - left_padding;
            
            let frame_left_px = col_i * actual_frame_w_i;
            
            let tex_rect_left = frame_left_px + left_padding - tex_extra_left_i;
            let tex_rect_right = (col_i + 1) * actual_frame_w_i - right_padding + tex_extra_right_i;

            let tex_rect = [
                tex_rect_left as f32,
                (row_i * actual_frame_h_i) as f32,
                tex_rect_right as f32,
                ((row_i + 1) * actual_frame_h_i) as f32,
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
                    // FIX: Log the final modified width_i, not the initial base_w.
                    trace!(
                        "  [{}] GLYPH {} -> frame {} | width_i={} hadv={} chop={} extraL={} extraR={} \
                         size=[{:.3}x{:.3}] offset=[{:.3},{:.3}] advance={:.3} \
                         tex_rect=[{:.1},{:.1},{:.1},{:.1}]",
                        page_name, fmt_char(ch), i, width_i, hadvance, chop_i, extra_left_i, extra_right_i,
                        glyph.size[0], glyph.size[1], glyph.offset[0], glyph.offset[1], glyph.advance,
                        tex_rect[0], tex_rect[1], tex_rect[2], tex_rect[3],
                    );
                    all_glyphs.insert(ch, glyph.clone());
                }
            }

            if page_idx == 0 && i == 0 {
                all_glyphs.entry(FONT_DEFAULT_CHAR).or_insert_with(|| glyph.clone());
            }
        }
    }

    synthesize_space_from_nbsp(&mut all_glyphs);

    let default_glyph = all_glyphs.get(&FONT_DEFAULT_CHAR).cloned();
    let font = Font {
        glyph_map: all_glyphs,
        default_glyph,
        height: default_page_metrics.0,
        line_spacing: default_page_metrics.1,
    };

    if !font.glyph_map.contains_key(&' ') {
        let adv = font.default_glyph.as_ref().map(|g| g.advance).unwrap_or(0.0);
        warn!(
            "Font '{}' is missing SPACE (U+0020). Falling back to default glyph (advance {:.1}).",
            ini_path_str, adv
        );
    } else if let Some(g) = font.glyph_map.get(&' ') {
        trace!(
            "SPACE metrics (draw): advance={:.3} size=[{:.3}x{:.3}] offset=[{:.3},{:.3}]",
            g.advance, g.size[0], g.size[1], g.offset[0], g.offset[1]
        );
        debug!("SPACE mapped: draw advance {:.3} (texture='{}')", g.advance, g.texture_key);
    }

    info!(
        "--- FINISHED Parsing font '{}' with {} glyphs and {} textures. ---\n",
        ini_path_str, font.glyph_map.len(), required_textures.len()
    );

    Ok(FontLoadData { font, required_textures })
}

/* ======================= API ======================= */

/// Functional helper (pure): compute total advance for a line (draw units).
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

/* ======================= LAYOUT HELPERS USED BY UI ======================= */

#[inline(always)]
fn apply_space_nbsp_symmetry(char_to_frame: &mut std::collections::HashMap<char, usize>) {
    // If SPACE exists but NBSP doesn't, map NBSP -> SPACE frame.
    if let Some(&space_idx) = char_to_frame.get(&' ') {
        char_to_frame.entry('\u{00A0}').or_insert(space_idx);
    }
    // If NBSP exists but SPACE doesn't, map SPACE -> NBSP frame. (Wendy relies on this)
    if let Some(&nbsp_idx) = char_to_frame.get(&'\u{00A0}') {
        char_to_frame.entry(' ').or_insert(nbsp_idx);
    }
}

#[inline(always)]
fn synthesize_space_from_nbsp(all_glyphs: &mut std::collections::HashMap<char, Glyph>) {
    if !all_glyphs.contains_key(&' ') {
        if let Some(nbsp) = all_glyphs.get(&'\u{00A0}').cloned() {
            all_glyphs.insert(' ', nbsp);
            debug!("SPACE synthesized from NBSP glyph at font level (SM parity).");
        }
    }
}
