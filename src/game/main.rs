use engine::{HawkEngine, start_engine, ecs::{components::general::{Transform, Camera, Movement}, resources::ActiveCamera}};
use nalgebra_glm::vec3;
use specs::{WorldExt, Builder};

fn main() {
    let mut engine = HawkEngine::new();

    let world = &mut engine.ecs.world;

    // Add a camera
    let camera_entity = world
        .create_entity()
        .with(Camera)
        .with(Transform::default())
        .with(Movement {speed: 0.1, sensitivity: 0.1, yaw: 0.0, pitch: 0.0, last_x: 0.0, last_y: 0.0})
        .build();
    world.insert(ActiveCamera(camera_entity));

    for i in 0..2 {
        let renderable = engine.vulkan.create_renderable("viking_room", Some("default".into()));

        match renderable {
            Ok(v) => {
                world
                    .create_entity()
                    .with(v)
                    .with(Transform {
                        pos: vec3(0.0, 0.0, i as f32 * 1.0),
                        ..Transform::default()
                    })
                    .build();
            }
            Err(e) => println!("Failed creating viking_room renderable: {:?}", e)
        }
    }

    start_engine(engine);
}