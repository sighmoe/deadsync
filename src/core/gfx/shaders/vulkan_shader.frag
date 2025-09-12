#version 450
layout(location=0) in  vec2 in_uv;
layout(location=1) in  vec2 in_quad;
layout(location=2) flat in vec4 in_tint;
layout(location=3) flat in vec4 in_edge_fade;

layout(location=0) out vec4 out_frag_color;

layout(set = 0, binding = 0) uniform sampler2D tex_sampler;

float edge_fade_factor(vec2 q, vec4 e) {
    float f = 1.0;
    if (e.x > 0.0) f *= clamp(q.x / e.x, 0.0, 1.0);
    if (e.y > 0.0) f *= clamp((1.0 - q.x) / e.y, 0.0, 1.0);
    if (e.z > 0.0) f *= clamp(q.y / e.z, 0.0, 1.0);
    if (e.w > 0.0) f *= clamp((1.0 - q.y) / e.w, 0.0, 1.0);
    return f;
}

void main() {
    vec4 tex = texture(tex_sampler, in_uv);
    tex.a *= edge_fade_factor(in_quad, in_edge_fade);
    out_frag_color = tex * in_tint;
}
