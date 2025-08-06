use cgmath::Matrix4;

#[derive(Clone)]
pub struct Screen {
    pub clear_color: [f32; 4],
    pub objects: Vec<ScreenObject>,
}

#[derive(Clone)]
pub struct ScreenObject {
    pub vertices: Vec<[f32; 2]>,
    pub indices: Vec<u16>,
    pub color: [f32; 4],
    pub transform: Matrix4<f32>,
}