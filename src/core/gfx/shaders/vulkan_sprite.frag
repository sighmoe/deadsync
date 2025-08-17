#version 450
layout(location=0) in vec2 in_uv;
layout(location=0) out vec4 out_frag_color;

layout(set=0, binding=0) uniform sampler2D tex_sampler;

layout(push_constant) uniform PC {
    mat4 mvp;
    vec4 tint;
    vec2 uv_scale;
    vec2 uv_offset;
} pc;

void main() {
    vec4 tex_color = texture(tex_sampler, in_uv);
    out_frag_color = tex_color * pc.tint;
}