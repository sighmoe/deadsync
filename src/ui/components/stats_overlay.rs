use crate::core::gfx::BackendType;
use crate::ui::actors::{Actor, Anchor, TextAlign};

#[inline(always)]
fn backend_label(b: BackendType) -> &'static str {
    match b {
        BackendType::OpenGL => "OpenGL",
        BackendType::Vulkan => "Vulkan",
    }
}

/// Three-line stats: FPS, VPF, Backend — top-right, miso, white.
pub fn build(backend: BackendType, fps: f32, vpf: u32) -> Vec<Actor> {
    const PX: f32 = 12.0;
    const GAP: f32 = 4.0;
    const MARGIN_X: f32 = -16.0; // inset from right for TopRight anchor
    const MARGIN_Y: f32 = 16.0;
    let color = [1.0, 1.0, 1.0, 1.0];

    vec![
        Actor::Text {
            anchor:  Anchor::TopRight,
            offset:  [MARGIN_X, MARGIN_Y],
            px:      PX,
            color,
            font:    "miso",
            content: format!("{:.0} FPS", fps.max(0.0)),
            align:   TextAlign::Right,
        },
        Actor::Text {
            anchor:  Anchor::TopRight,
            offset:  [MARGIN_X, MARGIN_Y + PX + GAP],
            px:      PX,
            color,
            font:    "miso",
            content: format!("{} VPF", vpf),
            align:   TextAlign::Right,
        },
        Actor::Text {
            anchor:  Anchor::TopRight,
            offset:  [MARGIN_X, MARGIN_Y + 2.0 * PX + 2.0 * GAP],
            px:      PX,
            color,
            font:    "miso",
            content: backend_label(backend).to_string(),
            align:   TextAlign::Right,
        },
    ]
}
