#![deny(
    nonstandard_style,
    warnings,
    rust_2018_idioms,
    unused,
    future_incompatible,
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo
)]

use std::sync::Arc;
use anyhow::{anyhow, Result};
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window, WindowBuilder};
use vulkano::VulkanLibrary;
use vulkano::instance::{
    Instance, 
    InstanceCreateInfo,
    Version
};

#[derive(Clone, Debug)]
struct App {
    instance: Arc<Instance>
}

impl App {
    unsafe fn create(_window: &Window) -> Result<Self> {
        let instance = App::create_instance()?;
        Ok(Self { instance })
    }

    unsafe fn create_instance() -> Result<Arc<Instance>> {
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
            .map_err(|b| anyhow!("{}", b))?;

        return Ok(res);
    }

    unsafe fn render(&mut self, _window: &Window) -> Result<()> {
        Ok(())
    }

    unsafe fn destroy(&mut self) {}
}

#[derive(Clone, Debug, Default)]
struct AppData;

fn main() -> Result<()> {
    pretty_env_logger::init();

    // Window
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("HawkEngine")
        .with_inner_size(LogicalSize::new(1024, 768))
        .build(&event_loop)?;

    // App
    let mut app = unsafe {App::create(&window)?};
    let mut destroying = false;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            // Render a frame if app not being destroyed
            Event::MainEventsCleared if !destroying =>
                unsafe { app.render(&window) }.unwrap(),
            // Destroy the app
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                destroying = true;
                *control_flow = ControlFlow::Exit;
                unsafe { app.destroy(); }
            }
            _ => {}
        }
    })
}
