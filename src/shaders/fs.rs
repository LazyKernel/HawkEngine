use vulkano_shaders;

vulkano_shaders::shader! {
    ty: "fragment",
    src: "
#version 450

layout(set = 1, binding = 0) uniform sampler2D tex_sampler;

layout(location = 0) in vec3 frag_color;
layout(location = 1) in vec2 frag_tex_coord;

layout(location = 0) out vec4 f_color;

void main() {
    f_color = texture(tex_sampler, frag_tex_coord) * vec4(frag_color, 1.0);
}
"
}
