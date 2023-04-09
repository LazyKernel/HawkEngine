use rapier3d::prelude::IntegrationParameters;
use specs::{System, Write, Read, ReadStorage, WriteStorage};

use crate::ecs::{resources::{physics::PhysicsData, DeltaTime}, components::{general::Transform, physics::{RigidBodyComponent, ColliderComponent}}};


pub struct Physics;

impl<'a> System<'a> for Physics {
    type SystemData = (
        Write<'a, PhysicsData>,
        Read<'a, DeltaTime>,

        WriteStorage<'a, Transform>,
        WriteStorage<'a, RigidBodyComponent>,
        ReadStorage<'a, ColliderComponent>
    );

    fn run(&mut self, (mut physics_data, delta_time, mut transform, mut rigid_body, collider): Self::SystemData) {
        use specs::Join;

        // Update entities
        for (t, r, c) in (&mut transform, &mut rigid_body, &collider).join() {
            if t.need_physics_update {
                r.apply_movement(Some(&t.pos), Some(&t.rot), delta_time.0, c, &mut physics_data);
            }
        }

        // Run physics step
        let (
            gravity, 
            integration_params,
            physics_pipeline,
            island_manager,
            broad_phase,
            narrow_phase,
            rigid_body_set,
            collider_set,
            impulse_joint_set,
            multibody_joint_set,
            ccd_solver
        ) = physics_data.split_borrow();

        // Using non-fixed time step
        let new_integration_params = IntegrationParameters {
            dt: delta_time.0,
            ..*integration_params
        };

        physics_pipeline.step(
            &gravity,
            &new_integration_params,
            island_manager,
            broad_phase,
            narrow_phase,
            rigid_body_set,
            collider_set,
            impulse_joint_set,
            multibody_joint_set,
            ccd_solver,
            None,
            &(),
            &()
        )
    }
} 
