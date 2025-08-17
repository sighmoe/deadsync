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

#[derive(Clone, Debug)]
pub enum Actor {
    Quad { anchor: Anchor, offset: [f32; 2], size: [SizeSpec; 2], color: [f32; 4] },
    Sprite { anchor: Anchor, offset: [f32; 2], size: [SizeSpec; 2], texture: &'static str },
    SpriteCell {
        anchor: Anchor,
        offset: [f32; 2],
        size: [SizeSpec; 2],
        texture: &'static str,
        tint: [f32; 4],
        cell: (u32, u32),
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
