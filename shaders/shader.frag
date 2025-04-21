#version 450 // Use Vulkan GLSL version

layout(location = 0) in vec2 fragTexCoord; // Input UVs from vertex shader

layout(location = 0) out vec4 outColor;

// Combined Image Sampler descriptor
layout(set = 0, binding = 1) uniform sampler2D texSampler; // ADDED

// Push constants (only need color here, but struct must match vertex shader layout)
layout(push_constant) uniform PushConstants {
    mat4 model; // Unused in fragment shader
    vec4 color;
    vec2 uvOffset; // Unused in fragment shader
    vec2 uvScale;  // Unused in fragment shader
} pushConsts;


void main() {
    vec4 texColor = texture(texSampler, fragTexCoord); // Sample the texture

    // Discard fully transparent pixels for cleaner edges on sprites
    if (texColor.a < 0.01) {
        discard;
    }

    // Combine texture color with push constant tint
    outColor = texColor * pushConsts.color; // MODIFIED
}