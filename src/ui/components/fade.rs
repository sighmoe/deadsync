// src/ui/components/fade.rs
use crate::act;
use crate::core::space::globals::*;
use crate::ui::actors::Actor;

const Z_OVERLAY: i16 = 1200;

/// Full-screen black overlay at the top of the stack.
pub fn black(alpha: f32) -> Actor {
    let w = screen_width();
    let h = screen_height();
    act!(quad:
        align(0.0, 0.0):
        xy(0.0, 0.0):
        zoomto(w, h):
        diffuse(0.0, 0.0, 0.0, alpha.clamp(0.0, 1.0)):
        z(Z_OVERLAY)
    )
}

/// (Optional) white overlay if you ever want a white flash.
#[allow(dead_code)]
pub fn white(alpha: f32) -> Actor {
    let w = screen_width();
    let h = screen_height();
    act!(quad:
        align(0.0, 0.0):
        xy(0.0, 0.0):
        zoomto(w, h):
        diffuse(1.0, 1.0, 1.0, alpha.clamp(0.0, 1.0)):
        z(Z_OVERLAY)
    )
}
