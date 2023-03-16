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
mod ecs;
mod graphics;
mod shaders;

use ecs::ECS;
use ecs::components::general::{Transform, Camera, Movement};
use ecs::resources::{ProjectionMatrix, ActiveCamera, RenderData, CommandBuffer, RenderDataFrameBuffer, CursorGrab};
use ecs::systems::general::PlayerInput;
use ecs::systems::render::Render;
use graphics::utils::get_window_from_surface;
use graphics::vulkan::Vulkan;
use shaders::vs::ty::VPUniformBufferObject;
use specs::{World, WorldExt, Builder, DispatcherBuilder};
use vulkano::buffer::CpuBufferPool;
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::device::physical::{PhysicalDevice};
use vulkano::pipeline::{GraphicsPipeline, Pipeline};
use vulkano::pipeline::graphics::viewport::{Viewport};
use vulkano::swapchain::{Swapchain, SwapchainCreateInfo, Surface, SwapchainCreationError, acquire_next_image, AcquireError, SwapchainPresentInfo};
use vulkano::sync::{self, GpuFuture, FenceSignalFuture};
use vulkano::sync::FlushError;
use winit::dpi::{LogicalPosition, PhysicalPosition};
use winit_input_helper::WinitInputHelper;

use std::sync::Arc;
use std::time::Instant;
use winit::event::{Event, WindowEvent, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window, CursorGrabMode};
use vulkano::instance::{
    Instance
};
use vulkano::device::{
    Device, 
    Queue, DeviceExtensions,
};
use vulkano::image::{SwapchainImage};
use vulkano::render_pass::{RenderPass, Framebuffer};

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
    ubo_pool: Arc<CpuBufferPool<VPUniformBufferObject>>,

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

        let mut vulkan = Vulkan::new(&device, &queue);

        let (swapchain, images) = vulkan.create_swapchain(&physical, &surface);
        let render_pass = vulkan.create_render_pass(&swapchain);
        let framebuffers= vulkan.create_framebuffers(&render_pass, &images);
        let pipeline = vulkan.create_pipeline("default", &render_pass, &surface, None);
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

    let mut input = WinitInputHelper::new();

    // App
    let event_loop = EventLoop::new();
    let mut app = App::create(&event_loop);

    let frames_in_flight = app.images.len();
    let mut fences: Vec<Option<Arc<FenceSignalFuture<_>>>> = vec![None; frames_in_flight];
    let mut previous_fence_i = 0;

    let mut destroying = false;
    let mut recreate_swapchain = false;
    let mut cursor_grabbed = false;

    let mut proj = nalgebra_glm::perspective(
        app.swapchain.image_extent()[0] as f32 / app.swapchain.image_extent()[1] as f32,
        nalgebra_glm::radians(&nalgebra_glm::vec1(45.0))[0],
        0.1,
        1000.0,
    );
    // convert from OpenGL to Vulkan coordinates
    proj[(1, 1)] *= -1.0;

    // Create ECS classes
    let mut ecs = ECS::new();
    let mut dispatcher = DispatcherBuilder::new()
        // Using thread_local for player input for a couple of reasons
        // 1. it's probably a good idea to have the camera view be updated 
        //    in a single thread while there are no other updates going on
        //    which have a chance of using its value
        // 2. the whole program hangs when trying to set cursor grab on windows
        //    if the operation happens from another thread
        .with_thread_local(PlayerInput)
        .with_thread_local(Render)
        .build();
        

    // TODO: move elsewhere
    let renderable = app.vulkan.create_renderable("viking_room", Some("default".into()));

    match renderable {
        Ok(v) => {
            ecs.world
                .create_entity()
                .with(v)
                .with(Transform::default())
                .build();
        }
        Err(e) => println!("Failed creating viking_room renderable: {:?}", e)
    }
    
    // Add initial input
    ecs.world.insert(Arc::new(input.clone()));
    // Add initial surface
    ecs.world.insert(app.surface.clone());
    // Add initial cursor grab
    ecs.world.insert(CursorGrab { 0: false });
    // Add projection matrix
    ecs.world.insert(ProjectionMatrix(proj));
    // Add a camera
    let camera_entity = ecs.world
        .create_entity()
        .with(Camera)
        .with(Transform::default())
        .with(Movement {speed: 0.1, sensitivity: 0.01, yaw: 0.0, pitch: 0.0})
        .build();
    ecs.world.insert(ActiveCamera(camera_entity));
    // Add initial render data
    ecs.world.insert(RenderData {
        pipeline: app.pipeline.clone(),
        ubo_pool: app.ubo_pool.clone(),
        command_buffer_allocator: app.vulkan.command_buffer_allocator.clone(),
        descriptor_set_allocator: app.vulkan.descriptor_set_allocator.clone(),
        queue_family_index: app.vulkan.queue.queue_family_index()
    });
    ecs.world.insert(RenderDataFrameBuffer(app.framebuffers[0].clone()));
    // Add empty command buffer
    ecs.world.insert(CommandBuffer { command_buffer: None });

    // look into this when rendering https://www.reddit.com/r/vulkan/comments/e7n5b6/drawing_multiple_objects/
    event_loop.run(move |event, _, control_flow| {
        // Render a frame if app not being destroyed
        if input.update(&event) && !destroying {
            if input.quit() {
                destroying = true;
                *control_flow = ControlFlow::Exit;
                { app.destroy(); }
            }

            if input.window_resized().is_some() || recreate_swapchain {
                recreate_swapchain = false;

                let new_dimensions = input.window_resized().expect("Window resized was none");

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

                if input.window_resized().is_some() {

                    let viewport = Viewport {
                        origin: [0.0, 0.0],
                        dimensions: new_dimensions.into(),
                        depth_range: 0.0..1.0,
                    };

                    let new_pipeline = app.vulkan.create_pipeline(
                        "default", 
                        &app.render_pass, 
                        &app.surface, 
                        Some(&viewport)
                    );
                    app.images = new_images;
                    app.pipeline = new_pipeline;
                    app.framebuffers = new_framebuffers;

                    // Recreate projection matrix
                    let mut proj = nalgebra_glm::perspective(
                        app.swapchain.image_extent()[0] as f32 / app.swapchain.image_extent()[1] as f32,
                        nalgebra_glm::radians(&nalgebra_glm::vec1(45.0))[0],
                        0.1,
                        1000.0,
                    );
                    // convert from OpenGL to Vulkan coordinates
                    proj[(1, 1)] *= -1.0;

                    let mut projection_mat = ecs.world.write_resource::<ProjectionMatrix>();
                    *projection_mat = ProjectionMatrix(proj);
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

            // Own scope for immutable reference
            {
                // Update render data
                let mut framebuffer = ecs.world.write_resource::<RenderDataFrameBuffer>();
                *framebuffer = RenderDataFrameBuffer(app.framebuffers[image_i].clone());
                
                let mut input_res = ecs.world.write_resource::<Arc<WinitInputHelper>>();
                *input_res = Arc::new(input.clone());
            }

            dispatcher.dispatch(&ecs.world);
            ecs.world.maintain();

            let command_buffer = ecs.world.read_resource::<CommandBuffer>();
            let command_buffer = match &command_buffer.command_buffer {
                Some(v) => v,
                None => return eprintln!("Command buffer received from ECS was none, skipping rendering for this frame")
            };

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
        }
    });
}
