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

use graphics::vulkan;
use vulkano::device::physical::{PhysicalDevice};
use vulkano::instance::debug::{DebugUtilsMessageSeverity, DebugUtilsMessageType, DebugUtilsMessenger, DebugUtilsMessengerCreateInfo};
use vulkano::pipeline::GraphicsPipeline;
use vulkano::pipeline::graphics::viewport::{Viewport};
use vulkano::swapchain::{Swapchain, SwapchainCreateInfo, Surface, SwapchainCreationError, acquire_next_image, AcquireError, SwapchainPresentInfo};
use vulkano::sync::{self, GpuFuture, FenceSignalFuture};
use vulkano::sync::FlushError;

use std::sync::Arc;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window, self};
use vulkano::instance::{
    Instance
};
use vulkano::device::{
    Device, 
    Queue, DeviceExtensions,
};
use vulkano::command_buffer::{ PrimaryAutoCommandBuffer};
use vulkano::image::{ SwapchainImage};
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
    command_buffers: Vec<Arc<PrimaryAutoCommandBuffer>>
}

impl App {
    fn create(event_loop: &EventLoop<()>) -> Self {
        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::empty()
        };
        let instance = vulkan::create_instance(true);

        let surface = vulkan::create_surface(&instance, event_loop);

        let (physical, queue_index) = vulkan::select_physical_device(&instance, &surface, &device_extensions);
        let (device, queue) = vulkan::create_device(&physical, queue_index, &device_extensions);
        let (swapchain, images) = vulkan::create_swapchain(&device, &physical, &surface);
        let render_pass = vulkan::create_render_pass(&device, &swapchain);
        let framebuffers= vulkan::create_framebuffers(&render_pass, &images);
        let pipeline = vulkan::create_pipeline(&device, &render_pass, &surface, None);
        let command_buffers = vulkan::create_command_buffers(&device, &queue, &pipeline, &framebuffers);
        return Self { instance, device, physical, queue, render_pass, framebuffers, pipeline, surface, swapchain, images, command_buffers };
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

    let frames_in_flight = app.images.len();
    let mut fences: Vec<Option<Arc<FenceSignalFuture<_>>>> = vec![None; frames_in_flight];
    let mut previous_fence_i = 0;

    let mut destroying = false;
    let mut window_resized = false;
    let mut recreate_swapchain = false;

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
                    let new_framebuffers = vulkan::create_framebuffers(
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

                        let new_pipeline = vulkan::create_pipeline(&app.device, &app.render_pass, &app.surface, Some(&viewport));
                        app.command_buffers = vulkan::create_command_buffers(&app.device, &app.queue, &new_pipeline, &new_framebuffers);
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
                    .then_execute(app.queue.clone(), app.command_buffers[image_i].clone())
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
