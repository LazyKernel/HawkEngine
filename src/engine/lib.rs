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
mod network;
mod physics;
mod shaders;

pub use graphics::renderer::Renderer;
pub use graphics::window::WindowState;

use bitflags::bitflags;
use ecs::systems::general::PlayerInput;
use ecs::systems::physics::Physics;
use ecs::systems::render::Render;
use ecs::ECS;
use log::{trace, warn};
use specs::{Dispatcher, DispatcherBuilder, WorldExt};
use winit::event_loop::EventLoop;

use crate::{
    ecs::systems::network::{
        connection_handler::ConnectionHandler, generic_replicated_handler::GenericHandler,
        player_handler::PlayerHandler,
    },
    network::tokio::start_network_thread,
};

pub type PostInitFn = fn(&mut HawkEngine<'_>);

bitflags! {
    pub struct EngineFeatures: u8 {
        const PHYSICS = 1;
        const NETWORK = 1 << 1;
    }
}

pub struct HawkEngine<'a> {
    pub renderer: Option<Renderer>,

    pub ecs: ECS,
    dispatchers: Vec<Dispatcher<'a, 'a>>,

    post_init_functions: Vec<PostInitFn>,
}

impl<'a> HawkEngine<'a> {
    /*
    If use_physics is true, PhysicsData is expected to be provided as a resource
    If use_networking is true, expecting start_network_thread to be called at some point
    */
    pub fn new(enabled_features: EngineFeatures) -> Self {
        match pretty_env_logger::try_init() {
            Ok(_) => {}
            Err(e) => trace!(
                "Failed to init pretty_env_logger, probably already initialized: {:?}",
                e
            ),
        }

        // Create ECS classes
        let ecs = ECS::new();

        let mut dbuilder = DispatcherBuilder::new();

        if enabled_features.contains(EngineFeatures::PHYSICS) {
            dbuilder.add(Physics::default(), "physics", &[]);
        }

        if enabled_features.contains(EngineFeatures::NETWORK) {
            dbuilder.add(
                GenericHandler::default(),
                "replicated_handler",
                &["physics"],
            );
            dbuilder.add(
                PlayerHandler::default(),
                "player_handler",
                &["replicated_handler"],
            );
            dbuilder.add(ConnectionHandler::default(), "connection_handler", &[]);
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

        return Self {
            renderer: None,
            ecs,
            dispatchers,
            post_init_functions: vec![],
        };
    }

    pub fn add_dispatcher(&mut self, dispatcher: Dispatcher<'a, 'a>) {
        self.dispatchers.push(dispatcher);
    }

    pub fn add_post_init_fn(&mut self, func: PostInitFn) {
        self.post_init_functions.push(func);
    }

    pub fn set_renderer(&mut self, renderer: Renderer) {
        renderer.setup_engine(self);
        self.renderer = Some(renderer);
        self.post_init_functions
            .clone()
            .into_iter()
            .for_each(|x| x(self));

        // setup dispatchers
        for dispatcher in self.dispatchers.iter_mut() {
            dispatcher.setup(&mut self.ecs.world);
        }
    }

    pub fn start_networking(&mut self, address: &str, port: u16, server: bool) {
        match start_network_thread(address, port, server) {
            Some(netdata) => self.ecs.world.insert(netdata),
            None => warn!("Network data received from start_network_thread was None"),
        }
    }
}

pub fn start_engine(engine: HawkEngine<'static>, event_loop: EventLoop<()>) {
    // look into this when rendering https://www.reddit.com/r/vulkan/comments/e7n5b6/drawing_multiple_objects/
    let mut window_state = WindowState::new();
    window_state.run(event_loop, engine);
}
