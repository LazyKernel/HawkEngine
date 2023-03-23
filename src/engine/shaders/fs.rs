use vulkano_shaders;

vulkano_shaders::shader! {
    ty: "fragment",
    src: "
#version 450

layout(set = 1, binding = 0) uniform sampler2D tex_sampler;

layout(location = 0) in vec3 frag_color;
layout(location = 1) in vec2 frag_tex_coord;
layout(location = 2) in vec3 v_normal;

layout(location = 0) out vec4 f_color;

const vec3 LIGHT = vec3(0.0, 0.0, 1.0);

void main() {
    float brightness = dot(normalize(v_normal), normalize(LIGHT));
    f_color = vec4(texture(tex_sampler, frag_tex_coord).rgb * brightness, 1.0);
}
"
}
