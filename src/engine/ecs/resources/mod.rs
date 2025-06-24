use std::sync::Arc;

use nalgebra::Matrix4;
use specs::Entity;
use vulkano::{buffer::Buffer, command_buffer::{allocator::StandardCommandBufferAllocator, PrimaryAutoCommandBuffer}, descriptor_set::allocator::StandardDescriptorSetAllocator, memory::allocator::StandardMemoryAllocator, pipeline::GraphicsPipeline, render_pass::Framebuffer};
use winit::window::CursorGrabMode;


pub mod network;
pub mod physics;

pub struct RenderData {
    pub pipeline: Arc<GraphicsPipeline>,
    pub pipeline_wireframe: Arc<GraphicsPipeline>,
    pub ubo_pool: Arc<Buffer>,
    pub buffer_allocator: Arc<StandardMemoryAllocator>,
    pub command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
    pub descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
    pub queue_family_index: u32
}

pub struct RenderDataFrameBuffer(pub Arc<Framebuffer>);

#[derive(Default)]
pub struct CommandBuffer {
    pub command_buffer: Option<Arc<PrimaryAutoCommandBuffer>>
}

#[derive(Default)]
pub struct ProjectionMatrix(pub Matrix4<f32>);

pub struct ActiveCamera(pub Entity);

pub struct CursorGrab {
    pub grabbed: bool,
    pub mode: CursorGrabMode
}

impl Default for CursorGrab {
    fn default() -> Self {
        CursorGrab { grabbed: false, mode: CursorGrabMode::None }
    }
}

#[derive(Default)]
pub struct DeltaTime(pub f32);
