use crate::core::gfx::BackendType;
use crate::ui::actors::{Actor, Anchor, TextAlign};

#[inline(always)]
fn backend_label(b: BackendType) -> &'static str {
    match b {
        BackendType::OpenGL => "OpenGL",
        BackendType::Vulkan => "Vulkan",
    }
}

/// Two-line status (FPS + backend), top-right corner, miso font, white text.
pub fn build(backend: BackendType, fps: f32) -> Vec<Actor> {
    const PX: f32 = 12.0;
    const LINE_GAP: f32 = 4.0;
    const MARGIN_X: f32 = -12.0; // inset from right (TopRight anchor)
    const MARGIN_Y: f32 = 12.0;

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
            offset:  [MARGIN_X, MARGIN_Y + PX + LINE_GAP],
            px:      PX,
            color,
            font:    "miso",
            content: backend_label(backend).to_string(),
            align:   TextAlign::Right,
        },
    ]
}
