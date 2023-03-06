use crate::data_structures::graphics::Vertex;
use crate::shaders;
use crate::shaders::vs::ty::UniformBufferObject;
use vulkano::buffer::cpu_pool::CpuBufferPoolSubbuffer;
use vulkano::command_buffer::allocator::{StandardCommandBufferAllocator, StandardCommandBufferAllocatorCreateInfo};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::memory::allocator::{StandardMemoryAllocator, MemoryUsage, GenericMemoryAllocator, FreeListAllocator, FastMemoryAllocator};
use vulkano::pipeline::{GraphicsPipeline, Pipeline, PipelineBindPoint};
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::BuffersDefinition;
use vulkano::pipeline::graphics::viewport::{Viewport, ViewportState};
use vulkano::swapchain::{Swapchain, SwapchainCreateInfo, Surface};
use vulkano_win::VkSurfaceBuild;

use std::sync::Arc;
use anyhow::{anyhow};
use winit::dpi::LogicalSize;
use winit::event_loop::{EventLoop};
use winit::window::{Window, WindowBuilder};
use vulkano::{VulkanLibrary, descriptor_set};
use vulkano::instance::{
    Instance, 
    InstanceCreateInfo,
    Version, InstanceExtensions
};
use vulkano::device::{
    Device,
    DeviceCreateInfo,
    QueueCreateInfo, 
    Queue, DeviceExtensions
};
use vulkano::buffer::{CpuAccessibleBuffer, BufferUsage, TypedBufferAccess, CpuBufferPool};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, RenderPassBeginInfo, SubpassContents, PrimaryAutoCommandBuffer};
use vulkano::image::{ImageUsage, SwapchainImage};
use vulkano::image::view::ImageView;
use vulkano::render_pass::{RenderPass, Framebuffer, FramebufferCreateInfo, Subpass};

pub struct Vulkan {
    device: Arc<Device>,
    buffer_memory_allocator: Arc<StandardMemoryAllocator>,
    command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
    fast_buffer_memory_allocator: Arc<FastMemoryAllocator>,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>
}

impl Vulkan {
    pub fn new(device: &Arc<Device>) -> Self {
        let buffer_memory_allocator = Arc::new(StandardMemoryAllocator::new_default(device.clone()));
        let command_buffer_allocator = Arc::new(StandardCommandBufferAllocator::new(device.clone(), StandardCommandBufferAllocatorCreateInfo::default()));
        let fast_buffer_memory_allocator = Arc::new(FastMemoryAllocator::new_default(device.clone()));
        let descriptor_set_allocator = Arc::new(StandardDescriptorSetAllocator::new(device.clone()));

        Self { device: device.clone(), buffer_memory_allocator, command_buffer_allocator, fast_buffer_memory_allocator, descriptor_set_allocator }
    }

    pub fn create_surface(instance: &Arc<Instance>, event_loop: &EventLoop<()>) -> Arc<Surface> {
        WindowBuilder::new()
        .with_title("HawkEngine")
        .with_inner_size(LogicalSize::new(1024, 768))
        .build_vk_surface(event_loop, instance.clone())
        .unwrap()
    }
    
    pub fn create_instance(debug: bool) -> Arc<Instance> {
        let library = VulkanLibrary::new().unwrap();
        let required_extensions = vulkano_win::required_extensions(&library);
    
        const VALIDATION_LAYER_NAME: &str = "VK_LAYER_KHRONOS_validation";
        let mut layers: Vec<String> = vec![];
    
        if debug {
            // Iterate layers for validation layer support
            let has_validation_support = library
                .layer_properties()
                .unwrap()
                .any(|v| { v.name() == VALIDATION_LAYER_NAME });
            if has_validation_support {
                layers = vec![VALIDATION_LAYER_NAME.to_string()];
            }
        }
    
        let extensions = InstanceExtensions {
            ext_debug_utils: debug,
            ..required_extensions
        };
    
        let create_info = InstanceCreateInfo {
            application_name: Some("Hawk Engine - Test".into()),
            application_version: Version { major: 0, minor: 0, patch: 1 },
            engine_name: Some("Hawk Engine".into()),
            engine_version: Version { major: 0, minor: 0, patch: 1 },
            enabled_extensions: extensions,
            enabled_layers: layers,
            enumerate_portability: true,
            ..Default::default()
        };
    
        Instance::new(library, create_info)
            .map_err(|b| anyhow!("{}", b)).expect("Failed creating instance")
    }
    
    pub fn select_physical_device(instance: &Arc<Instance>, surface: &Arc<Surface>, device_extensions: &DeviceExtensions) -> (Arc<PhysicalDevice>, u32) {
        instance
            .enumerate_physical_devices()
            .expect("could not enumerate devices")
            .filter(|p| p.supported_extensions().contains(&device_extensions))
            .filter_map(|p| {
                p.queue_family_properties()
                    .iter()
                    .enumerate()
                    .position(|(i, q)| {
                        q.queue_flags.graphics && p.surface_support(i as u32, &surface).unwrap_or(false)
                    })
                    .map(|q| (p, q as u32))
            })
            .min_by_key(|(p, _)| match p.properties().device_type {
                PhysicalDeviceType::DiscreteGpu => 0,
                PhysicalDeviceType::IntegratedGpu => 1,
                PhysicalDeviceType::VirtualGpu => 2,
                PhysicalDeviceType::Cpu => 3,
                
                _ => 4
            })
            .expect("no device available")
    }
    
    pub fn create_device(physical: &Arc<PhysicalDevice>, queue_family_index: u32, device_extensions: &DeviceExtensions) -> (Arc<Device>, Arc<Queue>) {
        let (device, mut queues) = Device::new(
            physical.clone(),
            DeviceCreateInfo { 
                queue_create_infos: vec![QueueCreateInfo {
                    queue_family_index,
                    ..Default::default()
                }],
                enabled_extensions: *device_extensions,
                ..Default::default()
            }
        )
        .expect("failed to create device");
    
        return (device, queues.next().unwrap());
    }
    
    pub fn create_pipeline(
        &self,
        render_pass: &Arc<RenderPass>, 
        surface: &Arc<Surface>, 
        viewport: Option<&Viewport>
    ) -> Arc<GraphicsPipeline> {
        let vs = shaders::vs::load(self.device.clone()).expect("Failed to create vs");
        let fs = shaders::fs::load(self.device.clone()).expect("Failed to load fs");
    
        let viewport_value = match viewport {
            Some(viewport) => viewport.clone(),
            None => Viewport {
                origin: [0.0, 0.0],
                dimensions: surface.object().unwrap().downcast_ref::<Window>().unwrap().inner_size().into(),
                depth_range: 0.0..1.0,
            }
        };
    
        let pipeline = GraphicsPipeline::start()
            .vertex_input_state(BuffersDefinition::new().vertex::<Vertex>())
            .vertex_shader(vs.entry_point("main").unwrap(), ())
            .input_assembly_state(InputAssemblyState::new())
            .viewport_state(ViewportState::viewport_fixed_scissor_irrelevant([viewport_value]))
            .fragment_shader(fs.entry_point("main").unwrap(), ())
            .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
            .build(self.device.clone())
            .unwrap();
    
        return pipeline;
    }
    
    pub fn create_render_pass(&self, swapchain: &Arc<Swapchain>,) -> Arc<RenderPass> {
        vulkano::single_pass_renderpass!(
            self.device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: swapchain.image_format(),
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {}
            }
        ).unwrap()
    }
    
    pub fn create_framebuffers(&self, render_pass: &Arc<RenderPass>, images: &Vec<Arc<SwapchainImage>>) -> Vec<Arc<Framebuffer>> {
        images
            .iter()
            .map(|image| {
                let view = ImageView::new_default(image.clone()).unwrap();
                Framebuffer::new(
                    render_pass.clone(),
                    FramebufferCreateInfo { 
                        attachments: vec![view],
                        ..Default::default()
                    }
                ).unwrap()
            })
            .collect::<Vec<_>>()
    }
    
    pub fn create_swapchain(&self, physical: &Arc<PhysicalDevice>, surface: &Arc<Surface>) -> (Arc<Swapchain>, Vec<Arc<SwapchainImage>>) {
        let caps = physical
            .surface_capabilities(surface, Default::default())
            .expect("failed to get surface capabilities");
    
        let dimensions = surface.object().unwrap().downcast_ref::<Window>().unwrap().inner_size();
        let composite_alpha = caps.supported_composite_alpha.iter().next().unwrap();
        let image_format = Some(
            physical
                .surface_formats(surface, Default::default())
                .unwrap()[0]
                .0,
        );
    
        Swapchain::new(
            self.device.clone(),
            surface.clone(),
            SwapchainCreateInfo {
                min_image_count: caps.min_image_count + 1,
                image_format,
                image_extent: dimensions.into(),
                image_usage: ImageUsage {
                    color_attachment: true,
                    ..Default::default()
                },
                composite_alpha,
                ..Default::default()
            }
        ).unwrap()
    }
    
    pub fn construct_triangle(&self) -> (
        Arc<CpuAccessibleBuffer<[Vertex]>>, 
        Arc<CpuAccessibleBuffer<[u16]>>
    ) {
        let v1 = Vertex { position: [-0.5, -0.5, 0.0], color: [1.0, 0.0, 0.0] };
        let v2 = Vertex { position: [ 0.5, -0.5, 0.0], color: [0.0, 1.0, 0.0] };
        let v3 = Vertex { position: [ 0.5,  0.5, 0.0], color: [0.0, 0.0, 1.0] };
        let v4 = Vertex { position: [-0.5,  0.5, 0.0], color: [1.0, 1.0, 1.0] };
    
        let indices: [u16; 6] = [0, 1, 2, 2, 3, 0];
    
        let vertex_buffer = CpuAccessibleBuffer::from_iter(
            &self.buffer_memory_allocator,
            BufferUsage {
                vertex_buffer: true,
                ..Default::default()
            },
            false,
            vec![v1, v2, v3, v4].into_iter()
        ).unwrap();
    
        let index_buffer = CpuAccessibleBuffer::from_iter(
            &self.buffer_memory_allocator,
            BufferUsage {
                index_buffer: true,
                ..Default::default()
            },
            false,
            indices.into_iter()
        ).unwrap();
    
        return (vertex_buffer, index_buffer);
    }
    
    pub fn create_view_ubo_pool(&self) -> Arc<CpuBufferPool<UniformBufferObject>> {
        CpuBufferPool::<UniformBufferObject>::new(
            self.buffer_memory_allocator.clone(),
            BufferUsage {
                uniform_buffer: true,
                ..Default::default()
            },
            MemoryUsage::Upload
        ).into()
    }
    
    pub fn create_command_buffer(
        &self,
        queue: &Arc<Queue>,
        pipeline: &Arc<GraphicsPipeline>,
        framebuffer: &Arc<Framebuffer>,
        vertex_buffer: &Arc<CpuAccessibleBuffer<[Vertex]>>,
        index_buffer: &Arc<CpuAccessibleBuffer<[u16]>>,
        view_ubo: &Arc<CpuBufferPoolSubbuffer<UniformBufferObject>>,
    ) -> Arc<PrimaryAutoCommandBuffer> {
        // TODO: don't recreate the command buffer anew, but reset and write over the same one

        let layout = pipeline.layout().set_layouts().get(0).unwrap();
        let descriptor_set = PersistentDescriptorSet::new(
            &self.descriptor_set_allocator,
            layout.clone(),
            [WriteDescriptorSet::buffer(0, view_ubo.clone())]
        ).unwrap();

        let mut builder = AutoCommandBufferBuilder::primary(
            &self.command_buffer_allocator,
            queue.queue_family_index(),
            CommandBufferUsage::MultipleSubmit
        ).unwrap();
    
        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values: vec![Some([0.1, 0.1, 0.1, 1.0].into())],
                    ..RenderPassBeginInfo::framebuffer(framebuffer.clone())
                },
                SubpassContents::Inline,
            )
            .unwrap()
            .bind_pipeline_graphics(pipeline.clone())
            .bind_descriptor_sets(PipelineBindPoint::Graphics, pipeline.layout().clone(), 0, descriptor_set)
            .bind_vertex_buffers(0, vertex_buffer.clone())
            .bind_index_buffer(index_buffer.clone())
            .draw_indexed(index_buffer.len() as u32, 1, 0, 0, 0)
            .unwrap()
            .end_render_pass()
            .unwrap();
    
        Arc::new(builder.build().unwrap())
    }
    
    pub fn create_command_buffers(
        &self,
        queue: &Arc<Queue>,
        pipeline: &Arc<GraphicsPipeline>,
        framebuffers: &Vec<Arc<Framebuffer>>
    ) -> Vec<Arc<PrimaryAutoCommandBuffer>> {
        let (vertex_buffer, index_buffer) = self.construct_triangle();
    
        framebuffers
            .iter()
            .map(|framebuffer| {
                let mut builder = AutoCommandBufferBuilder::primary(
                    &self.command_buffer_allocator,
                    queue.queue_family_index(),
                    CommandBufferUsage::MultipleSubmit
                ).unwrap();
    
                builder
                    .begin_render_pass(
                        RenderPassBeginInfo {
                            clear_values: vec![Some([0.1, 0.1, 0.1, 1.0].into())],
                            ..RenderPassBeginInfo::framebuffer(framebuffer.clone())
                        },
                        SubpassContents::Inline,
                    )
                    .unwrap()
                    .bind_pipeline_graphics(pipeline.clone())
                    .bind_vertex_buffers(0, vertex_buffer.clone())
                    .bind_index_buffer(index_buffer.clone())
                    .draw_indexed(index_buffer.len() as u32, 1, 0, 0, 0)
                    .unwrap()
                    .end_render_pass()
                    .unwrap();
    
                Arc::new(builder.build().unwrap())
            })
            .collect()
    }
}