// FILE: src/core/assets.rs
use std::collections::HashMap;
use std::sync::RwLock;
use std::path::{Path, PathBuf};

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
