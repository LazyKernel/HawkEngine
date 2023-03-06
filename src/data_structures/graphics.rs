use bytemuck::{Pod, Zeroable};
use vulkano;

#[repr(C)]
#[derive(Default, Copy, Clone, Zeroable, Pod)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 3]
}

vulkano::impl_vertex!(Vertex, position, color);
