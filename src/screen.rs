use cgmath::Matrix4;
use std::borrow::Cow;

pub const QUAD_INDICES: [u16; 6] = [0, 1, 2, 2, 3, 0];

#[derive(Clone)]
pub enum ObjectType {
    SolidColor { color: [f32; 4] },
    Textured { texture_id: &'static str }, // Changed from String
}

#[derive(Clone)]
pub struct Screen {
    pub clear_color: [f32; 4],
    pub objects: Vec<ScreenObject>,
}

#[derive(Clone)]
pub struct ScreenObject {
    pub vertices: Vec<[f32; 4]>,
    pub indices: Cow<'static, [u16]>,
    pub object_type: ObjectType,
    pub transform: Matrix4<f32>,
}