use nalgebra::Matrix4;
use rapier3d::prelude::{RigidBodyHandle, RigidBody};
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

    pub fn transformation_matrix(&self, physics_data: &mut PhysicsData) -> Option<Matrix4<f32>> {
        let rigid_body = physics_data.rigid_body_set.get(self.handle);

        // match rigid_body {
        //     Some(v) => {
        //         let translate = nalgebra_glm::translate(&nalgebra_glm::identity(), &v.translation().into());
        //         let rotation = nalgebra_glm::quat_to_mat4(&self.rot);
        //         let scale = nalgebra_glm::scale(&nalgebra_glm::identity(), &self.scale);
                
        //         translate * rotation * scale
        //     }
        // }
        Some(Matrix4::identity())
    }
}
