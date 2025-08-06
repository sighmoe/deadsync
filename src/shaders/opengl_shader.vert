#version 330 core

layout (location = 0) in vec2 a_pos;

uniform mat4 u_model_view_proj;

void main() {
    gl_Position = u_model_view_proj * vec4(a_pos.x, a_pos.y, 0.0, 1.0);
}