use crate::core::gfx::BlendMode;

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum Background {
    Color([f32; 4]),
    Texture(&'static str),
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum SizeSpec {
    Px(f32),
    Fill,
}

/// A sprite can be sourced from either a texture or a solid color.
/// For `Solid`, the final color is `tint` (no sampling).
#[derive(Clone, Copy, Debug)]
pub enum SpriteSource {
    Texture(&'static str),
    Solid,
}

#[derive(Clone, Debug)]
pub enum Actor {
    /// Unified Sprite
    Sprite {
        align: [f32; 2],
        offset: [f32; 2],
        size: [SizeSpec; 2],
        source: SpriteSource,
        tint: [f32; 4],
        z: i16,
        cell: Option<(u32, u32)>,
        grid: Option<(u32, u32)>,
        uv_rect: Option<[f32; 4]>,   // [u0,v0,u1,v1] top-left origin
        visible: bool,
        flip_x: bool,
        flip_y: bool,
        cropleft: f32,
        cropright: f32,
        croptop: f32,
        cropbottom: f32,
        fadeleft: f32,
        faderight: f32,
        fadetop: f32,
        fadebottom: f32,
        blend: BlendMode,
        rot_z_deg: f32,
        texcoordvelocity: Option<[f32; 2]>,
        animate: bool,       // if true, advance states by time
        state_delay: f32,    // seconds per state when animating (uniform)
    },

    /// Text actor (BitmapText-like)
    Text {
        align: [f32; 2],         // halign/valign pivot inside line box
        offset: [f32; 2],        // parent top-left space
        px: f32,                 // base pixel height (before zoom)
        color: [f32; 4],
        font: &'static str,
        content: String,
        align_text: TextAlign,   // talign: left/center/right
        z: i16,
        // StepMania zoom semantics (scale factors)
        scale: [f32; 2],
        // Optional “fit” targets (preserve aspect by scaling)
        fit_width: Option<f32>,
        fit_height: Option<f32>,
        // NEW: match SM — text honors blend mode too
        blend: BlendMode,
    },

    /// Frame/group box
    Frame {
        align: [f32; 2],
        offset: [f32; 2],
        size: [SizeSpec; 2],
        children: Vec<Actor>,
        background: Option<Background>,
        z: i16,
    },
}
