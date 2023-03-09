use vulkano_shaders;

vulkano_shaders::shader! {
    ty: "vertex",
    types_meta: {
        use bytemuck::{Pod, Zeroable};

        #[derive(Clone, Copy, Zeroable, Pod)]
    },
    src: "
#version 450

layout(binding = 0) uniform UniformBufferObject {
    mat4 model;
    mat4 view;
    mat4 proj;
} ubo;

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec3 color;
layout(location = 3) in vec2 tex_coord;

layout(location = 0) out vec3 frag_color;
layout(location = 1) out vec2 frag_tex_coord;
layout(location = 2) out vec3 v_normal;

void main() {
    mat4 worldview = ubo.view * ubo.model;
    gl_Position = ubo.proj * worldview * vec4(position, 1.0);
    frag_color = color;
    frag_tex_coord = tex_coord;
    v_normal = transpose(inverse(mat3(worldview))) * normal;
}
"
}
