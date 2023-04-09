use rapier3d::prelude::IntegrationParameters;
use specs::{System, Write, Read};

use crate::ecs::resources::{physics::PhysicsData, DeltaTime};


pub struct Physics;

impl<'a> System<'a> for Physics {
    type SystemData = (
        Write<'a, PhysicsData>,
        Read<'a, DeltaTime>
    );

    fn run(&mut self, (mut physics_data, delta_time): Self::SystemData) {
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
