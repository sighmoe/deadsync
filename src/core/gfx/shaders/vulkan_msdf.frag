#version 450
layout(location=0) in vec2 out_uv;
layout(location=0) out vec4 out_frag_color;

layout(set=0, binding=0) uniform sampler2D tex_sampler;

layout(push_constant) uniform PC {
    mat4 mvp;
    vec2 uv_scale;
    vec2 uv_offset;
    vec4 color;
    float px_range;
} pc;

float median3(vec3 v){ return max(min(v.r,v.g), min(max(v.r,v.g), v.b)); }

void main() {
    vec4 s = texture(tex_sampler, out_uv);
    float sd = median3(s.rgb) - 0.5;
    float w  = fwidth(sd) * (pc.px_range * 1.0);
    float a  = smoothstep(-w, w, sd);
    out_frag_color = vec4(pc.color.rgb, a * pc.color.a);
}
