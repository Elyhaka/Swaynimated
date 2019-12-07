#version 450

layout(location = 0) in vec2 v_TexCoord;
layout(location = 0) out vec4 outColor;
layout(set = 0, binding = 0) uniform sampler s_Color;
layout(set = 0, binding = 1) uniform texture2DArray t_Color;
layout(set = 0, binding = 2) uniform Locals {
    uint layer;
};

void main() {
    outColor = texture(
        sampler2DArray(t_Color, s_Color),
        vec3(v_TexCoord, layer)
    );
}
