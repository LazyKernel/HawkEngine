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
pub mod ecs;
mod graphics;
mod physics;
mod network;
mod shaders;

use ecs::ECS;
use ecs::resources::{ProjectionMatrix, RenderData, CommandBuffer, RenderDataFrameBuffer, CursorGrab, DeltaTime};
use ecs::systems::general::PlayerInput;
use ecs::systems::physics::Physics;
use ecs::systems::render::Render;
use graphics::vulkan::Vulkan;
use log::{info, trace};
use nalgebra::Perspective3;
use specs::{WorldExt, DispatcherBuilder, Dispatcher};
use vulkano::buffer::Buffer;
use vulkano::pipeline::graphics::rasterization::{RasterizationState, PolygonMode};
use vulkano::pipeline::{GraphicsPipeline};
use vulkano::pipeline::graphics::viewport::{Viewport};
use vulkano::swapchain::{Swapchain, SwapchainCreateInfo, Surface, acquire_next_image, SwapchainPresentInfo};
use vulkano::sync::future::FenceSignalFuture;
use vulkano::sync::{self, GpuFuture};
use vulkano::VulkanError;
use winit_input_helper::WinitInputHelper;

use std::sync::Arc;
use std::time::Instant;
use winit::event_loop::EventLoop;
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

pub struct HawkEngine<'a> {
    device: Arc<Device>,
    queue: Arc<Queue>,
    render_pass: Arc<RenderPass>,
    framebuffers: Vec<Arc<Framebuffer>>,
    pipeline: Arc<GraphicsPipeline>,
    pipeline_wireframe: Arc<GraphicsPipeline>,
    surface: Arc<Surface>,
    swapchain: Arc<Swapchain>,
    images: Vec<Arc<Image>>,
    ubo_pool: Arc<Buffer>,

    pub vulkan: Vulkan,

    pub ecs: ECS,
    dispatchers: Vec<Dispatcher<'a,'a>>,
    event_loop: EventLoop<()>
}

impl<'a> HawkEngine<'a> {
    /*
    If use_physics is true, PhysicsData is expected to be provided as a resource
    */
    pub fn new(use_physics: bool) -> Self {
        match pretty_env_logger::try_init() {
            Ok(_) => {},
            Err(e) => trace!("Failed to init pretty_env_logger, probably already initialized: {:?}", e)
        }

        // Create ECS classes
        let ecs = ECS::new();

        let mut dbuilder = DispatcherBuilder::new();

        if use_physics {
            dbuilder.add(Physics::default(), "physics", &[]);
        }

        let dispatcher = dbuilder
            // Using thread_local for player input for a couple of reasons
            // 1. it's probably a good idea to have the camera view be updated 
            //    in a single thread while there are no other updates going on
            //    which have a chance of using its value
            // 2. the whole program hangs when trying to set cursor grab on windows
            //    if the operation happens from another thread
            //    (this works on macos probably because macos is really particular about
            //     threading for UI operations and the winit team has taken this into
            //     account probably for macos only)
            .with_thread_local(PlayerInput)
            .with_thread_local(Render)
            .build();
        let dispatchers = vec![dispatcher];

        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::empty()
        };

        let event_loop = EventLoop::new().expect("Could not create event loop");
        let instance = Vulkan::create_instance(&event_loop, ENABLE_VALIDATION_LAYERS);
        let surface = Vulkan::create_surface(&instance, &event_loop);
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
        return Self { device, queue, render_pass, framebuffers, pipeline, pipeline_wireframe, surface, swapchain, images, ubo_pool, vulkan, ecs, dispatchers, event_loop };
    }

    pub fn add_dispatcher(&mut self, dispatcher: Dispatcher<'a, 'a>) {
        self.dispatchers.push(dispatcher);
    }
}


pub fn start_engine(mut engine: HawkEngine<'static>) {
    let mut input = WinitInputHelper::new();

    let frames_in_flight = engine.images.len();
    let mut fences: Vec<Option<Arc<FenceSignalFuture<_>>>> = vec![None; frames_in_flight];
    let mut previous_fence_i = 0;

    let mut destroying = false;
    let mut recreate_swapchain = false;

    let mut proj = Perspective3::new(
        engine.swapchain.image_extent()[0] as f32 / engine.swapchain.image_extent()[1] as f32,
        (45.0 as f32).to_radians(),
        0.1,
        1000.0,
    ).to_homogeneous();
    // convert from OpenGL to Vulkan coordinates
    proj[(1, 1)] *= -1.0;
    
    // Add initial input
    engine.ecs.world.insert(Arc::new(input.clone()));
    // Add initial surface
    engine.ecs.world.insert(engine.surface.clone());
    // Add initial cursor grab
    engine.ecs.world.insert(CursorGrab::default());
    // Add projection matrix
    engine.ecs.world.insert(ProjectionMatrix(proj));
    // Add initial render data
    engine.ecs.world.insert(RenderData {
        pipeline: engine.pipeline.clone(),
        pipeline_wireframe: engine.pipeline_wireframe.clone(),
        ubo_pool: engine.ubo_pool.clone(),
        buffer_allocator: engine.vulkan.buffer_memory_allocator.clone(),
        command_buffer_allocator: engine.vulkan.command_buffer_allocator.clone(),
        descriptor_set_allocator: engine.vulkan.descriptor_set_allocator.clone(),
        queue_family_index: engine.vulkan.queue.queue_family_index()
    });
    engine.ecs.world.insert(RenderDataFrameBuffer(engine.framebuffers[0].clone()));
    // Add empty command buffer
    engine.ecs.world.insert(CommandBuffer { command_buffer: None });
    // Add 0 delta time
    engine.ecs.world.insert(DeltaTime(0.0));

    let mut last_time = Instant::now();

    // look into this when rendering https://www.reddit.com/r/vulkan/comments/e7n5b6/drawing_multiple_objects/
    let _ = engine.event_loop.run(move |event, window_target| {
        // Render a frame if app not being destroyed
        if input.update(&event) && !destroying {
            if input.destroyed() {
                destroying = true;
                window_target.exit();
            }

            if input.window_resized().is_some() || recreate_swapchain {
                recreate_swapchain = false;

                let new_dimensions = input.window_resized().expect("Window resized was none");

                // ignore rendering if one of the dimensions is 0
                if new_dimensions.height == 0 || new_dimensions.width == 0 {
                    return
                }

                let (new_swapchain, new_images) = match engine.swapchain.recreate(SwapchainCreateInfo {
                    image_extent: new_dimensions.into(),
                    ..engine.swapchain.create_info()
                }) {
                    Ok(r) => r,
                    // Apparently the creation can fail if the user keeps resizing
                    // In that case we can just try to recreate again on the next frame
                    //Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => return,
                    // Happens when minimized
                    //Err(SwapchainCreationError::ImageExtentZeroLengthDimensions { .. }) => return,
                    Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
                };
                engine.swapchain = new_swapchain;
                let new_framebuffers = engine.vulkan.create_framebuffers(
                    &engine.render_pass,
                    &new_images
                );

                if input.window_resized().is_some() {

                    let viewport = Viewport {
                        offset: [0.0, 0.0],
                        extent: new_dimensions.into(),
                        depth_range: 0.0..=1.0,
                    };

                    // TODO: do not load these again every time
                    let vs = shaders::default::vs::load(engine.device.clone()).expect("Failed to create vs");
                    let fs = shaders::default::fs::load(engine.device.clone()).expect("Failed to load fs");
                    // Wireframe
                    let vsw = shaders::wireframe::vs::load(engine.device.clone()).expect("Failed to load wireframe vs");
                    let fsw = shaders::wireframe::fs::load(engine.device.clone()).expect("Failed to load wireframe fs");
                    let new_pipeline = engine.vulkan.create_pipeline(
                        "default", 
                        &engine.render_pass, 
                        &engine.surface, 
                        &vs,
                        &fs,
                        Some(&viewport),
                        None
                    );
                    let rasterization_state = RasterizationState { polygon_mode: PolygonMode::Line, ..Default::default() };
                    let new_pipeline_wireframe = engine.vulkan.create_pipeline(
                        "wireframe", 
                        &engine.render_pass, 
                        &engine.surface, 
                        &vsw,
                        &fsw,
                        Some(&viewport),
                        Some(&rasterization_state)
                    );

                    // TODO: shouldn't we update renderdata in ecs here???
                    engine.images = new_images;
                    engine.pipeline = new_pipeline;
                    engine.pipeline_wireframe = new_pipeline_wireframe;
                    engine.framebuffers = new_framebuffers;

                    // Recreate projection matrix
                    let mut proj = Perspective3::new(
                        engine.swapchain.image_extent()[0] as f32 / engine.swapchain.image_extent()[1] as f32,
                        (45.0 as f32).to_radians(),
                        0.1,
                        1000.0,
                    ).to_homogeneous();
                    // convert from OpenGL to Vulkan coordinates
                    proj[(1, 1)] *= -1.0;

                    let mut projection_mat = engine.ecs.world.write_resource::<ProjectionMatrix>();
                    *projection_mat = ProjectionMatrix(proj);
                }
            }

            let (image_i, suboptimal, acquire_future) =
                match acquire_next_image(engine.swapchain.clone(), None) {
                    Ok(r) => (usize::try_from(r.0).unwrap(), r.1, r.2),
                    /*Err(AcquireError::OutOfDate) => {
                        recreate_swapchain = true;
                        return;
                    }*/
                    Err(e) => panic!("Failed to acquire next image: {:?}", e),
                };
            
            if suboptimal {
                recreate_swapchain = true;
            }

            // Own scope for immutable reference
            {
                // Update render data
                let mut framebuffer = engine.ecs.world.write_resource::<RenderDataFrameBuffer>();
                *framebuffer = RenderDataFrameBuffer(engine.framebuffers[image_i].clone());
                
                let mut input_res = engine.ecs.world.write_resource::<Arc<WinitInputHelper>>();
                *input_res = Arc::new(input.clone());

                // Update delta time
                let delta = Instant::now() - last_time;
                let mut deltatime_resource = engine.ecs.world.write_resource::<DeltaTime>();
                *deltatime_resource = DeltaTime(delta.as_secs_f32());
                last_time = Instant::now();
            }

            // Iterate through all dispatchers, with the internal being last
            for dispatcher in engine.dispatchers.iter_mut().rev() {
                dispatcher.dispatch(&engine.ecs.world);
            }
            engine.ecs.world.maintain();

            let command_buffer = engine.ecs.world.read_resource::<CommandBuffer>();
            let command_buffer = match &command_buffer.command_buffer {
                Some(v) => v,
                None => return eprintln!("Command buffer received from ECS was none, skipping rendering for this frame")
            };

            if let Some(image_fence) = &fences[image_i] {
                image_fence.wait(None).unwrap();
            }

            let previous_future = match fences[previous_fence_i].clone() {
                None => {
                    let mut now = sync::now(engine.device.clone());
                    now.cleanup_finished();

                    now.boxed()
                }

                Some(fence) => fence.boxed(),
            };

            let future = previous_future
                .join(acquire_future)
                .then_execute(engine.queue.clone(), command_buffer.clone())
                .unwrap()
                .then_swapchain_present(
                    engine.queue.clone(),
                    SwapchainPresentInfo::swapchain_image_index(engine.swapchain.clone(), image_i.try_into().unwrap())
                )
                .then_signal_fence_and_flush();

            fences[image_i] = match future {
                Ok(value) => Some(Arc::new(value)),
                /*Err(FlushError::OutOfDate) => {
                    recreate_swapchain = true;
                    None
                }*/
                Err(e) => {
                    info!("Failed to flush future: {:?}", e);
                    None
                }
            };

            previous_fence_i = image_i;
        }
    });
}
