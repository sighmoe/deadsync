#version 330 core

layout (location = 0) in vec2 a_pos;

// UV coordinates for the texture
// (0,0) is bottom-left, (1,1) is top-right
// Flipped Y to correct for image orientation
const vec2 uvs[4] = vec2[](
    vec2(0.0, 1.0),
    vec2(1.0, 1.0),
    vec2(1.0, 0.0),
    vec2(0.0, 0.0)
);

out vec2 v_tex_coord;

uniform mat4 u_model_view_proj;

void main() {
    gl_Position = u_model_view_proj * vec4(a_pos.x, a_pos.y, 0.0, 1.0);
    v_tex_coord = uvs[gl_VertexID];
}