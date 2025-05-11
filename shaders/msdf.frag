// shaders/msdf.frag
#version 450

layout(location = 0) in vec2 fragTexCoord;
layout(location = 1) in vec4 fragVertexColor;

layout(location = 0) out vec4 outColor;

layout(set = 0, binding = 1) uniform sampler2D atlasSampler;

layout(push_constant) uniform PushConstants {
    mat4 model;
    vec4 color;
    vec2 uvOffset;
    vec2 uvScale;
    float pxRange; // This is font.metrics.msdf_pixel_range (e.g., 8.0 from your last generation)
} pushConsts;

float median(float r, float g, float b) {
    return max(min(r, g), min(max(r, g), b));
}

void main() {
    vec4 texSample = texture(atlasSampler, fragTexCoord);
    vec3 finalColorComponent;
    float finalAlphaComponent;

    if (pushConsts.pxRange > 0.001) {
    vec3 msdf = texSample.rgb;
    float sigDist = median(msdf.r, msdf.g, msdf.b);

    float atlasTexelsPerScreenPixel = length(fwidth(fragTexCoord * vec2(textureSize(atlasSampler, 0))));
    
    float w = 0.0;
    if (pushConsts.pxRange > 0.0001) { // Your pxRange from atlas generation (e.g., 8.0)
        w = (atlasTexelsPerScreenPixel / pushConsts.pxRange) * 0.5;
    } else { 
        w = length(fwidth(vec2(sigDist))) * 0.5; // Fallback
    }
    
    // THIS CLAMP IS CRUCIAL FOR CONSISTENCY
    // For pxRange = 8, try these ranges:
    // Option A: Slightly more tolerance for blur, good for smaller text too
    // w = clamp(w, 0.02, 0.08); 
    // Option B: Sharper, might be good for larger text, might alias small text
    // w = clamp(w, 0.015, 0.06); 
    // Option C: A common general purpose recommendation for many engines
    w = clamp(w, 1.0/255.0, 0.125 * (pushConsts.pxRange / 4.0) ); // Scale max blur with pxRange
                                                                 // If pxRange = 8, max = 0.25
                                                                 // If pxRange = 4, max = 0.125
                                                                 // This is too wide, leading to blur.

    // Let's go with a more direct experimental clamp based on your pxRange of 8
    // Target w around 0.5 / pxRange when atlasTexelsPerScreenPixel is 1.0
    // 0.5 / 8.0 = 0.0625
    // So min could be around 0.0625 / 4 = 0.015
    // Max could be around 0.0625 * 2 = 0.125
    // Try:
    w = clamp(w, 0.015, 0.075); // Start with this
    // OR
    //w = clamp(w, 0.02, 0.1);
    //w = clamp(w, 0.025, 0.15);


    finalAlphaComponent = smoothstep(0.5 - w, 0.5 + w, sigDist);
    finalAlphaComponent *= fragVertexColor.a; 
    finalColorComponent = fragVertexColor.rgb;

    } else { // Regular Texture Path (should not be hit for MSDF text)
        finalColorComponent = fragVertexColor.rgb * texSample.rgb;
        finalAlphaComponent = texSample.a * fragVertexColor.a;
    }

    if (finalAlphaComponent < 0.001) { // Discard nearly transparent pixels
        discard;
    }

    outColor = vec4(finalColorComponent, finalAlphaComponent);
}