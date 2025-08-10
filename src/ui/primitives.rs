use cgmath::{Matrix4, Vector2, Vector3};
use crate::core::gfx::{ObjectType, ScreenObject};

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

// This function no longer needs to provide vertex or index data.
pub fn to_screen_object(element: &UIElement) -> ScreenObject {
    match element {
        UIElement::Quad(quad) => ScreenObject {
            object_type: ObjectType::SolidColor { color: quad.color },
            transform: Matrix4::from_translation(Vector3::new(quad.center.x, quad.center.y, 0.0))
                * Matrix4::from_nonuniform_scale(quad.size.x, quad.size.y, 1.0),
        },
        UIElement::Sprite(sprite) => ScreenObject {
            object_type: ObjectType::Textured { texture_id: sprite.texture_id },
            transform: Matrix4::from_translation(Vector3::new(sprite.center.x, sprite.center.y, 0.0))
                * Matrix4::from_nonuniform_scale(sprite.size.x, sprite.size.y, 1.0),
        },
    }
}