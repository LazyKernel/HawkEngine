use std::sync::Arc;

use nalgebra_glm::Mat4x4;
use specs::Entity;
use vulkano::{command_buffer::{PrimaryAutoCommandBuffer, allocator::StandardCommandBufferAllocator}, pipeline::GraphicsPipeline, render_pass::Framebuffer, buffer::CpuBufferPool, descriptor_set::allocator::StandardDescriptorSetAllocator};

use crate::shaders::vs::ty::VPUniformBufferObject;


pub struct RenderData {
    pub pipeline: Arc<GraphicsPipeline>,
    pub ubo_pool: Arc<CpuBufferPool<VPUniformBufferObject>>,
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
pub struct ProjectionMatrix(pub Mat4x4);

pub struct ActiveCamera(pub Entity);