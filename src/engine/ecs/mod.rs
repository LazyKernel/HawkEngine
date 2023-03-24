use specs::{World, WorldExt};

use crate::ecs::components::general::{Transform, Renderable};

use self::components::general::{Camera, Movement};

pub mod components;
pub mod resources;
pub mod systems;

pub struct ECS {
    pub world: World
}

impl ECS {
    pub fn new() -> Self {
        let mut world = World::new();
        ECS::register_components(&mut world);
        return Self { world }
    }

    fn register_components(world: &mut World) {
        world.register::<Transform>();
        world.register::<Renderable>();
        world.register::<Camera>();
        world.register::<Movement>();
    }
}
