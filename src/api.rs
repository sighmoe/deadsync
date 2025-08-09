use cgmath::{Matrix4, Vector2, Vector3};

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
pub fn to_screen_object(element: &UIElement) -> crate::screen::ScreenObject {
    match element {
        UIElement::Quad(quad) => {
            crate::screen::ScreenObject {
                object_type: crate::screen::ObjectType::SolidColor { color: quad.color },
                // The transform now handles both position and size.
                // Scale is applied first, then translation (T * S).
                transform: Matrix4::from_translation(Vector3::new(quad.center.x, quad.center.y, 0.0))
                    * Matrix4::from_nonuniform_scale(quad.size.x, quad.size.y, 1.0),
            }
        }
        UIElement::Sprite(sprite) => {
            crate::screen::ScreenObject {
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