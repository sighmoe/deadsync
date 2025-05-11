#version 450

layout(location = 0) in vec2 inPosition;   // Quad vertex position (e.g., -0.5 to 0.5)
layout(location = 1) in vec2 inTexCoord;   // Quad texture coordinate (0.0 to 1.0)

layout(location = 0) out vec2 fragTexCoord; // UVs for the MSDF atlas
layout(location = 1) out vec4 fragVertexColor; // Pass through tint color

layout(set = 0, binding = 0) uniform UniformBufferObject {
    mat4 projection;
} ubo;

layout(push_constant) uniform PushConstants {
    mat4 model;      // Model matrix for this glyph quad
    vec4 color;      // Tint color for the glyph
    vec2 uvOffset;   // Top-left UV in atlas (glyphInfo.u0, glyphInfo.v0)
    vec2 uvScale;    // UV dimensions in atlas (glyphInfo.u1-u0, glyphInfo.v1-v0)
    float pxRange;   // Pixel range used for MSDF generation (passed from FontMetrics)
    // float screenPxRange; // Optional: For advanced screen-space anti-aliasing
} pushConsts;

void main() {
    gl_Position = ubo.projection * pushConsts.model * vec4(inPosition, 0.0, 1.0);
    
    // Transform quad's local UVs (0-1) to atlas UVs for this specific glyph
    fragTexCoord = inTexCoord * pushConsts.uvScale + pushConsts.uvOffset;
    fragVertexColor = pushConsts.color;
}