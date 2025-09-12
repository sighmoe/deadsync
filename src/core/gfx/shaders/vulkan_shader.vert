#version 450
layout(location=0) in vec2 in_position;
layout(location=1) in vec2 in_uv;

layout(location=2) in vec2 i_center;
layout(location=3) in vec2 i_size;
layout(location=4) in vec2 i_rot_sin_cos; // (sin, cos)
layout(location=5) in vec4 i_tint;
layout(location=6) in vec2 i_uv_scale;
layout(location=7) in vec2 i_uv_offset;
layout(location=8) in vec4 i_edge_fade;

layout(location=0) out vec2 out_uv;
layout(location=1) out vec2 out_quad;
layout(location=2) flat out vec4 out_tint;
layout(location=3) flat out vec4 out_edge_fade;

layout(push_constant) uniform PC {
    mat4 proj;
} pc;

void main() {
    float s = i_rot_sin_cos.x;
    float c = i_rot_sin_cos.y;

    // scale quad to pixel size
    vec2 p = in_position * i_size;

    // rotate around origin, then translate to center
    vec2 pr = vec2(p.x * c - p.y * s, p.x * s + p.y * c) + i_center;

    gl_Position = pc.proj * vec4(pr, 0.0, 1.0);
    out_uv = in_uv * i_uv_scale + i_uv_offset;
    out_quad = in_uv;
    out_tint = i_tint;
    out_edge_fade = i_edge_fade;
}
