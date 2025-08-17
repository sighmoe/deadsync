use cgmath::Matrix4;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendMode {
    Alpha,
    Add,
    Multiply,
}

#[derive(Clone)]
pub enum ObjectType {
    SolidColor { color: [f32; 4] },
    Textured { texture_id: &'static str },
    Sprite {
        texture_id: &'static str,
        tint: [f32; 4],
        uv_scale: [f32; 2],
        uv_offset: [f32; 2],
    },
    MsdfGlyph {
        texture_id: &'static str,
        uv_scale: [f32; 2],
        uv_offset: [f32; 2],
        color: [f32; 4],
        px_range: f32, // distance range in texels from your msdf generator
    },
}

#[derive(Clone)]
pub struct Screen {
    pub clear_color: [f32; 4],
    pub objects: Vec<ScreenObject>,
}

#[derive(Clone)]
pub struct ScreenObject {
    pub object_type: ObjectType,
    pub transform: Matrix4<f32>,
    pub blend: BlendMode,
}
