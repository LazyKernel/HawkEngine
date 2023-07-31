use std::sync::Arc;
use log::warn;
use nalgebra::{Matrix4, Vector3, UnitQuaternion};
use specs::{Component, VecStorage, HashMapStorage, NullStorage};
use vulkano::{buffer::CpuAccessibleBuffer, descriptor_set::PersistentDescriptorSet};

use crate::data_structures::graphics::Vertex;


#[derive(Component, Debug)]
#[storage(VecStorage)]
pub struct Transform {
    /*
    Coordinate system is right handed, -z forward, y up, x right
    */

    // TODO: possibility to optimize
    // lazy generating transformation matrix
    // for example: if the object doesnt move,
    // we still recreate the transformation matrix anew
    // on every render

    pub pos: Vector3<f32>,
    pub rot: UnitQuaternion<f32>,
    pub scale: Vector3<f32>,

    pub mov: Vector3<f32>,
    pub vel: Vector3<f32>,
    pub accel: Vector3<f32>,

    pub need_physics_update: bool
}

impl Transform {
    pub fn transformation_matrix(&self) -> Matrix4<f32> {
        let translate = Matrix4::new_translation(&self.pos);
        let rotation = &self.rot.to_homogeneous();
        let scale = Matrix4::new_nonuniform_scaling(&self.scale);
        
        translate * rotation * scale
    }

    pub fn forward(&self) -> Vector3<f32> {
        self.rot * Vector3::new(0.0, 0.0, -1.0)
    }

    pub fn up(&self) -> Vector3<f32> {
        self.rot * Vector3::new(0.0, 1.0, 0.0)
    }

    pub fn right(&self) -> Vector3<f32> {
        self.rot * Vector3::new(1.0, 0.0, 0.0)
    }

    pub fn apply_movement(&mut self, movement: &Vector3<f32>) {
        self.mov += movement;
        self.need_physics_update = true;
    }

    pub fn apply_velocity(&mut self, vel: &Vector3<f32>) {
        self.vel += vel;
        self.need_physics_update = true;
    }

    pub fn apply_acceleration(&mut self, accel: &Vector3<f32>) {
        self.accel += accel;
        self.need_physics_update = true;
    }
}

impl Default for Transform {
    fn default() -> Self {
        let default_vec = Vector3::default();
        let default_quat = UnitQuaternion::identity();
        let default_scale = Vector3::new(1.0, 1.0, 1.0);
        Transform { pos: default_vec, mov: default_vec, vel: default_vec, accel: default_vec, rot: default_quat, scale: default_scale, need_physics_update: true }
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

#[derive(Component, Default)]
#[storage(NullStorage)]
pub struct Wireframe;

#[derive(Component, Debug)]
#[storage(HashMapStorage)]
pub struct Camera;

#[derive(Component, Debug, Default)]
#[storage(HashMapStorage)]
pub struct Movement {
    pub speed: f32,
    pub boost: f32,
    pub slow: f32,
    pub jump: f32,
    pub sensitivity: f32,

    pub yaw: f32,
    pub pitch: f32,

    pub last_x: f32,
    pub last_y: f32,

    // number of consecutive jumps allowed
    // e.g. 2 allows jumping once while in air
    pub max_jumps: u8,
    // the number of jumps we have left
    // TODO: ideally this would be private, but rust doesnt allow 
    // default with private members in a reasonable way as of yet
    pub num_jumps_remaining: u8
}

impl Movement {
    pub fn can_jump(&self, grounded: bool) -> bool {
        if grounded && self.max_jumps > 0 {
            return true;
        }
        else if !grounded && self.num_jumps_remaining > 0 {
            return true;
        }
        else {
            return false;
        }
    }

    pub fn consume_jump(&mut self, grounded: bool) {
        if grounded {
            self.num_jumps_remaining = self.max_jumps;
        }

        if self.num_jumps_remaining == 0 {
            warn!("Tried to consume jump, but no jumps remaining!");
            return;
        }

        self.num_jumps_remaining -= 1;
    }
}