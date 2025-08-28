use crate::core::gfx::BackendType;
use crate::ui::actors::Actor;
use crate::act;
use crate::core::space::globals::*;

/// Three-line stats: FPS, VPF, Backend — top-right, miso, white.
pub fn build(backend: BackendType, fps: f32, vpf: u32) -> Vec<Actor> {
    const PX: f32 = 12.0;
    const GAP: f32 = 4.0;
    const MARGIN_X: f32 = -16.0; // inset from right; negative means leftwards from right edge
    const MARGIN_Y: f32 = 16.0;
    let color = [1.0, 1.0, 1.0, 1.0];

    let w = screen_width();

    vec![
        act!(text:
            align(1.0, 0.0):
            xy(w + MARGIN_X, MARGIN_Y):
            zoomtoheight(PX):
            diffuse(color[0], color[1], color[2], color[3]):
            font("miso"):
            settext(format!("{:.0} FPS", fps.max(0.0))):
            horizalign(right):
            z(200)
        ),
        act!(text:
            align(1.0, 0.0):
            xy(w + MARGIN_X, MARGIN_Y + PX + GAP):
            zoomtoheight(PX):
            diffuse(color[0], color[1], color[2], color[3]):
            font("miso"):
            settext(format!("{} VPF", vpf)):
            horizalign(right):
            z(200)
        ),
        act!(text:
            align(1.0, 0.0):
            xy(w + MARGIN_X, MARGIN_Y + 2.0 * PX + 2.0 * GAP):
            zoomtoheight(PX):
            diffuse(color[0], color[1], color[2], color[3]):
            font("miso"):
            settext(backend.to_string()):
            horizalign(right):
            z(200)
        ),
    ]
}
