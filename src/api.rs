use crate::screen::QUAD_INDICES;
use cgmath::{Matrix4, Vector2};
use std::borrow::Cow;

#[derive(Debug, Clone)]
pub enum UIElement {
    Quad(Quad),
    Sprite(Sprite),
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
    pub texture_id: String,
}

pub fn to_screen_object(element: &UIElement) -> crate::screen::ScreenObject {
    match element {
        UIElement::Quad(quad) => {
            let half_size = quad.size / 2.0;
            crate::screen::ScreenObject {
                vertices: create_vertices(half_size),
                indices: Cow::Borrowed(&QUAD_INDICES), // Use borrowed static slice
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
                indices: Cow::Borrowed(&QUAD_INDICES), // Use borrowed static slice
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

// Updated to include UVs in each vertex
fn create_vertices(half_size: Vector2<f32>) -> Vec<[f32; 4]> {
    vec![
        [-half_size.x, -half_size.y, 0.0, 1.0], // bottom-left
        [half_size.x, -half_size.y, 1.0, 1.0],  // bottom-right
        [half_size.x, half_size.y, 1.0, 0.0],   // top-right
        [-half_size.x, half_size.y, 0.0, 0.0],  // top-left
    ]
}