use cgmath::{Matrix4, Vector2};

// The high-level description of an object to be rendered.
// We'll start with just Quads, but can expand this to Sprites, Text, etc.
#[derive(Debug, Clone)]
pub enum UIElement {
    Quad(Quad),
}

// Describes a simple, colored rectangle.
#[derive(Debug, Clone)]
pub struct Quad {
    pub center: Vector2<f32>,
    pub size: Vector2<f32>,
    pub color: [f32; 4],
}

// A helper function to easily create screen objects from our API.
// This will translate the high-level Quad into low-level vertices and a transform.
pub fn to_screen_object(element: &UIElement) -> crate::screen::ScreenObject {
    match element {
        UIElement::Quad(quad) => {
            let half_size = quad.size / 2.0;
            crate::screen::ScreenObject {
                vertices: vec![
                    [-half_size.x, -half_size.y],
                    [half_size.x, -half_size.y],
                    [half_size.x, half_size.y],
                    [-half_size.x, half_size.y],
                ],
                indices: vec![0, 1, 2, 2, 3, 0],
                color: quad.color,
                transform: Matrix4::from_translation(cgmath::Vector3::new(
                    quad.center.x,
                    quad.center.y,
                    0.0,
                )),
            }
        }
    }
}