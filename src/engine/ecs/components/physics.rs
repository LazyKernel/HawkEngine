use log::error;
use nalgebra::Matrix4;
use rapier3d::prelude::{RigidBodyHandle, RigidBody, Collider, ColliderHandle};
use specs::{Component, VecStorage};

use crate::ecs::resources::physics::PhysicsData;


#[derive(Component, Default, Debug)]
#[storage(VecStorage)]
pub struct RigidBodyComponent {
    handle: RigidBodyHandle
}

impl RigidBodyComponent {
    pub fn new(rigid_body: RigidBody, physics_data: &mut PhysicsData) -> Self {
        let handle = physics_data.rigid_body_set.insert(rigid_body);
        RigidBodyComponent { handle }
    }

    pub fn transformation_matrix(&self, physics_data: &mut PhysicsData) -> Matrix4<f32> {
        let rigid_body = physics_data.rigid_body_set.get(self.handle);

        match rigid_body {
            Some(v) => {
                let translate = Matrix4::new_translation(&v.translation());
                let rotation = &v.rotation().to_homogeneous();

                translate * rotation
            }
            None => {
                error!("Could not find entity with handle: {:?}", self.handle);
                Matrix4::identity()
            }
        }
    }
}


#[derive(Component, Default, Debug)]
#[storage(VecStorage)]
pub struct ColliderComponent {
    handle: ColliderHandle,
    parent_handle: Option<RigidBodyHandle>
}

impl ColliderComponent {
    pub fn new(collider: Collider, parent_handle: Option<RigidBodyHandle>, physics_data: &mut PhysicsData) -> Self {
        let handle = match parent_handle {
            Some(v) => physics_data.collider_set.insert_with_parent(collider, v, &mut physics_data.rigid_body_set),
            None => physics_data.collider_set.insert(collider)
        };
        ColliderComponent { handle, parent_handle }
    }
}
