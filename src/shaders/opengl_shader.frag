#version 330 core
out vec4 FragColor;
uniform vec4 u_color; // New uniform

void main()
{
    FragColor = u_color;
}