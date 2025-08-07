#version 330 core

in vec2 v_tex_coord;

out vec4 FragColor;

uniform vec4 u_color;
uniform sampler2D u_texture;
uniform bool u_use_texture;

void main()
{
    if (u_use_texture) {
        FragColor = texture(u_texture, v_tex_coord);
    } else {
        FragColor = u_color;
    }
}