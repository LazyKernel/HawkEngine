use rapier3d::prelude::EventHandler;


#[derive(Default)]
pub struct DebugEventHandler;

impl EventHandler for DebugEventHandler {
    fn handle_collision_event(
        &self,
        bodies: &rapier3d::prelude::RigidBodySet,
        colliders: &rapier3d::prelude::ColliderSet,
        event: rapier3d::prelude::CollisionEvent,
        contact_pair: Option<&rapier3d::prelude::ContactPair>,
    ) {
        println!("Collision event:");
        println!("started?: {:?}, stopped?: {:?}, removed?: {:?}", event.started(), event.stopped(), event.removed());
    }

    fn handle_contact_force_event(
        &self,
        dt: rapier3d::prelude::Real,
        bodies: &rapier3d::prelude::RigidBodySet,
        colliders: &rapier3d::prelude::ColliderSet,
        contact_pair: &rapier3d::prelude::ContactPair,
        total_force_magnitude: rapier3d::prelude::Real,
    ) {
        println!("Contact force event:");
        println!("dt: {:?}, total force magnitude: {:?}", dt, total_force_magnitude);

        //bodies.iter().for_each(|(handle, rb)| println!("", ))
    }
}
