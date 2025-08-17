// src/ui/actors.rs

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
    Sprite {
        anchor: Anchor,
        offset: [f32; 2],
        size: [SizeSpec; 2],
        source: SpriteSource,
        tint: [f32; 4],
        cell: Option<(u32, u32)>,
        grid: Option<(u32, u32)>,
        uv_rect: Option<[f32; 4]>,
    },

    Text {
        anchor: Anchor,
        offset: [f32; 2],
        px: f32,
        color: [f32; 4],
        font: &'static str,
        content: String,
        align: TextAlign,
    },

    Frame {
        anchor: Anchor,
        offset: [f32; 2],
        size: [SizeSpec; 2],
        children: Vec<Actor>,
        background: Option<Background>,
    },
}

/// Convenience macro to build a textured Sprite with sensible defaults:
/// Required keys: anchor, offset, size, texture
/// Optional keys: tint, cell, grid, uv_rect
#[macro_export]
macro_rules! sprite {
    (
        anchor: $anchor:expr,
        offset: $offset:expr,
        size: $size:expr,
        texture: $texture:expr
        $(, tint: $tint:expr )?
        $(, cell: $cell:expr )?
        $(, grid: $grid:expr )?
        $(, uv_rect: $uv_rect:expr )?
        $(,)?
    ) => {
        $crate::ui::actors::Actor::Sprite {
            anchor: $anchor,
            offset: $offset,
            size:   $size,
            source: $crate::ui::actors::SpriteSource::Texture($texture),
            tint:    sprite!(@tint $( $tint )?),
            cell:    sprite!(@opt  $( $cell )?),
            grid:    sprite!(@opt  $( $grid )?),
            uv_rect: sprite!(@opt  $( $uv_rect )?),
        }
    };

    (@tint $t:expr) => { $t };
    (@tint) => { [1.0, 1.0, 1.0, 1.0] };

    (@opt $x:expr) => { Some($x) };
    (@opt) => { None };
}

/// Convenience macro to build a **solid color quad** as a Sprite.
/// Required: anchor, offset, size, color
#[macro_export]
macro_rules! quad {
    (
        anchor: $anchor:expr,
        offset: $offset:expr,
        size: $size:expr,
        color: $color:expr
        $(,)?
    ) => {
        $crate::ui::actors::Actor::Sprite {
            anchor: $anchor,
            offset: $offset,
            size:   $size,
            source: $crate::ui::actors::SpriteSource::Solid,
            tint:   $color,
            cell:   None,
            grid:   None,
            uv_rect: None,
        }
    };
}
