use cgmath::Matrix4;
use crate::utils::layout::metrics_for_window;

#[inline(always)]
pub fn ortho_for_window(width: u32, height: u32) -> Matrix4<f32> {
    let m = metrics_for_window(width, height);
    cgmath::ortho(m.left, m.right, m.bottom, m.top, -1.0, 1.0)
}
