use std::sync::Arc;

use nalgebra_glm::clamp_scalar;
use specs::{System, Read, ReadStorage, WriteStorage};
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

use crate::ecs::components::general::{Camera, Transform, Movement};

pub struct PlayerInput;

impl<'a> System<'a> for PlayerInput {
    type SystemData = (
        Option<Read<'a, Arc<WinitInputHelper>>>,
        ReadStorage<'a, Camera>,
        WriteStorage<'a, Movement>,
        WriteStorage<'a, Transform>,
    );

    fn run(&mut self, (input, camera, mut movement, mut transform): Self::SystemData) {
        use specs::Join;
        // Verify we have all dependencies
        // Abort if not
        let input = match input {
            Some(v) => v,
            None => {
                eprintln!("Input helper was none");
                return
            }
        };

        for (_, m, t) in (&camera, &mut movement, &mut transform).join() {
            let mouse_diff = input.mouse_diff();
            if mouse_diff != (0.0, 0.0) {
                let (dx, dy) = mouse_diff;

                m.yaw -= dx;
                m.pitch -= dy;

                let qx = nalgebra_glm::quat_rotate(
                    &nalgebra_glm::Quat::identity(), 
                    m.yaw * m.sensitivity, 
                    &nalgebra_glm::vec3(0.0, 0.0, 1.0)
                );
                let right = nalgebra_glm::quat_rotate_vec3(
                    &qx.normalize(), 
                    &nalgebra_glm::vec3(1.0, 0.0, 0.0)
                );
                let qy = nalgebra_glm::quat_rotate(
                    &nalgebra_glm::Quat::identity(), 
                    m.pitch * m.sensitivity, 
                    &right
                );

                t.rot = qy * qx;
            }

            let mut cum_move = nalgebra_glm::vec3(0.0, 0.0, 0.0);
            if input.key_held(VirtualKeyCode::W) {
                cum_move -= t.forward() * m.speed;
            }
            if input.key_held(VirtualKeyCode::S) {
                cum_move += t.forward() * m.speed;
            }
            if input.key_held(VirtualKeyCode::A) {
                cum_move -= t.right() * m.speed;
            }
            if input.key_held(VirtualKeyCode::D) {
                cum_move += t.right() * m.speed;
            }

            t.pos += cum_move;
        }
    }
}
