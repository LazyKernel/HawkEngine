use crate::data_structures::graphics::Vertex;
use crate::ecs::components::general::Renderable;
use crate::shaders;
use crate::shaders::vs::ty::VPUniformBufferObject;
use vulkano::buffer::cpu_pool::CpuBufferPoolSubbuffer;
use vulkano::command_buffer::allocator::{StandardCommandBufferAllocator, StandardCommandBufferAllocatorCreateInfo};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet, DescriptorSet};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::format::Format;
use vulkano::memory::allocator::{StandardMemoryAllocator, MemoryUsage, FastMemoryAllocator};
use vulkano::pipeline::graphics::color_blend::ColorBlendState;
use vulkano::pipeline::graphics::depth_stencil::DepthStencilState;
use vulkano::pipeline::{GraphicsPipeline, Pipeline, PipelineBindPoint};
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::vertex_input::BuffersDefinition;
use vulkano::pipeline::graphics::viewport::{Viewport, ViewportState};
use vulkano::sampler::{Sampler, SamplerCreateInfo, Filter, SamplerAddressMode};
use vulkano::swapchain::{Swapchain, SwapchainCreateInfo, Surface};
use vulkano::sync::GpuFuture;
use vulkano_win::VkSurfaceBuild;

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use anyhow::{anyhow};
use winit::dpi::LogicalSize;
use winit::event_loop::{EventLoop};
use winit::window::{Window, WindowBuilder};
use vulkano::{VulkanLibrary};
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
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, RenderPassBeginInfo, SubpassContents, PrimaryAutoCommandBuffer, CommandBufferLevel, PrimaryCommandBufferAbstract};
use vulkano::image::{ImageUsage, SwapchainImage, ImmutableImage, ImageDimensions, MipmapsCount, ImageAccess, AttachmentImage};
use vulkano::image::view::ImageView;
use vulkano::render_pass::{RenderPass, Framebuffer, FramebufferCreateInfo, Subpass};

#[derive(Clone)]
pub struct Vulkan {
    device: Arc<Device>,
    pub queue: Arc<Queue>,
    sampler: Arc<Sampler>,
    pipelines: HashMap<String, Arc<GraphicsPipeline>>,
    buffer_memory_allocator: Arc<StandardMemoryAllocator>,
    pub command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
    fast_buffer_memory_allocator: Arc<FastMemoryAllocator>,
    // TODO: temporarily public
    pub descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>
}

impl Vulkan {
    /*
    The functions should be called in the correct order
    1.  Create Vulkan instance using create_instance
    2.  Create Vulkan surface and winit window using create_surface
    3.  Select the best available physical device (gpu) using select_physical_device
    4.  Create Vulkan device using create_device
    5.  Call the constructor of this class
    6.  Create the swapchain images for n buffering using create_swapchain
    7.  Create the render pass, we only use a single one at the moment using create_render_pass
    8.  Create the framebuffers for each swapchain image using create_framebuffers
    9.  Create the Vulkan graphics pipeline using create_pipeline
    10. Create the CpuBuffer Pool for allocating UniformBufferObjects for the view matrix using create_ubo_pool
    */

    pub fn new(device: &Arc<Device>, queue: &Arc<Queue>) -> Self {
        let buffer_memory_allocator = Arc::new(StandardMemoryAllocator::new_default(device.clone()));
        let command_buffer_allocator = Arc::new(StandardCommandBufferAllocator::new(device.clone(), StandardCommandBufferAllocatorCreateInfo::default()));
        let fast_buffer_memory_allocator = Arc::new(FastMemoryAllocator::new_default(device.clone()));
        let descriptor_set_allocator = Arc::new(StandardDescriptorSetAllocator::new(device.clone()));

        let sampler = Sampler::new(
            device.clone(),
            SamplerCreateInfo {
                mag_filter: Filter::Linear,
                min_filter: Filter::Linear,
                address_mode: [SamplerAddressMode::Repeat; 3],
                ..Default::default()
            }
        ).unwrap();

        Self { 
            device: device.clone(), 
            queue: queue.clone(), 
            sampler: sampler.clone(),
            pipelines: HashMap::new(),
            buffer_memory_allocator, 
            command_buffer_allocator, 
            fast_buffer_memory_allocator,
            descriptor_set_allocator
        }
    }


    //--------------------------
    // Static functions
    //--------------------------

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

    pub fn create_surface(instance: &Arc<Instance>, event_loop: &EventLoop<()>) -> Arc<Surface> {
        WindowBuilder::new()
        .with_title("HawkEngine")
        .with_inner_size(LogicalSize::new(1024, 768))
        .build_vk_surface(event_loop, instance.clone())
        .unwrap()
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

    //--------------------------
    // Member functions
    //--------------------------

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

    pub fn create_render_pass(&self, swapchain: &Arc<Swapchain>) -> Arc<RenderPass> {
        vulkano::single_pass_renderpass!(
            self.device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: swapchain.image_format(),
                    samples: 1,
                },
                depth: {
                    load: Clear,
                    store: DontCare,
                    format: Format::D16_UNORM,
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {depth}
            }
        ).unwrap()
    }
    
    pub fn create_framebuffers(&self, render_pass: &Arc<RenderPass>, images: &Vec<Arc<SwapchainImage>>) -> Vec<Arc<Framebuffer>> {
        // Create depth buffer
        let dimensions = images[0].dimensions().width_height();
        let depth_buffer = ImageView::new_default(
            AttachmentImage::transient(&self.buffer_memory_allocator, dimensions, Format::D16_UNORM).unwrap()
        ).unwrap();

        images
            .iter()
            .map(|image| {
                let view = ImageView::new_default(image.clone()).unwrap();
                Framebuffer::new(
                    render_pass.clone(),
                    FramebufferCreateInfo { 
                        attachments: vec![view, depth_buffer.clone()],
                        ..Default::default()
                    }
                ).unwrap()
            })
            .collect::<Vec<_>>()
    }
    
    pub fn create_pipeline(
        &mut self,
        pipeline_name: &str,
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
    
        let subpass = Subpass::from(render_pass.clone(), 0).unwrap();
        let pipeline = GraphicsPipeline::start()
            .vertex_input_state(BuffersDefinition::new().vertex::<Vertex>())
            .vertex_shader(vs.entry_point("main").unwrap(), ())
            .input_assembly_state(InputAssemblyState::new())
            .viewport_state(ViewportState::viewport_fixed_scissor_irrelevant([viewport_value]))
            .fragment_shader(fs.entry_point("main").unwrap(), ())
            .color_blend_state(ColorBlendState::new(subpass.num_color_attachments()).blend_alpha())
            .depth_stencil_state(DepthStencilState::simple_depth_test())
            .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
            .build(self.device.clone())
            .unwrap();
    
        // Insert to pipelines so we can use it later without needing a reference
        self.pipelines.insert(pipeline_name.into(), pipeline.clone());

        return pipeline;
    }

    pub fn create_view_ubo_pool(&self) -> Arc<CpuBufferPool<VPUniformBufferObject>> {
        CpuBufferPool::<VPUniformBufferObject>::new(
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
        pipeline: &Arc<GraphicsPipeline>,
        framebuffer: &Arc<Framebuffer>,
        vertex_buffer: &Arc<CpuAccessibleBuffer<[Vertex]>>,
        index_buffer: &Arc<CpuAccessibleBuffer<[u32]>>,
        view_ubo: &Arc<CpuBufferPoolSubbuffer<VPUniformBufferObject>>,
        descriptor_set_texture: &Arc<PersistentDescriptorSet>
    ) -> Arc<PrimaryAutoCommandBuffer> {
        // TODO: don't recreate the command buffer anew, but reset and write over the same one
        // Not gonna optimize yet, since the library seems to have some type of optimizations already

        // Setup MVP descriptor set
        let layout_view = pipeline.layout().set_layouts().get(0).unwrap();
        let descriptor_set_view = PersistentDescriptorSet::new(
            &self.descriptor_set_allocator,
            layout_view.clone(),
            [WriteDescriptorSet::buffer(0, view_ubo.clone())]
        ).unwrap();

        let mut builder = AutoCommandBufferBuilder::primary(
            &self.command_buffer_allocator,
            self.queue.queue_family_index(),
            CommandBufferUsage::MultipleSubmit
        ).unwrap();

        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values: vec![Some([0.0, 0.0, 0.0, 1.0].into()), Some(1f32.into())],
                    ..RenderPassBeginInfo::framebuffer(framebuffer.clone())
                },
                SubpassContents::Inline,
            )
            .unwrap()
            .bind_pipeline_graphics(pipeline.clone())
            .bind_descriptor_sets(PipelineBindPoint::Graphics, pipeline.layout().clone(), 0, descriptor_set_view.clone())
            .bind_descriptor_sets(PipelineBindPoint::Graphics, pipeline.layout().clone(), 1, descriptor_set_texture.clone())
            .bind_vertex_buffers(0, vertex_buffer.clone())
            .bind_index_buffer(index_buffer.clone())
            .draw_indexed(index_buffer.len() as u32, 1, 0, 0, 0)
            .unwrap()
            .end_render_pass()
            .unwrap();
    
        Arc::new(builder.build().unwrap())
    }


    //--------------------------
    // Utils
    //--------------------------
    
    pub fn load_image(&self, path: &str) -> (Arc<ImageView<ImmutableImage>>, Box<dyn GpuFuture>) {
        // TODO: add error handling
        let image = File::open(path).unwrap();

        let decoder = png::Decoder::new(image);
        let mut reader = decoder.read_info().unwrap();

        let mut pixels = vec![0; reader.info().raw_bytes()];
        reader.next_frame(&mut pixels).unwrap();

        let size = reader.info().raw_bytes() as u64;
        let (width, height) = reader.info().size();

        let dimensions = ImageDimensions::Dim2d { 
            width, 
            height, 
            array_layers: 1 
        };

        let mut uploads = AutoCommandBufferBuilder::primary(
            &self.command_buffer_allocator,
            self.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        let image = ImmutableImage::from_iter(
            &self.buffer_memory_allocator,
            pixels,
            dimensions,
            MipmapsCount::One,
            Format::R8G8B8A8_SRGB,
            &mut uploads
        ).unwrap();

        // Need to use the created command buffer to upload the texture to the gpu
        let mut image_upload = uploads
            .build()
            .unwrap()
            .execute(self.queue.clone())
            .unwrap()
            .boxed();

        // TODO: move this to somewhere smart for cleanup
        //image_upload.as_mut().cleanup_finished();

        let texture = ImageView::new_default(image).unwrap();

        return (texture, image_upload);
    }

    pub fn load_model(&self, path: &str) -> (
        Arc<CpuAccessibleBuffer<[Vertex]>>, 
        Arc<CpuAccessibleBuffer<[u32]>>
    ) {
        // TODO: add error handling
        let mut reader = BufReader::new(File::open(path).unwrap());

        let (models, _) = tobj::load_obj_buf(
            &mut reader, 
            &tobj::LoadOptions { triangulate: true, single_index: true, ..Default::default() }, 
            |_| Ok(Default::default())
        ).unwrap();

        let mut vertices: Vec<Vertex> = Vec::with_capacity(1000);
        let mut indices: Vec<u32> = Vec::with_capacity(1000);
        let mut unique_vertices = HashMap::new();
        for model in &models {
            for index in &model.mesh.indices {
                let pos_offset = (3 * index) as usize;
                let normal_offset = (3 * index) as usize;
                let tex_coord_offset = (2 * index) as usize;

                let vertex = Vertex {
                    position: [
                        model.mesh.positions[pos_offset],
                        model.mesh.positions[pos_offset + 1], 
                        model.mesh.positions[pos_offset + 2]
                    ],
                    normal: [
                        model.mesh.normals[normal_offset],
                        model.mesh.normals[normal_offset + 1], 
                        model.mesh.normals[normal_offset + 2]
                    ],
                    color: [1.0, 1.0, 1.0],
                    tex_coord: [
                        model.mesh.texcoords[tex_coord_offset], 
                        1.0 - model.mesh.texcoords[tex_coord_offset + 1]
                    ]
                };

                if let Some(index) = unique_vertices.get(&vertex) {
                    indices.push(*index as u32);
                }
                else {
                    let index = vertices.len();
                    unique_vertices.insert(vertex, index);
                    vertices.push(vertex);
                    indices.push(index as u32);
                }
            }
        };

        let vertex_buffer = CpuAccessibleBuffer::from_iter(
            &self.buffer_memory_allocator,
            BufferUsage {
                vertex_buffer: true,
                ..Default::default()
            },
            false,
            vertices.into_iter()
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

    
    pub fn create_renderable(&self, model_name: &str, pipeline_name: Option<String>) -> Result<Renderable, String> {
        let model_path = format!("resources/{}.obj", model_name);
        let texture_path = format!("resources/{}.png", model_name);
        let (vertices, indices) = self.load_model(&model_path);
        let (texture, image_upload) = self.load_image(&texture_path);
        // TODO: save image_upload to an array and periodically check if they are finished
        // Should also probably check that the upload has finished before using it

        let pipeline_name = match pipeline_name {
            Some(v) => v,
            None => "default".into()
        };

        let pipeline = self.pipelines.get(&pipeline_name);

        let pipeline = match pipeline {
            Some(v) => v,
            None => return Err(format!("No pipeline called '{}' exists", pipeline_name))
        };

        let layout_texture = pipeline.layout().set_layouts().get(1).unwrap();
        let descriptor_set_texture = PersistentDescriptorSet::new(
            &self.descriptor_set_allocator,
            layout_texture.clone(),
            [WriteDescriptorSet::image_view_sampler(0, texture.clone(), self.sampler.clone())]
        ).unwrap();

        Ok(Renderable { vertex_buffer: vertices, index_buffer: indices, descriptor_set_texture })
    }

}
