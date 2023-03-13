use std::sync::Arc;

use nalgebra_glm::Mat4x4;
use specs::Entity;
use vulkano::{command_buffer::PrimaryAutoCommandBuffer, pipeline::GraphicsPipeline, render_pass::Framebuffer, buffer::CpuBufferPool};

use crate::shaders::vs::ty::VPUniformBufferObject;


pub struct RenderData {
    pub pipeline: Arc<GraphicsPipeline>,
    pub framebuffer: Arc<Framebuffer>,
    pub ubo_pool: Arc<CpuBufferPool<VPUniformBufferObject>>
}

#[derive(Default)]
pub struct CommandBuffer {
    pub command_buffer: Option<Arc<PrimaryAutoCommandBuffer>>
}

#[derive(Default)]
pub struct ProjectionMatrix(pub Mat4x4);

pub struct ActiveCamera(pub Entity);
