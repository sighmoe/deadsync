use cgmath::Vector2;
use crate::ui::primitives::{UIElement, Quad};

/// Three colored squares used on the Options screen.
pub fn options_swatches() -> Vec<UIElement> {
    let size = Vector2::new(100.0, 100.0);
    vec![
        UIElement::Quad(Quad { center: Vector2::new(-150.0, 0.0), size, color: [1.0, 0.0, 0.0, 1.0] }),
        UIElement::Quad(Quad { center: Vector2::new(   0.0, 0.0), size, color: [0.0, 1.0, 0.0, 1.0] }),
        UIElement::Quad(Quad { center: Vector2::new( 150.0, 0.0), size, color: [0.0, 0.0, 1.0, 1.0] }),
    ]
}
