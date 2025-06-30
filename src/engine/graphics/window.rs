use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use log::{info, trace, warn};
use nalgebra::Perspective3;
use specs::WorldExt;
use vulkano::command_buffer::CommandBufferExecFuture;
use vulkano::image::Image;
use vulkano::pipeline::graphics::rasterization::{PolygonMode, RasterizationState};
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::render_pass::Framebuffer;
use vulkano::swapchain::{acquire_next_image, PresentFuture, SwapchainAcquireFuture, SwapchainCreateInfo, SwapchainPresentInfo};
use vulkano::sync::{self, GpuFuture};
use vulkano::sync::future::{FenceSignalFuture, JoinFuture};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};
use crate::ecs::resources::{CommandBuffer, DeltaTime, ProjectionMatrix, RenderDataFrameBuffer};
use crate::ecs::utils::input::InputHelper;
use crate::graphics::renderer::Renderer;
use crate::{shaders, HawkEngine};

pub struct WindowState<'a> {
    pub window: Option<Arc<Window>>,
    input_helper: InputHelper,
    engine: Option<HawkEngine<'a>>,
    last_time: Instant,

    fences: Vec<Option<Arc<FenceSignalFuture<PresentFuture<CommandBufferExecFuture<JoinFuture<Box<dyn GpuFuture + 'static>, SwapchainAcquireFuture>>>>>>>,
    previous_fence_i: usize
}

impl<'a> WindowState<'a> {
    pub fn new() -> WindowState<'a> {
        Self {
            window: None,
            engine: None,
            input_helper: InputHelper::new(),
            last_time: Instant::now(),

            fences: vec![None; 0],
            previous_fence_i: 0
        }
    }

    fn renderer_postinit(&mut self) {
        let frames_in_flight = self.engine.as_ref().unwrap().renderer.as_ref().unwrap().images.len();
        self.fences = vec![None; frames_in_flight];
    }

    pub fn run(&mut self, event_loop: EventLoop<()>, engine: HawkEngine<'a>) {
        self.engine = Some(engine);
        let _ = event_loop.run_app(self);
    }

    // TODO: Move to renderer
    fn recreate_swapchain(&self, renderer: &mut Renderer) -> (Vec<Arc<Image>>, Vec<Arc<Framebuffer>>) {
        let Some(ref win) = self.window else {
            return (vec![], vec![]);
        };

        let new_dimensions = win.inner_size();

        // ignore rendering if one of the dimensions is 0
        if new_dimensions.height == 0 || new_dimensions.width == 0 {
            return (vec![], vec![]);
        }

        let (new_swapchain, new_images) = match renderer.swapchain.recreate(SwapchainCreateInfo {
            image_extent: new_dimensions.into(),
            ..renderer.swapchain.create_info()
        }) {
            Ok(r) => r,
            // Apparently the creation can fail if the user keeps resizing
            // In that case we can just try to recreate again on the next frame
            //Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => return,
            // Happens when minimized
            //Err(SwapchainCreationError::ImageExtentZeroLengthDimensions { .. }) => return,
            Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
        };
        renderer.swapchain = new_swapchain;
        let new_framebuffers = renderer.vulkan.create_framebuffers(
            &renderer.render_pass,
            &new_images
        );

        (new_images, new_framebuffers)
    }

    /*fn handle_window_resize(&mut self, &mut engine: &mut HawkEngine<'_>, renderer: &mut Renderer) {
        let (new_images, new_framebuffers) = self.recreate_swapchain(renderer);

        let Some(ref win) = self.window else {
            return;
        };

        let viewport = Viewport {
            offset: [0.0, 0.0],
            extent: win.inner_size().into(),
            depth_range: 0.0..=1.0,
        };

        // TODO: do not load these again every time
        let vs = shaders::default::vs::load(self.engine.device.clone()).expect("Failed to create vs");
        let fs = shaders::default::fs::load(self.engine.device.clone()).expect("Failed to load fs");
        // Wireframe
        let vsw = shaders::wireframe::vs::load(self.engine.device.clone()).expect("Failed to load wireframe vs");
        let fsw = shaders::wireframe::fs::load(self.engine.device.clone()).expect("Failed to load wireframe fs");
        let new_pipeline = self.engine.vulkan.create_pipeline(
            "default", 
            &self.engine.render_pass, 
            &self.engine.surface, 
            &vs,
            &fs,
            Some(&viewport),
            None
        );
        let rasterization_state = RasterizationState { polygon_mode: PolygonMode::Line, ..Default::default() };
        let new_pipeline_wireframe = self.engine.vulkan.create_pipeline(
            "wireframe", 
            &self.engine.render_pass, 
            &self.engine.surface, 
            &vsw,
            &fsw,
            Some(&viewport),
            Some(&rasterization_state)
        );

        // TODO: shouldn't we update renderdata in ecs here???
        self.engine.images = new_images;
        self.engine.pipeline = new_pipeline;
        self.engine.pipeline_wireframe = new_pipeline_wireframe;
        self.engine.framebuffers = new_framebuffers;

        // Recreate projection matrix
        let mut proj = Perspective3::new(
            self.engine.swapchain.image_extent()[0] as f32 / self.engine.swapchain.image_extent()[1] as f32,
            (45.0 as f32).to_radians(),
            0.1,
            1000.0,
        ).to_homogeneous();
        // convert from OpenGL to Vulkan coordinates
        proj[(1, 1)] *= -1.0;

        let mut projection_mat = self.engine.ecs.world.write_resource::<ProjectionMatrix>();
        *projection_mat = ProjectionMatrix(proj);
    }*/

    fn render(&mut self) {
        let engine: &mut HawkEngine<'a> = match self.engine.as_mut() {
            Some(x) => x,
            None => {
                trace!("Engine is None, cannot render");
                return;
            }
        };

        let renderer = match engine.renderer.as_mut() {
            Some(x) => x,
            None => {
                trace!("Renderer is None, cannot render");
                return;
            }
        };

        let (image_i, suboptimal, acquire_future) =
            match acquire_next_image(renderer.swapchain.clone(), None) {
                Ok(r) => (usize::try_from(r.0).unwrap(), r.1, r.2),
                /*Err(AcquireError::OutOfDate) => {
                    recreate_swapchain = true;
                    return;
                }*/
                Err(e) => panic!("Failed to acquire next image: {:?}", e),
            };
        
        if suboptimal {
            // TODO: recreate swap chain
        }


        // Own scope for immutable reference
        {
            // Update render data
            let mut framebuffer = engine.ecs.world.write_resource::<RenderDataFrameBuffer>();
            *framebuffer = RenderDataFrameBuffer(renderer.framebuffers[image_i].clone());
            
            let mut input_res = engine.ecs.world.write_resource::<InputHelper>();
            // HACK: not ideal, but the input helper shouldnt be too big
            *input_res = self.input_helper.clone();

            // Update delta time
            let delta = Instant::now() - self.last_time;
            let mut deltatime_resource = engine.ecs.world.write_resource::<DeltaTime>();
            *deltatime_resource = DeltaTime(delta.as_secs_f32());
            self.last_time = Instant::now();
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

        if let Some(image_fence) = &self.fences[image_i] {
            image_fence.wait(None).unwrap();
        }

        let previous_future = match self.fences[self.previous_fence_i].clone() {
            None => {
                let mut now = sync::now(renderer.device.clone());
                now.cleanup_finished();

                now.boxed()
            }

            Some(fence) => fence.boxed(),
        };

        let future = previous_future
            .join(acquire_future)
            .then_execute(renderer.queue.clone(), command_buffer.clone())
            .unwrap()
            .then_swapchain_present(
                renderer.queue.clone(),
                SwapchainPresentInfo::swapchain_image_index(renderer.swapchain.clone(), image_i.try_into().unwrap())
            )
            .then_signal_fence_and_flush();

        self.fences[image_i] = match future {
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

        self.previous_fence_i = image_i;
    }
}

impl ApplicationHandler for WindowState<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes()
            .with_title("HawkEngine")
            .with_inner_size(LogicalSize::new(1024, 768));
        
        let window: Arc<Window> = event_loop.create_window(window_attributes).unwrap().into();
        self.window = Some(window.clone());
        
        let renderer = Renderer::new(event_loop, window.clone());
        
        self.engine.as_mut().expect("Engine not defined when creating window").set_renderer(renderer);
        
        self.renderer_postinit();
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::Resized(physical_size) => info!("Resize requested"),
            WindowEvent::CloseRequested => info!("Close requested"),
            WindowEvent::Destroyed => info!("Window destroyed"),
            WindowEvent::Focused(_) => info!("Window focused"),
            WindowEvent::KeyboardInput { device_id: _, event, is_synthetic: _ } => self.input_helper.handle_keyboard_input(event),
            WindowEvent::ModifiersChanged(modifiers) => self.input_helper.handle_modifiers(modifiers),
            WindowEvent::CursorMoved { device_id: _, position: _ } => trace!("CursorMoved not implemented, using device event instead"),
            WindowEvent::CursorEntered { device_id: _ } => trace!("CursorEntered not implemented"),
            WindowEvent::CursorLeft { device_id: _ } => trace!("CursorLeft not implemented"),
            WindowEvent::MouseWheel { device_id: _, delta: _, phase: _ } => trace!("MouseWheel not implemented"),
            WindowEvent::MouseInput { device_id: _, state, button } => self.input_helper.handle_mouse_event(state, button),
            WindowEvent::RedrawRequested => self.render(),
            _ => warn!("Missing arm for winit event {:?}", event)
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        self.input_helper.handle_mouse_move_device(event);
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        let _ = event_loop;
    }

}
