use vulkano_shaders;

vulkano_shaders::shader! {
    ty: "vertex",
    types_meta: {
        use bytemuck::{Pod, Zeroable};

        #[derive(Clone, Copy, Zeroable, Pod)]
    },
    src: "
#version 450

layout(binding = 0) uniform VPUniformBufferObject {
    mat4 view;
    mat4 proj;
} ubo_vp;

layout(push_constant) uniform ModelPushConstants {
    mat4 model;
} pcs_m;

layout(location = 0) in vec3 position;
layout(location = 1) in vec3 color;

layout(location = 0) out vec3 frag_color;

void main() {
    mat4 worldview = ubo_vp.view * pcs_m.model;
    gl_Position = ubo_vp.proj * worldview * vec4(position, 1.0);
    frag_color = color;
}
"
}
