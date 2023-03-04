#![deny(
    nonstandard_style,
    //warnings,
    rust_2018_idioms,
    //unused,
    future_incompatible,
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo
)]

mod data_structures;
mod shaders;

use data_structures::graphics::Vertex;
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::pipeline::GraphicsPipeline;
use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
use vulkano::pipeline::graphics::render_pass;
use vulkano::pipeline::graphics::vertex_input::BuffersDefinition;
use vulkano::pipeline::graphics::viewport::{Viewport, ViewportState};
use vulkano::swapchain::{Swapchain, SwapchainCreateInfo, Surface, SwapchainCreationError, acquire_next_image, AcquireError, PresentInfo};
use vulkano::sync::{self, GpuFuture};
use vulkano::sync::FlushError;
use vulkano_win::VkSurfaceBuild;

use std::sync::Arc;
use anyhow::{anyhow, Result};
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window, WindowBuilder, self};
use vulkano::{VulkanLibrary};
use vulkano::instance::{
    Instance, 
    InstanceCreateInfo,
    Version
};
use vulkano::device::{
    Device,
    DeviceCreateInfo,
    QueueCreateInfo, 
    Queue, DeviceExtensions, physical
};
use vulkano::buffer::{CpuAccessibleBuffer, BufferUsage, TypedBufferAccess};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, RenderPassBeginInfo, SubpassContents, PrimaryAutoCommandBuffer};
use vulkano::format::Format;
use vulkano::image::{StorageImage, ImageDimensions, ImageUsage, SwapchainImage, swapchain};
use vulkano::image::view::ImageView;
use vulkano::render_pass::{RenderPass, Framebuffer, FramebufferCreateInfo, Subpass};


struct App {
    instance: Arc<Instance>,
    physical: Arc<PhysicalDevice>,
    device: Arc<Device>,
    queue: Arc<Queue>,
    framebuffers: Vec<Arc<Framebuffer>>,
    pipeline: Arc<GraphicsPipeline>,
    event_loop: EventLoop<()>,
    surface: Arc<Surface<Window>>,
    swapchain: Arc<Swapchain<Window>>,
    images: Vec<Arc<SwapchainImage<Window>>>,
    command_buffers: Vec<Arc<PrimaryAutoCommandBuffer>>
}

impl App {
    fn create() -> Self {
        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::empty()
        };
        let instance = App::create_instance();

        let event_loop = EventLoop::new();
        let surface = WindowBuilder::new()
            .with_title("HawkEngine")
            .with_inner_size(LogicalSize::new(1024, 768))
            .build_vk_surface(&event_loop, instance)
            .unwrap();

        let (physical, queue_index) = App::select_physical_device(&instance, &surface, &device_extensions);
        let (device, queue) = App::create_device(instance.clone(), physical.clone(), queue_index, &device_extensions);
        let (swapchain, images) = App::create_swapchain(&device, &physical, &surface);
        let (framebuffers, render_pass) = App::create_render_deps(device.clone(), &swapchain, &images);
        let pipeline = App::create_pipeline(device.clone(), render_pass.clone(), surface.clone());
        let command_buffers = App::create_command_buffers(&device, &queue, &pipeline, &framebuffers);
        return Self { instance, device, physical, queue, framebuffers, pipeline, event_loop, surface, swapchain, images, command_buffers };
    }

    fn create_instance() -> Arc<Instance> {
        let library = VulkanLibrary::new().unwrap();
        let required_extensions = vulkano_win::required_extensions(&library);

        let create_info = InstanceCreateInfo {
            application_name: Some("Hawk Engine - Test".into()),
            application_version: Version { major: 0, minor: 0, patch: 1 },
            engine_name: Some("Hawk Engine".into()),
            engine_version: Version { major: 0, minor: 0, patch: 1 },
            enabled_extensions: required_extensions,
            enumerate_portability: true,
            ..Default::default()
        };

        let res = Instance::new(library, create_info)
            .map_err(|b| anyhow!("{}", b)).expect("Failed creating instance");

        return res;
    }

    fn select_physical_device(instance: &Arc<Instance>, surface: &Arc<Surface<Window>>, device_extensions: &DeviceExtensions) -> (Arc<PhysicalDevice>, u32) {
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

    fn create_device(instance: Arc<Instance>, physical: Arc<PhysicalDevice>, queue_family_index: u32, device_extensions: &DeviceExtensions) -> (Arc<Device>, Arc<Queue>) {
        let (device, mut queues) = Device::new(
            physical,
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

    fn create_pipeline(device: Arc<Device>, render_pass: Arc<RenderPass>, surface: Arc<Surface<Window>>) -> Arc<GraphicsPipeline> {
        let vs = shaders::vs::load(device.clone()).expect("Failed to create vs");
        let fs = shaders::fs::load(device.clone()).expect("Failed to load fs");

        let viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: surface.window().inner_size().into(),
            depth_range: 0.0..1.0,
        };

        let pipeline = GraphicsPipeline::start()
            .vertex_input_state(BuffersDefinition::new().vertex::<Vertex>())
            .vertex_shader(vs.entry_point("main").unwrap(), ())
            .input_assembly_state(InputAssemblyState::new())
            .viewport_state(ViewportState::viewport_fixed_scissor_irrelevant([viewport]))
            .fragment_shader(fs.entry_point("main").unwrap(), ())
            .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
            .build(device.clone())
            .unwrap();

        return pipeline;
    }

    fn draw_triangle(device: &Arc<Device>) -> Arc<CpuAccessibleBuffer<[Vertex]>> {
        let v1 = Vertex { position: [-0.5, -0.5 ] };
        let v2 = Vertex { position: [ 0.0,  0.5 ] };
        let v3 = Vertex { position: [ 0.5, -0.25 ] };

        let vertex_buffer = CpuAccessibleBuffer::from_iter(
            device.clone(),
            BufferUsage {
                vertex_buffer: true,
                ..Default::default()
            },
            false,
            vec![v1, v2, v3].into_iter()
        ).unwrap();

        return vertex_buffer;
    }

    fn create_render_deps(device: Arc<Device>, swapchain: &Arc<Swapchain<Window>>, images: &Vec<Arc<SwapchainImage<Window>>>) -> (Vec<Arc<Framebuffer>>, Arc<RenderPass>) {
        let render_pass = vulkano::single_pass_renderpass!(
            device,
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
        ).unwrap();

        let framebuffers = images
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
            .collect::<Vec<_>>();
        

        return (framebuffers, render_pass);
    }

    fn create_swapchain(device: &Arc<Device>, physical: &Arc<PhysicalDevice>, surface: &Arc<Surface<Window>>) -> (Arc<Swapchain<Window>>, Vec<Arc<SwapchainImage<Window>>>) {
        let caps = physical
            .surface_capabilities(surface, Default::default())
            .expect("failed to get surface capabilities");

        let dimensions = surface.window().inner_size();
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
 
    fn create_command_buffers(
        device: &Arc<Device>,
        queue: &Arc<Queue>,
        pipeline: &Arc<GraphicsPipeline>,
        framebuffers: &Vec<Arc<Framebuffer>>,
    ) -> Vec<Arc<PrimaryAutoCommandBuffer>> {
        let vertex_buffer = App::draw_triangle(device);

        framebuffers
            .iter()
            .map(|framebuffer| {
                let mut builder = AutoCommandBufferBuilder::primary(
                    device.clone(),
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

    fn run(&mut self) -> () {
        let mut destroying = false;
        let mut window_resized = false;
        let mut recreate_swapchain = false;

        self.event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;
            match event {
                // Render a frame if app not being destroyed
                Event::MainEventsCleared if !destroying =>
                    { self.render() },
                // After draw
                Event::RedrawEventsCleared => {
                    if window_resized || recreate_swapchain {
                        recreate_swapchain = false;

                        let new_dimensions = self.surface.window().inner_size();

                        let (new_swapchain, new_images) = match self.swapchain.recreate(SwapchainCreateInfo {
                            image_extent: new_dimensions.into(),
                            ..self.swapchain.create_info()
                        }) {
                            Ok(r) => r,
                            // Apparently the creation can fail if the user keeps resizing
                            // In that case we can just try to recreate again on the next frame
                            Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => return,
                            Err(e) => panic!("Failed to recreate swapcahin: {:?}", e),
                        };
                        self.swapchain = new_swapchain;
                        let (new_framebuffers, new_render_pass) = App::create_render_deps(
                            self.device,
                            &self.swapchain,
                            &self.images
                        );

                        if window_resized {
                            window_resized = false;

                            let viewport = Viewport {
                                origin: [0.0, 0.0],
                                dimensions: new_dimensions.into(),
                                depth_range: 0.0..1.0,
                            };

                            let new_pipeline = App::create_pipeline(self.device, new_render_pass, self.surface);
                            self.command_buffers = App::create_command_buffers(&self.device, &self.queue, &new_pipeline, &new_framebuffers)
                        }
                    }

                    let (image_i, suboptimal, acquire_future) =
                        match acquire_next_image(self.swapchain.clone(), None) {
                            Ok(r) => r,
                            Err(AcquireError::OutOfDate) => {
                                recreate_swapchain = true;
                                return;
                            }
                            Err(e) => panic!("Failed to acquire next image: {:?}", e),
                        };
                    
                    if suboptimal {
                        recreate_swapchain = true;
                    }

                    let execution = sync::now(self.device.clone())
                        .join(acquire_future)
                        .then_execute(self.queue.clone(), self.command_buffers[image_i].clone())
                        .unwrap()
                        .then_swapchain_present(
                            self.queue.clone(),
                            PresentInfo {
                                index: image_i,
                                ..PresentInfo::swapchain(self.swapchain.clone())
                            },
                        )
                        .then_signal_fence_and_flush();

                    match execution {
                        Ok(future) => {
                            future.wait(None).unwrap();
                        }
                        Err(FlushError::OutOfDate) => {
                            recreate_swapchain = true;
                        }
                        Err(e) => {
                            println!("Failed to flush future: {:?}", e);
                        }
                    }
                },
                // Resize
                Event::WindowEvent { event: WindowEvent::Resized(_), .. } => {
                    window_resized = true;
                },
                // Destroy the app
                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                    destroying = true;
                    *control_flow = ControlFlow::Exit;
                    { self.destroy(); }
                },
                _ => {}
            }
        });
    }
    
    fn render(&mut self) {

    }

    fn destroy(&mut self) {}
}

#[derive(Clone, Debug, Default)]
struct AppData;

fn main() -> Result<()> {
    pretty_env_logger::init();

    // App
    let mut app = App::create();
    app.run();
    Ok(())
}
