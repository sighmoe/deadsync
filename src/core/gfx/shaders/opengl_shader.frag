#version 330 core
in vec2 v_tex_coord;
in vec2 v_quad;
out vec4 FragColor;

uniform vec4  u_color;
uniform sampler2D u_texture;
uniform vec4  u_edge_fade; // (left, right, top, bottom), quad fractions

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
    float f = edge_fade_factor(v_quad, u_edge_fade);
    s.a *= f;
    FragColor = s * u_color; // standard straight-alpha blend
}