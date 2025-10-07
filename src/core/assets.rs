use std::collections::HashMap;
use std::sync::RwLock;
use std::path::Path;

#[derive(Clone, Copy, Debug)]
pub struct TexMeta { pub w: u32, pub h: u32 }

static TEX_META: once_cell::sync::Lazy<RwLock<HashMap<String, TexMeta>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(HashMap::new()));

pub fn register_texture_dims(key: &str, w: u32, h: u32) {
    let mut m = TEX_META.write().unwrap();
    m.insert(key.to_string(), TexMeta { w, h });
}

pub fn texture_dims(key: &str) -> Option<TexMeta> {
    TEX_META.read().unwrap().get(key).copied()
}



/// Produce a stable texture key used everywhere in the engine:
/// - Strip the leading "assets" directory if present
/// - Normalize separators to forward slashes so keys are cross-platform
pub fn canonical_texture_key<P: AsRef<Path>>(p: P) -> String {
    let p = p.as_ref();
    let rel = p.strip_prefix(Path::new("assets")).unwrap_or(p);
    rel.to_string_lossy().replace('\\', "/")
}

/// Return the frame grid (cols, rows) from filename, ignoring any "(res WxH)" hints.
/// Strategy: collect all WxH pairs, drop those inside a "(res ...)" span (case-insensitive),
/// then pick the **last** remaining pair (matches common SM naming like "..._16x16.png").
#[inline(always)]
pub fn parse_sprite_sheet_dims(filename: &str) -> (u32, u32) {
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
            while j < n && lb[j] != b')' {
                j += 1;
            }
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
            if idx >= *a && idx <= *b {
                return true;
            }
        }
        false
    };

    // 2) Collect all WxH candidates preceded by an underscore.
    let mut pairs: Vec<(usize, u32, u32)> = Vec::new(); // (pos, W, H)
    i = 0;
    while i < n {
        if (bytes[i] == b'x' || bytes[i] == b'X') && i > 0 && bytes[i-1].is_ascii_digit() {
            // scan left for W
            let mut l = i;
            while l > 0 && bytes[l - 1].is_ascii_digit() {
                l -= 1;
            }
            // scan right for H
            let mut r = i + 1;
            while r < n && bytes[r].is_ascii_digit() {
                r += 1;
            }
            if l < i && i + 1 < r {
                if let (Ok(ws), Ok(hs)) = (std::str::from_utf8(&bytes[l..i]), std::str::from_utf8(&bytes[i+1..r])) {
                    if let (Ok(w), Ok(h)) = (ws.parse::<u32>(), hs.parse::<u32>()) {
                        if w > 0 && h > 0 { pairs.push((l, w, h)); }
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