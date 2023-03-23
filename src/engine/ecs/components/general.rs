use std::sync::Arc;
use nalgebra_glm::{Vec3, Quat};
use specs::{Component, VecStorage, HashMapStorage};
use vulkano::{buffer::CpuAccessibleBuffer, descriptor_set::PersistentDescriptorSet};

use crate::data_structures::graphics::Vertex;


#[derive(Component, Debug)]
#[storage(VecStorage)]
pub struct Transform {
    // TODO: possibility to optimize
    // lazy generating transformation matrix
    // for example: if the object doesnt move,
    // we still recreate the transformation matrix anew
    // on every render

    pub pos: Vec3,
    pub rot: Quat,
    pub scale: Vec3
}

impl Transform {
    pub fn transformation_matrix(&self) -> nalgebra_glm::Mat4 {
        let translate = nalgebra_glm::translate(&nalgebra_glm::identity(), &self.pos);
        let rotation = nalgebra_glm::quat_to_mat4(&self.rot);
        let scale = nalgebra_glm::scale(&nalgebra_glm::identity(), &self.scale);
        
        translate * rotation * scale
    }

    pub fn forward(&self) -> nalgebra_glm::Vec3 {
        nalgebra_glm::quat_rotate_vec3(&self.rot.normalize(), &nalgebra_glm::vec3(0.0, 0.0, 1.0))
    }

    pub fn up(&self) -> nalgebra_glm::Vec3 {
        nalgebra_glm::quat_rotate_vec3(&self.rot.normalize(), &nalgebra_glm::vec3(0.0, -1.0, 0.0))
    }

    pub fn right(&self) -> nalgebra_glm::Vec3 {
        nalgebra_glm::quat_rotate_vec3(&self.rot.normalize(), &nalgebra_glm::vec3(1.0, 0.0, 0.0))
    }
}

impl Default for Transform {
    fn default() -> Self {
        let default_vec = nalgebra_glm::Vec3::default();
        let default_quat = nalgebra_glm::Quat::identity();
        let default_scale = nalgebra_glm::vec3(1.0, 1.0, 1.0);
        Transform { pos: default_vec, rot: default_quat, scale: default_scale }
    }
}

#[derive(Component)]
#[storage(VecStorage)]
pub struct Renderable {
    // TODO: maybe switch to dense vec storage if we have a lot of 
    // non-rendered entities
    pub vertex_buffer: Arc<CpuAccessibleBuffer<[Vertex]>>, 
    pub index_buffer: Arc<CpuAccessibleBuffer<[u32]>>,
    pub descriptor_set_texture: Arc<PersistentDescriptorSet>
}

#[derive(Component, Debug)]
#[storage(HashMapStorage)]
pub struct Camera;

#[derive(Component, Debug)]
#[storage(HashMapStorage)]
pub struct Movement {
    pub speed: f32,
    pub sensitivity: f32,

    pub yaw: f32,
    pub pitch: f32,

    pub last_x: f32,
    pub last_y: f32
}
