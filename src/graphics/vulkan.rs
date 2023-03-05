use crate::data_structures::graphics::Vertex;
use crate::shaders;
use vulkano::command_buffer::allocator::{StandardCommandBufferAllocator, StandardCommandBufferAllocatorCreateInfo};
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::memory::allocator::StandardMemoryAllocator;
use vulkano::pipeline::GraphicsPipeline;
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
use vulkano::buffer::{CpuAccessibleBuffer, BufferUsage, TypedBufferAccess};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, RenderPassBeginInfo, SubpassContents, PrimaryAutoCommandBuffer};
use vulkano::image::{ImageUsage, SwapchainImage};
use vulkano::image::view::ImageView;
use vulkano::render_pass::{RenderPass, Framebuffer, FramebufferCreateInfo, Subpass};

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
    device: &Arc<Device>, 
    render_pass: &Arc<RenderPass>, 
    surface: &Arc<Surface>, 
    viewport: Option<&Viewport>
) -> Arc<GraphicsPipeline> {
    let vs = shaders::vs::load(device.clone()).expect("Failed to create vs");
    let fs = shaders::fs::load(device.clone()).expect("Failed to load fs");

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
        .build(device.clone())
        .unwrap();

    return pipeline;
}

pub fn create_render_pass(device: &Arc<Device>, swapchain: &Arc<Swapchain>,) -> Arc<RenderPass> {
    vulkano::single_pass_renderpass!(
        device.clone(),
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

pub fn create_framebuffers(render_pass: &Arc<RenderPass>, images: &Vec<Arc<SwapchainImage>>) -> Vec<Arc<Framebuffer>> {
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

pub fn create_swapchain(device: &Arc<Device>, physical: &Arc<PhysicalDevice>, surface: &Arc<Surface>) -> (Arc<Swapchain>, Vec<Arc<SwapchainImage>>) {
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
        device.clone(),
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

pub fn construct_triangle(device: &Arc<Device>) -> Arc<CpuAccessibleBuffer<[Vertex]>> {
    let v1 = Vertex { position: [-0.5, -0.5 ] };
    let v2 = Vertex { position: [ 0.0,  0.5 ] };
    let v3 = Vertex { position: [ 0.5, -0.25 ] };

    let memory_allocator = StandardMemoryAllocator::new_default(device.clone());

    let vertex_buffer = CpuAccessibleBuffer::from_iter(
        &memory_allocator,
        BufferUsage {
            vertex_buffer: true,
            ..Default::default()
        },
        false,
        vec![v1, v2, v3].into_iter()
    ).unwrap();

    return vertex_buffer;
}

pub fn create_command_buffers(
    device: &Arc<Device>,
    queue: &Arc<Queue>,
    pipeline: &Arc<GraphicsPipeline>,
    framebuffers: &Vec<Arc<Framebuffer>>,
) -> Vec<Arc<PrimaryAutoCommandBuffer>> {
    let vertex_buffer = construct_triangle(device);

    let memory_allocator = StandardCommandBufferAllocator::new(device.clone(), StandardCommandBufferAllocatorCreateInfo::default());

    framebuffers
        .iter()
        .map(|framebuffer| {
            let mut builder = AutoCommandBufferBuilder::primary(
                &memory_allocator,
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
                .draw(vertex_buffer.len() as u32, 1, 0, 0)
                .unwrap()
                .end_render_pass()
                .unwrap();

            Arc::new(builder.build().unwrap())
        })
        .collect()
}
