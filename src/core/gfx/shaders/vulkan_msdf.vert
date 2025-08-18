#version 450
// Per-vertex
layout(location=0) in vec2 in_position;
layout(location=1) in vec2 in_uv;

// Per-instance
layout(location=2) in vec2 i_center;
layout(location=3) in vec2 i_size;
layout(location=4) in vec2 i_uv_scale;
layout(location=5) in vec2 i_uv_offset;

layout(location=0) out vec2 out_uv;

layout(push_constant) uniform PC {
    mat4 mvp;
    vec4 color;
    float px_range;
} pc;

void main() {
    vec2 world = i_center + in_position * i_size;
    gl_Position = pc.mvp * vec4(world, 0.0, 1.0);
    out_uv = in_uv * i_uv_scale + i_uv_offset;
}
