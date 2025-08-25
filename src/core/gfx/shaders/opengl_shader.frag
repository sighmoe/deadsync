#version 330 core
in vec2 v_tex_coord;
in vec2 v_quad;
out vec4 FragColor;

uniform vec4  u_color;
uniform sampler2D u_texture;
uniform bool  u_is_msdf;
uniform float u_px_range; // distanceRange from atlas JSON (linear atlas!)
uniform vec4  u_edge_fade; // (left, right, top, bottom), quad fractions

float median3(vec3 v){ return max(min(v.r,v.g), min(max(v.r,v.g), v.b)); }

float edge_fade_factor(vec2 q, vec4 e) {
    // q in [0,1]^2 (0=left/top, 1=right/bottom)
    float f = 1.0;
    if (e.x > 0.0) f *= clamp(q.x / e.x, 0.0, 1.0);           // left
    if (e.y > 0.0) f *= clamp((1.0 - q.x) / e.y, 0.0, 1.0);   // right
    if (e.z > 0.0) f *= clamp(q.y / e.z, 0.0, 1.0);           // top
    if (e.w > 0.0) f *= clamp((1.0 - q.y) / e.w, 0.0, 1.0);   // bottom
    return f;
}

void main() {
    vec4 s = texture(u_texture, v_tex_coord);

    if (!u_is_msdf) {
        float f = edge_fade_factor(v_quad, u_edge_fade);
        s.a *= f;
        FragColor = s * u_color; // standard straight-alpha blend
        return;
    }

    // MSDF path (unchanged except optional edge fade on alpha)
    float sd = median3(s.rgb);
    vec2 texSize = vec2(textureSize(u_texture, 0));
    float texelsPerScreenPx = length(fwidth(v_tex_coord * texSize));
    float w = 0.5 * texelsPerScreenPx / max(u_px_range, 1e-6);
    float a = smoothstep(0.5 - w, 0.5 + w, sd);

    float f = edge_fade_factor(v_quad, u_edge_fade);
    FragColor = vec4(u_color.rgb, a * u_color.a * f);
}
