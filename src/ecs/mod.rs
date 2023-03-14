use specs::{World, WorldExt, Dispatcher, DispatcherBuilder};

use crate::ecs::components::general::{Transform, Renderable};

use self::components::general::{Camera, Movement};

pub mod components;
pub mod resources;
pub mod systems;

pub struct ECS<'a> {
    pub world: World,
    pub dispatcher: Dispatcher<'a, 'a>
}

impl ECS<'_> {
    pub fn new() -> Self {
        let mut world = World::new();
        ECS::register_components(&mut world);
        let dispatcher = DispatcherBuilder::new().build();
        return Self { world, dispatcher }
    }

    fn register_components(world: &mut World) {
        world.register::<Transform>();
        world.register::<Renderable>();
        world.register::<Camera>();
        world.register::<Movement>();
    }
}
