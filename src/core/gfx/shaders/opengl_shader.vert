#version 330 core
layout (location = 0) in vec2 a_pos;
layout (location = 1) in vec2 a_tex_coord;

out vec2 v_tex_coord;

uniform mat4 u_model_view_proj;
// NEW: maps unit [0..1] quad UVs into a subrect of the atlas
uniform vec2 u_uv_scale;
uniform vec2 u_uv_offset;

void main() {
    gl_Position = u_model_view_proj * vec4(a_pos, 0.0, 1.0);
    v_tex_coord = a_tex_coord * u_uv_scale + u_uv_offset;
}
