use cgmath::Matrix4;

#[inline(always)]
pub fn ortho_for_window(width: u32, height: u32) -> Matrix4<f32> {
    let aspect = width as f32 / height as f32;
    let (w, h) = if aspect >= 1.0 {
        (400.0 * aspect, 400.0)
    } else {
        (400.0, 400.0 / aspect)
    };
    cgmath::ortho(-w, w, -h, h, -1.0, 1.0)
}