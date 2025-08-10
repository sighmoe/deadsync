#version 450
layout(location=0) in vec2 in_position;
layout(location=1) in vec2 in_uv;
layout(location=0) out vec2 out_uv;

layout(push_constant) uniform PC {
    mat4 mvp;
    vec2 uv_scale;
    vec2 uv_offset;
    vec4 color;
    float px_range;
} pc;

void main() {
    gl_Position = pc.mvp * vec4(in_position, 0.0, 1.0);
    out_uv = in_uv * pc.uv_scale + pc.uv_offset;
}
