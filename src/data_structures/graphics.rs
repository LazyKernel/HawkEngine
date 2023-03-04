use bytemuck::{Pod, Zeroable};
use vulkano;

#[repr(C)]
#[derive(Default, Copy, Clone, Zeroable, Pod)]
pub struct Vertex {
    pub position: [f32; 2]
}

vulkano::impl_vertex!(Vertex, position);
