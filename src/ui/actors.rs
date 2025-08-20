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
pub enum Anchor {
    TopLeft, TopCenter, TopRight,
    CenterLeft, Center, CenterRight,
    BottomLeft, BottomCenter, BottomRight,
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
    /// - `source`: Texture(..) or Solid
    /// - `tint`: multiplied in shader for textures; for Solid it's the final RGBA
    /// - `cell`: optional (col,row) index into a grid atlas
    /// - `grid`: optional (cols,rows) to declare atlas grid explicitly (overrides filename parsing)
    /// - `uv_rect`: optional normalized [u0, v0, u1, v1] (top-left origin). Highest priority when set.
    /// - `visible`: if false, the sprite is culled during layout
    /// - `flip_x` / `flip_y`: mirror the subrect horizontally/vertically
    /// - per-side cropping (fractions in [0,1])
    /// - `blend`: Alpha/Add/Multiply
    Sprite {
        anchor: Anchor,
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

    Text {
        anchor: Anchor,
        offset: [f32; 2],
        px: f32,
        color: [f32; 4],
        font: &'static str,
        content: String,
        align: TextAlign,
        z: i16,
    },

    Frame {
        anchor: Anchor,
        offset: [f32; 2],
        size: [SizeSpec; 2],
        children: Vec<Actor>,
        background: Option<Background>,
        /// Multiplies all children’s RGBA (group diffuse). Default [1,1,1,1].
        mul_color: [f32; 4],
        /// Base layer for this frame (applies to its background and children).
        z: i16,
    },
}