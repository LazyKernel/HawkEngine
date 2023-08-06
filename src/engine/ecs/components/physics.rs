use std::{sync::Arc, ops::Mul};

use log::{error, warn};
use nalgebra::{Matrix4, Vector3, Quaternion, Isometry, UnitQuaternion, Point3};
use rapier3d::{prelude::{RigidBodyHandle, RigidBody, Collider, ColliderHandle, QueryFilter, Real, ShapeType}, control::KinematicCharacterController};
use specs::{Component, VecStorage, HashMapStorage};
use vulkano::buffer::CpuAccessibleBuffer;

use crate::{ecs::resources::physics::PhysicsData, data_structures::graphics::Vertex};


#[derive(Component, Default, Debug)]
#[storage(VecStorage)]
pub struct RigidBodyComponent {
    pub handle: RigidBodyHandle,
    pub grounded: bool,
    ccontrol: Option<KinematicCharacterController>
}

impl RigidBodyComponent {
    pub fn new(rigid_body: RigidBody, physics_data: &mut PhysicsData, character_controller: Option<KinematicCharacterController>) -> Self {
        if character_controller.is_some() && !rigid_body.is_kinematic() {
            warn!("KinematicCharacterController is set but rigid body is not set to kinematic, this rigid body will not move!");
        }

        let handle = physics_data.rigid_body_set.insert(rigid_body);
        RigidBodyComponent { handle, grounded: false, ccontrol: character_controller }
    }

    pub fn transformation_matrix(&self, physics_data: &PhysicsData) -> Matrix4<f32> {
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

    pub fn position(&self, physics_data: &PhysicsData) -> Isometry<f32, nalgebra::Unit<Quaternion<f32>>, 3> {
        let rigid_body = physics_data.rigid_body_set.get(self.handle);

        match rigid_body {
            Some(v) => {
                *v.position()
            }
            None => {
                error!("Could not find entity with handle: {:?}", self.handle);
                Isometry::default()
            }
        }
    }

    pub fn has_character_controller(&self) -> bool {
        self.ccontrol.is_some()
    }

    /*
    Applies movement if this component has a KinematicCharacterController
    */
    pub fn apply_movement(&self, movement: &Vector3<f32>, velocity: &Vector3<f32>, acceleration: &Vector3<f32>, rotation: Option<&UnitQuaternion<Real>>, dt: f32, collider: &ColliderComponent, physics_data: &mut PhysicsData) -> Option<bool> {
        let cc = match self.ccontrol {
            Some(v) => v,
            None => {
                error!("Tried to apply movement to a RigidBodyComponent which has no KinematicCharacterController");
                return None;
            }
        };

        let collider = match physics_data.collider_set.get(collider.handle) {
            Some(v) => v,
            None => {
                error!("Could not find collider with handle {:?}", collider.handle);
                return None;
            }
        };

        let mut position = self.position(physics_data);
        let accel_gravity = acceleration + physics_data.gravity * 5.0;
        let desired_translation = movement + velocity * dt + 0.5 * accel_gravity * dt * dt;

        let corrected_movement = cc.move_shape(
            dt, 
            &physics_data.rigid_body_set, 
            &physics_data.collider_set,
            &physics_data.query_pipeline, 
            collider.shape(), 
            &position, 
            desired_translation, 
            QueryFilter::default().exclude_rigid_body(self.handle), 
            |_| {}
        );
        
        position.append_translation_mut(&corrected_movement.translation.into());

        match rotation {
            Some(v) => position.append_rotation_wrt_center_mut(v),
            None => ()
        }
        
        match physics_data.rigid_body_set.get_mut(self.handle) {
            Some(v) => v.set_next_kinematic_position(position),
            None => error!("Was unable to get rigid body with handle {:?}", self.handle)
        }

        return Some(corrected_movement.grounded);
    }
}


#[derive(Component, Default, Debug)]
#[storage(VecStorage)]
pub struct ColliderComponent {
    handle: ColliderHandle
}

impl ColliderComponent {
    pub fn new(collider: Collider, parent_handle: Option<&RigidBodyHandle>, physics_data: &mut PhysicsData) -> Self {
        let handle = match parent_handle {
            Some(v) => physics_data.collider_set.insert_with_parent(collider, *v, &mut physics_data.rigid_body_set),
            None => physics_data.collider_set.insert(collider)
        };
        ColliderComponent { handle }
    }

    pub fn get_vertices(&self, physics_data: &PhysicsData) -> (Vec<Point3<Real>>, Vec<u32>) {
        let collider = match physics_data.collider_set.get(self.handle) {
            Some(v) => v,
            None => {
                error!("Failed to get collider with handle {:?}", self.handle);
                return (Vec::<Point3<Real>>::new(), Vec::<u32>::new())
            }
        };

        const SUBDIV: u32 = 8;
        let (points, indices) = match collider.shape().shape_type() {
            ShapeType::Ball => collider.shape().as_ball().unwrap().to_trimesh(SUBDIV, SUBDIV),
            ShapeType::Capsule => collider.shape().as_capsule().unwrap().to_trimesh(SUBDIV, SUBDIV),
            ShapeType::Cuboid => collider.shape().as_cuboid().unwrap().to_trimesh(),
            ShapeType::HeightField => collider.shape().as_heightfield().unwrap().to_trimesh(),
            s => {
                warn!("No mapping for ShapeType {:?}", s);
                (Vec::<Point3<Real>>::new(), Vec::<[u32; 3]>::new())
            }
        };

        (points, indices.iter().flatten().map(|v| *v).collect())
    }
}

#[derive(Component)]
#[storage(HashMapStorage)]
pub struct ColliderRenderable {
    pub vertex_buffer: Arc<CpuAccessibleBuffer<[Vertex]>>, 
    pub index_buffer: Arc<CpuAccessibleBuffer<[u32]>>
}

impl ColliderRenderable {
    pub fn convert_to_vertex(vertices: Vec<Point3<Real>>) -> Vec<Vertex> {
        vertices
            .iter()
            .map(|v| {
                Vertex {
                    position: v.coords.into(),
                    normal: [0.0, 0.0, 0.0],
                    color: [1.0, 0.0, 0.0],
                    tex_coord: [0.0, 0.0]
                }
            })
            .collect()
    }
}
