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
/// Optional keys: tint, cell, grid, uv_rect, visible, flip_x, flip_y, crop*, blend
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
        $(, visible: $visible:expr )?
        $(, flip_x: $flipx:expr )?
        $(, flip_y: $flipy:expr )?
        $(, cropleft:  $cl:expr )?
        $(, cropright: $cr:expr )?
        $(, croptop:   $ct:expr )?
        $(, cropbottom:$cb:expr )?
        $(, blend: $blend:expr )?
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
            visible: sprite!(@vis  $( $visible )?),
            flip_x:  sprite!(@bool $( $flipx )?),
            flip_y:  sprite!(@bool $( $flipy )?),
            cropleft:   sprite!(@f $( $cl )?),
            cropright:  sprite!(@f $( $cr )?),
            croptop:    sprite!(@f $( $ct )?),
            cropbottom: sprite!(@f $( $cb )?),
            blend:  sprite!(@blend $( $blend )?),
        }
    };

    (@tint $t:expr) => { $t };
    (@tint) => { [1.0, 1.0, 1.0, 1.0] };

    (@opt $x:expr) => { Some($x) };
    (@opt) => { None };

    (@vis $v:expr) => { $v };
    (@vis) => { true };

    (@bool $b:expr) => { $b };
    (@bool) => { false };

    (@f $v:expr) => { $v };
    (@f) => { 0.0 };

    (@blend $b:expr) => { $b };
    (@blend) => { $crate::core::gfx::types::BlendMode::Alpha };
}

#[macro_export]
macro_rules! quad {
    (
        anchor: $anchor:expr,
        offset: $offset:expr,
        size: $size:expr,
        color: $color:expr
        $(, visible: $visible:expr)?
        $(, flip_x: $flipx:expr)?
        $(, flip_y: $flipy:expr)?
        $(, cropleft:  $cl:expr )?
        $(, cropright: $cr:expr )?
        $(, croptop:   $ct:expr )?
        $(, cropbottom:$cb:expr )?
        $(, blend: $blend:expr )?
        $(,)?
    ) => {
        $crate::ui::actors::Actor::Sprite {
            anchor: $anchor,
            offset: $offset,
            size:   $size,
            source: $crate::ui::actors::SpriteSource::Solid,
            // For solids, `tint` is the fill color
            tint: $color,
            // No texture addressing for solids
            cell: None,
            grid: None,
            uv_rect: None,
            // Optionals + defaults
            visible: quad!(@vis $( $visible )?),
            flip_x:  quad!(@b $( $flipx )?),
            flip_y:  quad!(@b $( $flipy )?),
            cropleft:   quad!(@f $( $cl )?),
            cropright:  quad!(@f $( $cr )?),
            croptop:    quad!(@f $( $ct )?),
            cropbottom: quad!(@f $( $cb )?),
            blend:  quad!(@blend $( $blend )?),
        }
    };

    // helpers
    (@vis $v:expr) => { $v };
    (@vis) => { true };

    (@b $b:expr) => { $b };
    (@b) => { false };

    (@f $v:expr) => { $v };
    (@f) => { 0.0 };

    (@blend $b:expr) => { $b };
    (@blend) => { $crate::core::gfx::types::BlendMode::Alpha };
}
