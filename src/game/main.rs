use engine::{
    ecs::{
        components::{
            general::{Camera, Movement, Transform},
            physics::{ColliderComponent, ColliderRenderable, RigidBodyComponent},
        },
        resources::{physics::PhysicsData, ActiveCamera},
        utils::objects::create_terrain,
    },
    start_engine, HawkEngine,
;
use log::error;
use nalgebra::Vector3;
use rapier3d::{
    control::{CharacterLength, KinematicCharacterController},
    prelude::{ColliderBuilder, RigidBodyBuilder, RigidBodyType, SharedShape},
};
use specs::{Builder, WorldExt};
use winit::event_loop::EventLoop;

fn init(engine: &mut HawkEngine) {
    let world = &mut engine.ecs.world;

    let renderer = match &engine.renderer {
        Some(x) => x,
        None => panic!("Renderer wasn't set when calling init"),
    };

    // Add physics stuff
    let mut physics_data = PhysicsData::default();

    let character_controller = KinematicCharacterController {
        offset: CharacterLength::Relative(0.1),
        snap_to_ground: Some(CharacterLength::Relative(0.025)),
        ..Default::default()
    };
    let rigid_body = RigidBodyBuilder::new(RigidBodyType::KinematicPositionBased)
        //.ccd_enabled(true)
        .can_sleep(false)
        .enabled(true)
        .user_data(1)
        .translation(Vector3::new(0.0, 15.0, 0.0))
        .lock_rotations()
        .build();
    let collider = ColliderBuilder::new(SharedShape::capsule_y(0.9, 1.0))
        //.active_collision_types(ActiveCollisionTypes::default() | ActiveCollisionTypes::KINEMATIC_FIXED)
        //.friction(0.7)
        //.mass(80.0)
        .enabled(true)
        .build();

    let rigid_body_component =
        RigidBodyComponent::new(rigid_body, &mut physics_data, Some(character_controller));

    let collider = ColliderComponent::new(
        collider,
        Some(&rigid_body_component.handle),
        &mut physics_data,
    );
    let (v, i) = collider.get_vertices(&physics_data);
    let vert = ColliderRenderable::convert_to_vertex(v);
    let (vb, ib) = renderer.vulkan.create_vertex_buffers(vert, i);

    // Add a camera
    let camera_entity = world
        .create_entity()
        .with(Camera)
        .with(Transform {
            pos: Vector3::new(0.0, 15.0, 0.0),
            ..Default::default()
        })
        .with(Movement {
            speed: 10.0,
            boost: 20.0,
            slow: 5.0,
            jump: 3000.0,
            sensitivity: 0.1,
            max_jumps: 2,
            ..Default::default()
        })
        .with(collider)
        //.with(ColliderRenderable { vertex_buffer: (*vb).clone(), index_buffer: (*ib).clone() })
        .with(rigid_body_component)
        .build();
    world.insert(ActiveCamera(camera_entity));

    // Add a terrain
    let (terrain_renderable, terrain_rigid_body, terrain_collider) =
        create_terrain("terrain", "grass", &renderer.vulkan);

    match terrain_renderable {
        Ok(v) => {
            let terrain_rb_comp =
                RigidBodyComponent::new(terrain_rigid_body, &mut physics_data, None);

            let collider = ColliderComponent::new(
                terrain_collider,
                Some(&terrain_rb_comp.handle),
                &mut physics_data,
            );
            let a = 0;
            let (ve, i) = collider.get_vertices(&physics_data);
            let vert = ColliderRenderable::convert_to_vertex(ve);
            let (vb, ib) = renderer.vulkan.create_vertex_buffers(vert, i);

            let terrain = world
                .create_entity()
                .with(v)
                .with(Transform::default())
                .with(collider)
                .with(terrain_rb_comp)
                .with(ColliderRenderable {
                    vertex_buffer: (*vb).clone(),
                    index_buffer: (*ib).clone(),
                })
                .build();
            world.insert(terrain);
        }
        Err(e) => error!("An error occurred while trying to create terrain: {:?}", e),
    };

    // Inserting this last so the components can borrow it
    world.insert(physics_data);

    for i in 0..2 {
        let renderable = renderer
            .vulkan
            .create_renderable("viking_room", Some("default".into()));

        match renderable {
            Ok(v) => {
                let obj = world
                    .create_entity()
                    .with(v)
                    .with(Transform {
                        pos: Vector3::new(0.0, i as f32 * 1.0, -1.0),
                        ..Transform::default()
                    })
                    .build();
                world.insert(obj);
            }
            Err(e) => println!("Failed creating viking_room renderable: {:?}", e),
        }
    }
}

#[derive(FromArgs)]
struct GameArgs {
    #[argh(switch, short = 's')]
    server: bool,
    #[argh(option)]
    host: Option<String>,
    #[argh(option)]
    port: Option<u16>,
}

fn main() {
    let args: GameArgs = argh::from_env();
    let event_loop = EventLoop::new().expect("Could not create event loop");
    let mut engine = HawkEngine::new(true);

    engine.add_post_init_fn(init);

    if server {
        engine.start_networking(
            args.host.unwrap_or("0.0.0.0".into()).into(),
            args.port.unwrap_or(6742),
            true,
        );
    } else if args.host.is_some() && args.port.is_some() {
        engine.start_networking(
            args.host
                .expect("we just checked that args.host is something")
                .into(),
            args.port.expect("we just checked args.port is something"),
            false,
        );
    }

    start_engine(engine, event_loop);
}
