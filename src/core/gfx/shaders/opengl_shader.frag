#version 330 core
in vec2 v_tex_coord;
out vec4 FragColor;

uniform vec4  u_color;
uniform sampler2D u_texture;
uniform bool  u_use_texture;
// NEW:
uniform bool  u_is_msdf;
uniform float u_px_range; // in atlas texels

float median3(vec3 v) { return max(min(v.r,v.g), min(max(v.r,v.g), v.b)); }

void main() {
    if (!u_use_texture) {
        FragColor = u_color;
        return;
    }

    vec4 s = texture(u_texture, v_tex_coord);

    if (!u_is_msdf) {
        FragColor = s; // regular sprite
        return;
    }

    // MSDF decode (linear atlas!)
    float sd = median3(s.rgb) - 0.5;
    // screen-space derivative. Normalize by atlas texel range -> crisper scale invariance.
    float w = fwidth(sd) * (u_px_range * 1.0);
    float alpha = smoothstep(-w, w, sd);
    FragColor = vec4(u_color.rgb, alpha * u_color.a);
}
