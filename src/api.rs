use cgmath::{Matrix4, Vector2};

#[derive(Debug, Clone)]
pub enum UIElement {
    Quad(Quad),
    Sprite(Sprite), // New
}

#[derive(Debug, Clone)]
pub struct Quad {
    pub center: Vector2<f32>,
    pub size: Vector2<f32>,
    pub color: [f32; 4],
}

// New struct for textured quads
#[derive(Debug, Clone)]
pub struct Sprite {
    pub center: Vector2<f32>,
    pub size: Vector2<f32>,
    pub texture_id: String,
}

// Update the translation function
pub fn to_screen_object(element: &UIElement) -> crate::screen::ScreenObject {
    match element {
        UIElement::Quad(quad) => {
            let half_size = quad.size / 2.0;
            crate::screen::ScreenObject {
                vertices: create_vertices(half_size),
                indices: vec![0, 1, 2, 2, 3, 0],
                object_type: crate::screen::ObjectType::SolidColor { color: quad.color },
                transform: Matrix4::from_translation(cgmath::Vector3::new(
                    quad.center.x,
                    quad.center.y,
                    0.0,
                )),
            }
        }
        UIElement::Sprite(sprite) => {
            let half_size = sprite.size / 2.0;
            crate::screen::ScreenObject {
                vertices: create_vertices(half_size),
                indices: vec![0, 1, 2, 2, 3, 0],
                object_type: crate::screen::ObjectType::Textured {
                    texture_id: sprite.texture_id.clone(),
                },
                transform: Matrix4::from_translation(cgmath::Vector3::new(
                    sprite.center.x,
                    sprite.center.y,
                    0.0,
                )),
            }
        }
    }
}

// Helper to avoid repetition
fn create_vertices(half_size: Vector2<f32>) -> Vec<[f32; 2]> {
    vec![
        [-half_size.x, -half_size.y], // bottom-left
        [half_size.x, -half_size.y],  // bottom-right
        [half_size.x, half_size.y],   // top-right
        [-half_size.x, half_size.y],  // top-left
    ]
}