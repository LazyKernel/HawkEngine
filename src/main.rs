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
mod graphics;

use graphics::vulkan::Vulkan;
use shaders::vs::ty::UniformBufferObject;
use vulkano::buffer::CpuBufferPool;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::device::physical::{PhysicalDevice};
use vulkano::pipeline::{GraphicsPipeline, Pipeline};
use vulkano::pipeline::graphics::viewport::{Viewport};
use vulkano::swapchain::{Swapchain, SwapchainCreateInfo, Surface, SwapchainCreationError, acquire_next_image, AcquireError, SwapchainPresentInfo};
use vulkano::sync::{self, GpuFuture, FenceSignalFuture};
use vulkano::sync::FlushError;

use std::sync::Arc;
use std::time::Instant;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window};
use vulkano::instance::{
    Instance
};
use vulkano::device::{
    Device, 
    Queue, DeviceExtensions,
};
use vulkano::image::{SwapchainImage};
use vulkano::render_pass::{RenderPass, Framebuffer};

const VALIDATION_LAYER: &[&str] = &[
    "VK_LAYER_LUNARG_standard_validation"
];

#[cfg(all(debug_assertions))]
const ENABLE_VALIDATION_LAYERS: bool = true;
#[cfg(not(debug_assertions))]
const ENABLE_VALIDATION_LAYERS: bool = false;

struct App {
    instance: Arc<Instance>,
    physical: Arc<PhysicalDevice>,
    device: Arc<Device>,
    queue: Arc<Queue>,
    render_pass: Arc<RenderPass>,
    framebuffers: Vec<Arc<Framebuffer>>,
    pipeline: Arc<GraphicsPipeline>,
    surface: Arc<Surface>,
    swapchain: Arc<Swapchain>,
    images: Vec<Arc<SwapchainImage>>,
    ubo_pool: Arc<CpuBufferPool<UniformBufferObject>>,

    vulkan: Vulkan,

    start: Instant
}

impl App {
    fn create(event_loop: &EventLoop<()>) -> Self {
        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::empty()
        };

        let instance = Vulkan::create_instance(ENABLE_VALIDATION_LAYERS);
        let surface = Vulkan::create_surface(&instance, event_loop);
        let (physical, queue_index) = Vulkan::select_physical_device(&instance, &surface, &device_extensions);
        let (device, queue) = Vulkan::create_device(&physical, queue_index, &device_extensions);

        let vulkan = Vulkan::new(&device, &queue);

        let (swapchain, images) = vulkan.create_swapchain(&physical, &surface);
        let render_pass = vulkan.create_render_pass(&swapchain);
        let framebuffers= vulkan.create_framebuffers(&render_pass, &images);
        let pipeline = vulkan.create_pipeline(&render_pass, &surface, None);
        let ubo_pool = vulkan.create_view_ubo_pool();
        return Self { instance, device, physical, queue, render_pass, framebuffers, pipeline, surface, swapchain, images, ubo_pool, vulkan, start: Instant::now() };
    }

    fn run(&mut self) -> () {
        
    }
    
    fn render(&mut self) {

    }

    fn destroy(&mut self) {}
}

fn main() {
    pretty_env_logger::init();

    // App
    let event_loop = EventLoop::new();
    let mut app = App::create(&event_loop);

    // TODO: move away
    let (vertex_buffer, index_buffer) = app.vulkan.load_model();
    let (texture, sampler) = app.vulkan.load_image();
    // Setup texture descriptor set
    let layout_texture = app.pipeline.layout().set_layouts().get(1).unwrap();
    let descriptor_set_texture = PersistentDescriptorSet::new(
        &app.vulkan.descriptor_set_allocator,
        layout_texture.clone(),
        [WriteDescriptorSet::image_view_sampler(0, texture.clone(), sampler.clone())]
    ).unwrap();

    let frames_in_flight = app.images.len();
    let mut fences: Vec<Option<Arc<FenceSignalFuture<_>>>> = vec![None; frames_in_flight];
    let mut previous_fence_i = 0;

    let mut destroying = false;
    let mut window_resized = false;
    let mut recreate_swapchain = false;

    // look into this when rendering https://www.reddit.com/r/vulkan/comments/e7n5b6/drawing_multiple_objects/
    event_loop.run(move |event, _, control_flow| {
        //*control_flow = ControlFlow::Poll;
        match event {
            // Render a frame if app not being destroyed
            Event::MainEventsCleared if !destroying => {
                if window_resized || recreate_swapchain {
                    recreate_swapchain = false;

                    let new_dimensions = app.surface.object().unwrap().downcast_ref::<Window>().unwrap().inner_size();

                    // ignore rendering if one of the dimensions is 0
                    if new_dimensions.height == 0 || new_dimensions.width == 0 {
                        return
                    }

                    let (new_swapchain, new_images) = match app.swapchain.recreate(SwapchainCreateInfo {
                        image_extent: new_dimensions.into(),
                        ..app.swapchain.create_info()
                    }) {
                        Ok(r) => r,
                        // Apparently the creation can fail if the user keeps resizing
                        // In that case we can just try to recreate again on the next frame
                        Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => return,
                        // Happens when minimized
                        Err(SwapchainCreationError::ImageExtentZeroLengthDimensions { .. }) => return,
                        Err(e) => panic!("Failed to recreate swapcahin: {:?}", e),
                    };
                    app.swapchain = new_swapchain;
                    let new_framebuffers = app.vulkan.create_framebuffers(
                        &app.render_pass,
                        &new_images
                    );

                    if window_resized {
                        window_resized = false;

                        let viewport = Viewport {
                            origin: [0.0, 0.0],
                            dimensions: new_dimensions.into(),
                            depth_range: 0.0..1.0,
                        };

                        let new_pipeline = app.vulkan.create_pipeline(&app.render_pass, &app.surface, Some(&viewport));
                        app.images = new_images;
                        app.pipeline = new_pipeline;
                        app.framebuffers = new_framebuffers;
                    }
                }

                let (image_i, suboptimal, acquire_future) =
                    match acquire_next_image(app.swapchain.clone(), None) {
                        Ok(r) => (usize::try_from(r.0).unwrap(), r.1, r.2),
                        Err(AcquireError::OutOfDate) => {
                            recreate_swapchain = true;
                            return;
                        }
                        Err(e) => panic!("Failed to acquire next image: {:?}", e),
                    };
                
                if suboptimal {
                    recreate_swapchain = true;
                }

                // Set up ubo data, currently just the MVP matrices
                let time = app.start.elapsed().as_secs_f32();
                let model = nalgebra_glm::rotate(
                    &nalgebra_glm::identity(), 
                    time * nalgebra_glm::radians(&nalgebra_glm::vec1(90.0))[0], 
                    &nalgebra_glm::vec3(0.0, 0.0, 1.0)
                );
                let view = nalgebra_glm::look_at(
                    &nalgebra_glm::vec3(2.0, 2.0, 2.0),
                &nalgebra_glm::vec3(0.0, 0.0, 0.0),
                    &nalgebra_glm::vec3(0.0, 0.0, 1.0),
                );
                let mut proj = nalgebra_glm::perspective(
                    app.swapchain.image_extent()[0] as f32 / app.swapchain.image_extent()[1] as f32,
                    nalgebra_glm::radians(&nalgebra_glm::vec1(45.0))[0],
                    0.1,
                    10.0,
                );
                // convert from OpenGL to Vulkan coordinates
                proj[(1, 1)] *= -1.0;
                let ubo_data = UniformBufferObject {
                    model: model.into(),
                    view: view.into(),
                    proj: proj.into()
                };
                let view_ubo = app.ubo_pool.from_data(ubo_data).unwrap();

                let command_buffer = app.vulkan.create_command_buffer(
                    &app.pipeline, 
                    &app.framebuffers[image_i], 
                    &vertex_buffer, 
                    &index_buffer, 
                    &view_ubo,
                    &descriptor_set_texture,
                );

                if let Some(image_fence) = &fences[image_i] {
                    image_fence.wait(None).unwrap();
                }

                let previous_future = match fences[previous_fence_i].clone() {
                    None => {
                        let mut now = sync::now(app.device.clone());
                        now.cleanup_finished();

                        now.boxed()
                    }

                    Some(fence) => fence.boxed(),
                };

                let future = previous_future
                    .join(acquire_future)
                    .then_execute(app.queue.clone(), command_buffer.clone())
                    .unwrap()
                    .then_swapchain_present(
                        app.queue.clone(),
                        SwapchainPresentInfo::swapchain_image_index(app.swapchain.clone(), image_i.try_into().unwrap())
                    )
                    .then_signal_fence_and_flush();

                fences[image_i] = match future {
                    Ok(value) => Some(Arc::new(value)),
                    Err(FlushError::OutOfDate) => {
                        recreate_swapchain = true;
                        None
                    }
                    Err(e) => {
                        println!("Failed to flush future: {:?}", e);
                        None
                    }
                };

                previous_fence_i = image_i;
            },
            // Resize
            Event::WindowEvent { event: WindowEvent::Resized(_), .. } => {
                window_resized = true;
            },
            // Destroy the app
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                destroying = true;
                *control_flow = ControlFlow::Exit;
                { app.destroy(); }
            },
            _ => {}
        }
    });
}
