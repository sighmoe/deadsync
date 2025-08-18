#version 330 core
in vec2 v_tex_coord;
out vec4 FragColor;

uniform vec4  u_color;
uniform sampler2D u_texture;
uniform bool  u_is_msdf;
uniform float u_px_range; // distanceRange from atlas JSON (linear atlas!)

float median3(vec3 v){ return max(min(v.r,v.g), min(max(v.r,v.g), v.b)); }

void main() {
    vec4 s = texture(u_texture, v_tex_coord);

    if (!u_is_msdf) {
        FragColor = s * u_color; // solids: s == 1x1 white
        return;
    }

    float sd = median3(s.rgb);
    vec2 texSize = vec2(textureSize(u_texture, 0));
    float texelsPerScreenPx = length(fwidth(v_tex_coord * texSize));
    float w = 0.5 * texelsPerScreenPx / max(u_px_range, 1e-6);
    float a = smoothstep(0.5 - w, 0.5 + w, sd);
    FragColor = vec4(u_color.rgb, a * u_color.a);
}
