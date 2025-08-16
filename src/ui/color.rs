// src/ui/color.rs
#[inline(always)]
pub fn srgb8_to_linear(c: u8) -> f32 {
    let x = (c as f32) / 255.0;
    if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) }
}

/// Accepts "#rgb", "#rgba", "#rrggbb", "#rrggbbaa" (or without '#').
/// Panics on invalid input; use only with trusted literals.
pub fn rgba_hex(s: &str) -> [f32; 4] {
    #[inline(always)] fn nib(b: u8) -> u8 {
        match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => 10 + (b - b'a'),
            b'A'..=b'F' => 10 + (b - b'A'),
            _ => panic!("invalid hex digit"),
        }
    }
    #[inline(always)] fn byte2(h: u8, l: u8) -> u8 { (nib(h) << 4) | nib(l) }
    #[inline(always)] fn rep(n: u8) -> u8 { (n << 4) | n }

    let bytes = s.as_bytes();
    let off = if bytes.first() == Some(&b'#') { 1 } else { 0 };
    let n = bytes.len() - off;
    let b = &bytes[off..];

    match n {
        3 => { // #rgb
            let r = rep(b[0]); let g = rep(b[1]); let b = rep(b[2]);
            [srgb8_to_linear(r), srgb8_to_linear(g), srgb8_to_linear(b), 1.0]
        }
        4 => { // #rgba
            let r = rep(b[0]); let g = rep(b[1]); let bl = rep(b[2]); let a = rep(b[3]);
            [srgb8_to_linear(r), srgb8_to_linear(g), srgb8_to_linear(bl), (a as f32)/255.0]
        }
        6 => { // #rrggbb
            let r = byte2(b[0], b[1]); let g = byte2(b[2], b[3]); let bl = byte2(b[4], b[5]);
            [srgb8_to_linear(r), srgb8_to_linear(g), srgb8_to_linear(bl), 1.0]
        }
        8 => { // #rrggbbaa
            let r = byte2(b[0], b[1]); let g = byte2(b[2], b[3]);
            let bl = byte2(b[4], b[5]); let a = byte2(b[6], b[7]);
            [srgb8_to_linear(r), srgb8_to_linear(g), srgb8_to_linear(bl), (a as f32)/255.0]
        }
        _ => panic!("hex must be 3/4/6/8 digits"),
    }
}
