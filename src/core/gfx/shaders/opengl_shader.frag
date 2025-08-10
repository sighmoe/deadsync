#version 330 core
in vec2 v_tex_coord;
out vec4 FragColor;

uniform vec4  u_color;
uniform sampler2D u_texture;
uniform bool  u_use_texture;
uniform bool  u_is_msdf;
uniform float u_px_range; // distanceRange from atlas JSON (linear atlas!)

float median3(vec3 v){ return max(min(v.r,v.g), min(max(v.r,v.g), v.b)); }

void main() {
    if (!u_use_texture) {
        FragColor = u_color;
        return;
    }

    vec4 s = texture(u_texture, v_tex_coord);

    // Regular sprites (sRGB texture) — just show the texel
    if (!u_is_msdf) {
        FragColor = s;
        return;
    }

    // MSDF decode in LINEAR space
    float sd = median3(s.rgb);          // 0.5 is the edge

    // How many atlas texels map to one screen pixel
    vec2 texSize = vec2(textureSize(u_texture, 0));
    float texelsPerScreenPx = length(fwidth(v_tex_coord * texSize));

    // Smoothing width scales INVERSELY with px_range (bigger range => crisper)
    float w = 0.5 * texelsPerScreenPx / max(u_px_range, 1e-6);

    // Threshold around 0.5
    float a = smoothstep(0.5 - w, 0.5 + w, sd);
    FragColor = vec4(u_color.rgb, a * u_color.a);
}
