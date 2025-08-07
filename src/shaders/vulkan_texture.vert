#version 450

layout(location = 0) in vec2 in_position;

layout(location = 0) out vec2 out_uv;

layout(push_constant) uniform constants {
    mat4 mvp;
} PushConstants;

// Hardcoded UVs for a quad (flipped Y to match your viewport flip)
const vec2 uvs[4] = vec2[](
    vec2(0.0, 1.0),
    vec2(1.0, 1.0),
    vec2(1.0, 0.0),
    vec2(0.0, 0.0)
);

void main() {
    gl_Position = PushConstants.mvp * vec4(in_position, 0.0, 1.0);
    out_uv = uvs[gl_VertexIndex];
}