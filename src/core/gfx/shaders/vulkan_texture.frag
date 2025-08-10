#version 450

layout(location = 0) in vec2 in_uv;
layout(location = 0) out vec4 out_frag_color;

// The texture and its sampler are bound via descriptor sets
layout(set = 0, binding = 0) uniform sampler2D tex_sampler;

void main() {
    out_frag_color = texture(tex_sampler, in_uv);
}