use log::warn;
use nalgebra::Vector3;
use rapier3d::prelude::{IntegrationParameters, EventHandler};
use specs::{System, Write, Read, ReadStorage, WriteStorage};

use crate::ecs::{resources::{physics::PhysicsData, DeltaTime}, components::{general::{Transform, Movement}, physics::{RigidBodyComponent, ColliderComponent}}, utils::debug::DebugEventHandler};

#[derive(Default)]
pub struct Physics {
    debug_event_handler: DebugEventHandler
}

impl<'a> System<'a> for Physics {
    type SystemData = (
        Write<'a, PhysicsData>,
        Read<'a, DeltaTime>,

        WriteStorage<'a, Transform>,
        WriteStorage<'a, RigidBodyComponent>,
        ReadStorage<'a, ColliderComponent>,
        ReadStorage<'a, Movement>
    );

    fn run(&mut self, (mut physics_data, delta_time, mut transform, mut rigid_body, collider, movement): Self::SystemData) {
        use specs::Join;

        // Update entities
        for (t, r, c, m) in (&mut transform, &mut rigid_body, &collider, &movement).join() {
            if t.need_physics_update && r.has_character_controller() {
                match physics_data.rigid_body_set.get(r.handle) {
                    Some(v) => {
                        let mut total_vel = t.mov + v.linvel();
                        let mut total_vel_xz = total_vel.xz();
                        let total_vel_y = total_vel.y;
                        if total_vel_xz.norm() > m.max_vel {
                            total_vel_xz -= total_vel_xz.normalize() * (total_vel_xz.norm() - m.max_vel);
                        }

                        total_vel = Vector3::new(total_vel_xz.x, total_vel_y, total_vel_xz.y);
        
                        // need to add gravity manually as per docs
                        total_vel += physics_data.gravity * delta_time.0 * 2.0;

                        println!("{:?}", total_vel);

                        r.apply_movement(&total_vel, delta_time.0, c, &mut physics_data);
                        //t.mov -= t.mov.normalize() * m.deceleration;

                        t.mov.y += physics_data.gravity.y * delta_time.0 * 2.0;

                        if t.mov.y <= 0.1 {
                            t.mov.y = 0.0;
                        }

                        t.mov.x = 0.0;
                        t.mov.z = 0.0;

                        if t.mov.norm() < 0.1 {
                            t.need_physics_update = false;
                            t.mov = Vector3::zeros();
                        }
                    },
                    None => {
                        warn!("Failed to fetch rigid body with handle {:?}. Movement will not be updated.", r.handle);
                    }
                }
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
            ccd_solver,
            query_pipeline
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
            Some(query_pipeline),
            &(),
            &self.debug_event_handler
        );

        // Update transform
        // TODO: fix mismatch
        for (t, r) in (&mut transform, &rigid_body).join() {
            let pos = r.position(&physics_data);
            
            match physics_data.rigid_body_set.get(r.handle) {
                Some(v) => {
                    if !v.is_translation_locked() {
                        t.pos = pos.translation.vector;
                    }

                    // if any degrees of freedom are locked, don't update rotation
                    // todo: update properly
                    if !v.is_rotation_locked().iter().any(|x| *x) {
                        t.rot = pos.rotation;
                    }
                },
                None => {
                    warn!("Failed to fetch rigid body with handle {:?}. Translation and rotation will not be updated.", r.handle);
                }
            }
        }
    }
} 
