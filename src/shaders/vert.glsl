#version 450

layout(location = 0) out vec2 v_TexCoord;

const vec2 positions[6] = vec2[6](
    vec2(-1.0, +1.0),
    vec2(1.0, 1.0),
    vec2(1.0, -1.0),

    vec2(-1.0, +1.0),
    vec2(-1.0, -1.0),
    vec2(1.0, -1.0)
);

void main() {
    v_TexCoord = positions[gl_VertexIndex] * 0.5 + 0.5;
    gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
}