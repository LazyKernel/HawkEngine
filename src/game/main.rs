use engine::{HawkEngine, start_engine, ecs::{components::{general::{Transform, Camera, Movement}, physics::{RigidBodyComponent, ColliderComponent}}, resources::{ActiveCamera, physics::PhysicsData}}};
use nalgebra::Vector3;
use rapier3d::{control::KinematicCharacterController, prelude::{RigidBodyBuilder, RigidBodyType, ColliderBuilder, SharedShape}};
use specs::{WorldExt, Builder};

fn main() {
    let mut engine = HawkEngine::new(true);

    let world = &mut engine.ecs.world;

    // Add physics stuff
    let mut physics_data = PhysicsData::default();

    let character_controller = KinematicCharacterController::default();
    let rigid_body = RigidBodyBuilder::new(RigidBodyType::KinematicPositionBased)
        .enabled(true)
        .build();
    let collider = ColliderBuilder::new(SharedShape::ball(1.0))
        .enabled(true)
        .build();

    let rigid_body_component = RigidBodyComponent::new(rigid_body, &mut physics_data, Some(character_controller));

    // Add a camera
    let camera_entity = world
        .create_entity()
        .with(Camera)
        .with(Transform::default())
        .with(Movement {speed: 0.1, sensitivity: 0.1, yaw: 0.0, pitch: 0.0, last_x: 0.0, last_y: 0.0})
        .with(ColliderComponent::new(collider, Some(&rigid_body_component.handle), &mut physics_data))
        .with(rigid_body_component)
        .build();
    world.insert(ActiveCamera(camera_entity));
    
    // Inserting this last so the components can borrow it
    world.insert(physics_data);

    for i in 0..2 {
        let renderable = engine.vulkan.create_renderable("viking_room", Some("default".into()));

        match renderable {
            Ok(v) => {
                world
                    .create_entity()
                    .with(v)
                    .with(Transform {
                        pos: Vector3::new(0.0, i as f32 * 1.0, -1.0),
                        ..Transform::default()
                    })
                    .build();
            }
            Err(e) => println!("Failed creating viking_room renderable: {:?}", e)
        }
    }

    start_engine(engine);
}