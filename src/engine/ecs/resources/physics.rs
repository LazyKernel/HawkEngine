use nalgebra::Vector3;
use rapier3d::prelude::{RigidBodySet, ColliderSet, IntegrationParameters, PhysicsPipeline, IslandManager, BroadPhase, NarrowPhase, ImpulseJointSet, MultibodyJointSet, CCDSolver};


#[derive(Default)]
pub struct PhysicsData {
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,

    pub gravity: Vector3<f32>,
    pub integration_parameters: IntegrationParameters,
    pub physics_pipeline: PhysicsPipeline,
    pub island_manager: IslandManager,
    pub broad_phase: BroadPhase,
    pub narrow_phase: NarrowPhase,
    pub impulse_joint_set: ImpulseJointSet,
    pub multibody_joint_set: MultibodyJointSet,
    pub ccd_solver: CCDSolver
}

impl PhysicsData {
    pub fn split_borrow(&mut self) -> (
        &Vector3<f32>,
        &IntegrationParameters,
        &mut PhysicsPipeline,
        &mut IslandManager,
        &mut BroadPhase,
        &mut NarrowPhase,
        &mut RigidBodySet,
        &mut ColliderSet,
        &mut ImpulseJointSet,
        &mut MultibodyJointSet,
        &mut CCDSolver
    ) {
        (
            &self.gravity, 
            &self.integration_parameters,
            &mut self.physics_pipeline,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set, 
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver
        )
    }
}
