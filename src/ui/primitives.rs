use cgmath::Vector2;

#[derive(Debug, Clone)]
pub enum UIElement {
    Quad(Quad),
    Sprite(Sprite),
    Text(Text),
}

#[derive(Debug, Clone)]
pub struct Quad {
    pub center: Vector2<f32>,
    pub size: Vector2<f32>,
    pub color: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct Sprite {
    pub center: Vector2<f32>,
    pub size: Vector2<f32>,
    pub texture_id: &'static str, // Changed from String
}

#[derive(Debug, Clone)]
pub struct Text {
    pub origin: Vector2<f32>,      // baseline-left origin in your world coords
    pub pixel_height: f32,         // desired font pixel height
    pub color: [f32;4],
    pub font_id: &'static str,     // e.g. "wendy"
    pub content: String,
}