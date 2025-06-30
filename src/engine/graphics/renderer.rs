use crate::ecs::resources::{CommandBuffer, CursorGrab, DeltaTime, ProjectionMatrix, RenderData, RenderDataFrameBuffer};
use crate::ecs::utils::input::InputHelper;
use crate::graphics::vulkan::Vulkan;
use crate::{shaders, HawkEngine};
use nalgebra::Perspective3;
use vulkano::buffer::Buffer;
use vulkano::pipeline::graphics::rasterization::{RasterizationState, PolygonMode};
use vulkano::pipeline::{GraphicsPipeline};
use vulkano::swapchain::{Swapchain, Surface};
use winit::window::{Window};

use std::sync::Arc;
use winit::event_loop::ActiveEventLoop;
use vulkano::device::{
    Device, 
    Queue, DeviceExtensions,
};
use vulkano::image::{Image};
use vulkano::render_pass::{RenderPass, Framebuffer};


#[cfg(all(debug_assertions))]
const ENABLE_VALIDATION_LAYERS: bool = true;
#[cfg(not(debug_assertions))]
const ENABLE_VALIDATION_LAYERS: bool = false;

pub struct Renderer {
    pub(crate) device: Arc<Device>,
    pub(crate) queue: Arc<Queue>,
    pub(crate) render_pass: Arc<RenderPass>,
    pub(crate) framebuffers: Vec<Arc<Framebuffer>>,
    pipeline: Arc<GraphicsPipeline>,
    pipeline_wireframe: Arc<GraphicsPipeline>,
    surface: Arc<Surface>,
    pub(crate) swapchain: Arc<Swapchain>,
    pub(crate) images: Vec<Arc<Image>>,
    ubo_pool: Arc<Buffer>,

    pub vulkan: Vulkan,
}

impl Renderer {
    /*
    If use_physics is true, PhysicsData is expected to be provided as a resource
    */
    pub fn new(event_loop: &ActiveEventLoop, window: Arc<Window>) -> Self {
        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::empty()
        };

        let instance = Vulkan::create_instance(&event_loop, ENABLE_VALIDATION_LAYERS);
        let surface = Vulkan::create_surface(&instance, window.clone());
        let (physical, queue_index) = Vulkan::select_physical_device(&instance, &surface, &device_extensions);
        let (device, queue) = Vulkan::create_device(&physical, queue_index, &device_extensions);

        let mut vulkan = Vulkan::new(&device, &queue);

        // Default
        let vs = shaders::default::vs::load(device.clone()).expect("Failed to load default vs");
        let fs = shaders::default::fs::load(device.clone()).expect("Failed to load default fs");
        // Wireframe
        let vsw = shaders::wireframe::vs::load(device.clone()).expect("Failed to load wireframe vs");
        let fsw = shaders::wireframe::fs::load(device.clone()).expect("Failed to load wireframe fs");

        let (swapchain, images) = vulkan.create_swapchain(&physical, &surface);
        let render_pass = vulkan.create_render_pass(&swapchain);
        let framebuffers= vulkan.create_framebuffers(&render_pass, &images);
        let pipeline = vulkan.create_pipeline("default", &render_pass, &surface, &vs, &fs, None, None);
        let rasterization_state = RasterizationState { polygon_mode: PolygonMode::Line, ..Default::default() };
        let pipeline_wireframe = vulkan.create_pipeline("wireframe", &render_pass, &surface, &vsw, &fsw, None, Some(&rasterization_state));
        let ubo_pool = vulkan.create_view_ubo_pool();
        return Self { device, queue, render_pass, framebuffers, pipeline, pipeline_wireframe, surface, swapchain, images, ubo_pool, vulkan };
    }

    pub fn setup_engine(&self, engine: &mut HawkEngine<'_>) {
        let input = InputHelper::new();

        let mut proj = Perspective3::new(
            self.swapchain.image_extent()[0] as f32 / self.swapchain.image_extent()[1] as f32,
            (45.0 as f32).to_radians(),
            0.1,
            1000.0,
        ).to_homogeneous();
        // convert from OpenGL to Vulkan coordinates
        proj[(1, 1)] *= -1.0;
        
        // Add initial input
        engine.ecs.world.insert(input.clone());
        // Add initial surface
        engine.ecs.world.insert(self.surface.clone());
        // Add initial cursor grab
        engine.ecs.world.insert(CursorGrab::default());
        // Add projection matrix
        engine.ecs.world.insert(ProjectionMatrix(proj));
        // Add initial render data
        engine.ecs.world.insert(RenderData {
            pipeline: self.pipeline.clone(),
            pipeline_wireframe: self.pipeline_wireframe.clone(),
            ubo_pool: self.ubo_pool.clone(),
            buffer_allocator: self.vulkan.buffer_memory_allocator.clone(),
            command_buffer_allocator: self.vulkan.command_buffer_allocator.clone(),
            descriptor_set_allocator: self.vulkan.descriptor_set_allocator.clone(),
            queue_family_index: self.vulkan.queue.queue_family_index()
        });
        engine.ecs.world.insert(RenderDataFrameBuffer(self.framebuffers[0].clone()));
        // Add empty command buffer
        engine.ecs.world.insert(CommandBuffer { command_buffer: None });
        // Add 0 delta time
        engine.ecs.world.insert(DeltaTime(0.0));
    }
}

