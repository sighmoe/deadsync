use cgmath::Matrix4;

#[derive(Clone)]
pub enum ObjectType {
    SolidColor { color: [f32; 4] },
    Textured { texture_id: String },
}

#[derive(Clone)]
pub struct Screen {
    pub clear_color: [f32; 4],
    pub objects: Vec<ScreenObject>,
}

#[derive(Clone)]
pub struct ScreenObject {
    pub vertices: Vec<[f32; 4]>, // Updated: [x, y, u, v]
    pub indices: Vec<u16>,
    pub object_type: ObjectType,
    pub transform: Matrix4<f32>,
}