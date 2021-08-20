#version 100

uniform mat3 projection;
uniform vec4 color;

attribute vec2 position;
attribute vec2 texcoord;

varying vec4 v_color;
varying vec2 v_texcoord;

void main() {
	vec2 test = position;
	gl_Position = vec4(projection * vec3(position, 1.0), 1.0);
	v_color = color;
	v_texcoord = position;
};