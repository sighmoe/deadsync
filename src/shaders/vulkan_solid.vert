#version 450

layout(location = 0) in vec2 in_position;

layout(location = 0) out vec4 out_color; // Output to fragment shader

layout(push_constant) uniform constants {
    mat4 mvp;
    vec4 color;
} PushConstants;

void main() {
    gl_Position = PushConstants.mvp * vec4(in_position, 0.0, 1.0);
    out_color = PushConstants.color; // Pass color to frag
}