#version 450
layout(location=0) in vec2 out_uv;
layout(location=0) out vec4 out_frag_color;

layout(set=0, binding=0) uniform sampler2D tex_sampler;

layout(push_constant) uniform PC {
    mat4 mvp;
    vec2 uv_scale;
    vec2 uv_offset;
    vec4 color;
    float px_range; // distanceRange from atlas JSON
} pc;

float median3(vec3 v){ return max(min(v.r,v.g), min(max(v.r,v.g), v.b)); }

void main() {
    vec3 msdf = texture(tex_sampler, out_uv).rgb;
    float sd  = median3(msdf); // 0.5 is the edge in MSDF

    // atlas texels per screen pixel
    vec2 texSize = vec2(textureSize(tex_sampler, 0));
    float texelsPerScreenPx = length(fwidth(out_uv * texSize));

    // smoothing width scales INVERSELY with px_range
    float w = 0.5 * texelsPerScreenPx / max(pc.px_range, 1e-6);

    float a = smoothstep(0.5 - w, 0.5 + w, sd);
    out_frag_color = vec4(pc.color.rgb, a * pc.color.a);
}
