#version 450 // Use Vulkan GLSL version

layout(location = 0) in vec2 inPosition;
layout(location = 1) in vec2 inTexCoord; // ADDED

layout(location = 0) out vec2 fragTexCoord; // ADDED

layout(set = 0, binding = 0) uniform UniformBufferObject {
    mat4 projection;
} ubo;

// Push constants now include model matrix, color, AND UV info
layout(push_constant) uniform PushConstants {
    mat4 model;
    vec4 color;
    vec2 uvOffset;
    vec2 uvScale;
} pushConsts;

void main() {
    gl_Position = ubo.projection * pushConsts.model * vec4(inPosition, 0.0, 1.0);
    // Apply offset and scale to input UVs to select the correct atlas frame
    fragTexCoord = inTexCoord * pushConsts.uvScale + pushConsts.uvOffset; // MODIFIED
}