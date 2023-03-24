use specs::{System, Write};

use crate::ecs::resources::physics::PhysicsData;


pub struct Physics;

impl<'a> System<'a> for Physics {
    type SystemData = (
        Write<'a, PhysicsData>,
    );

    fn run(&mut self, (mut physics_data,): Self::SystemData) {
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

        physics_pipeline.step(
            &gravity,
            &integration_params,
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
