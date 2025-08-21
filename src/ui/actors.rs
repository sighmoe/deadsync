use crate::core::gfx::types::BlendMode;

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
    /// Unified Sprite:
    /// - `align`: [halign, valign] in *child* space (0=left/top, 0.5=center, 1=right/bottom). Continuous.
    /// - Parent reference is *the same fraction* inside parent (SM-like align).
    /// - `offset`: (x,y) added at that parent reference point, before pivot offset.
    /// - `size`: [Px(..) or Fill] in parent “SM px”.
    /// - `source`: texture or solid
    /// - `cell`/`grid`/`uv_rect` as before
    /// - cropping in fractions, flip flags, blend
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
        blend: BlendMode,
    },

    /// Text actor:
    /// - `align`: [halign, valign] applied to the text *line box* (top-left space).
    /// - Horizontal text alignment (`talign`) still controls glyph layout (left/center/right).
    Text {
        align: [f32; 2],
        offset: [f32; 2],
        px: f32,
        color: [f32; 4],
        font: &'static str,
        content: String,
        align_text: TextAlign, // renamed local var in layout; external API name kept
        z: i16,
    },

    /// Frame/group box in parent top-left space.
    /// - `align`: [halign, valign] continuous
    /// - `size`: [Px(..) or Fill]
    /// - `background`: optional color/texture quad filling this frame
    /// - `children`: laid out within this rect using the same top-left coordinate space
    Frame {
        align: [f32; 2],
        offset: [f32; 2],
        size: [SizeSpec; 2],
        children: Vec<Actor>,
        background: Option<Background>,
        z: i16,
    },
}
