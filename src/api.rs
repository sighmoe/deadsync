use crate::screen::QUAD_INDICES;
use cgmath::{Matrix4, Vector2, Vector3};
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
    pub texture_id: &'static str, // Changed from String
}

// A single, static unit quad with UVs.
// We will scale this quad using the transform matrix instead of creating new vertices per object.
const UNIT_QUAD_VERTICES: [[f32; 4]; 4] = [
    [-0.5, -0.5, 0.0, 1.0],
    [ 0.5, -0.5, 1.0, 1.0],
    [ 0.5,  0.5, 1.0, 0.0],
    [-0.5,  0.5, 0.0, 0.0],
];

pub fn to_screen_object(element: &UIElement) -> crate::screen::ScreenObject {
    match element {
        UIElement::Quad(quad) => {
            crate::screen::ScreenObject {
                // All quads now share the same unit vertices.
                vertices: Cow::Borrowed(&UNIT_QUAD_VERTICES),
                indices: Cow::Borrowed(&QUAD_INDICES),
                object_type: crate::screen::ObjectType::SolidColor { color: quad.color },
                // The transform now handles both position and size.
                // Scale is applied first, then translation (T * S).
                transform: Matrix4::from_translation(Vector3::new(quad.center.x, quad.center.y, 0.0))
                    * Matrix4::from_nonuniform_scale(quad.size.x, quad.size.y, 1.0),
            }
        }
        UIElement::Sprite(sprite) => {
            crate::screen::ScreenObject {
                // All sprites also share the same unit vertices.
                vertices: Cow::Borrowed(&UNIT_QUAD_VERTICES),
                indices: Cow::Borrowed(&QUAD_INDICES),
                object_type: crate::screen::ObjectType::Textured {
                    texture_id: sprite.texture_id,
                },
                // The transform now handles both position and size.
                transform: Matrix4::from_translation(Vector3::new(
                    sprite.center.x,
                    sprite.center.y,
                    0.0,
                )) * Matrix4::from_nonuniform_scale(sprite.size.x, sprite.size.y, 1.0),
            }
        }
    }
}