#version 450
layout(location=0) in vec2 in_position;  // unit quad verts
layout(location=1) in vec2 in_uv;

// Per-instance attributes (binding=1)
layout(location=2) in mat4 in_model;     // model matrix
layout(location=6) in vec4 in_tint;
layout(location=7) in vec2 in_uv_scale;
layout(location=8) in vec2 in_uv_offset;
layout(location=9) in vec4 in_edge_fade;

layout(location=0) out vec2 out_uv;
layout(location=1) out vec2 out_quad;
layout(location=2) flat out vec4 out_tint;
layout(location=3) flat out vec4 out_edge_fade;

layout(push_constant) uniform PC {
    mat4 proj; // projection only
} pc;

void main() {
    vec4 world = in_model * vec4(in_position, 0.0, 1.0);
    gl_Position = pc.proj * world;

    out_uv       = in_uv * in_uv_scale + in_uv_offset;
    out_quad     = in_uv;          // [0..1] quad space for edge fades
    out_tint     = in_tint;
    out_edge_fade= in_edge_fade;
}
