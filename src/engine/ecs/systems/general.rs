use std::sync::Arc;

use log::{error, debug, warn};
use nalgebra::{clamp, UnitQuaternion, Vector3};
use rapier3d::prelude::RigidBody;
use specs::{System, Read, ReadStorage, WriteStorage, Write};
use vulkano::swapchain::Surface;
use winit::{event::VirtualKeyCode, window::{CursorGrabMode}, dpi::PhysicalPosition};
use winit_input_helper::WinitInputHelper;

use crate::{ecs::{components::{general::{Camera, Transform, Movement}, physics::{RigidBodyComponent, ColliderComponent}}, resources::{CursorGrab, physics::PhysicsData, DeltaTime}}, graphics::utils::get_window_from_surface};

pub struct PlayerInput;

impl<'a> System<'a> for PlayerInput {
    type SystemData = (
        Read<'a, DeltaTime>,
        Option<Read<'a, Arc<WinitInputHelper>>>,
        Option<Read<'a, Arc<Surface>>>,
        Write<'a, CursorGrab>,
        ReadStorage<'a, Camera>,
        ReadStorage<'a, RigidBodyComponent>,
        WriteStorage<'a, Movement>,
        WriteStorage<'a, Transform>,
    );

    fn run(&mut self, (delta, input, surface, mut cursor_grabbed, camera, rigid_body, mut movement, mut transform): Self::SystemData) {
        use specs::Join;
        // Verify we have all dependencies
        // Abort if not
        let input = match input {
            Some(v) => v,
            None => {
                error!("Input helper was none");
                return
            }
        };

        let surface = match surface {
            Some(v) => v,
            None => {
                error!("Input helper was none");
                return
            }
        };

        let window = match get_window_from_surface(&surface) {
            Some(v) => v,
            None => return error!("Could not get window in PlayerInput")
        };

        let last_x: Option<f32>;
        let last_y: Option<f32>;
        if input.mouse_pressed(0) {
            let result = window.set_cursor_grab(CursorGrabMode::Confined)
                .or_else(|_e| window.set_cursor_grab(CursorGrabMode::Locked));

            match result {
                Ok(_) => (),
                Err(e) => debug!("Failed to grab cursor, probably not a problem: {:?}", e)
            }

            window.set_cursor_visible(false);
            cursor_grabbed.0 = true;
        }

        if input.key_pressed(VirtualKeyCode::Escape) {
            let result = window.set_cursor_grab(CursorGrabMode::None);

            match result {
                Ok(_) => (),
                Err(e) => debug!("Failed to ungrab cursor, this is weird: {:?}", e)
            }

            window.set_cursor_visible(true);
            cursor_grabbed.0 = false;
        }

        if cursor_grabbed.0 {
            let size = window.inner_size();

            last_x = Some((size.width / 2) as f32);
            last_y = Some((size.height / 2) as f32);

            let result = window.set_cursor_position(PhysicalPosition { x: size.width / 2, y: size.height / 2 });
            match result {
                Ok(_) => (),
                Err(e) => debug!("Failed to set cursor position, not available on some platforms: {:?}", e)
            };
        }
        else {
            return
        }

        // the mouse should never be outside but taking it into account still
        let (x, y) = match input.mouse() {
            Some(v) => v,
            None => (0.0, 0.0)
        };

        for (_, r, m, t) in (&camera, &rigid_body, &mut movement, &mut transform).join() {
            if !r.has_character_controller() {
                error!("Entity has movement but rigid body component does not have a character controller. Movement will not be applied!");
                continue;
            }

            t.rot = match self.calculate_rotation(x, y, last_x, last_y, m) {
                Some(v) => v,
                None => t.rot
            };

            if m.can_jump(r.grounded) && input.key_pressed(VirtualKeyCode::Space) {
                let jump_accel = Vector3::y() * m.jump;
                t.apply_acceleration(&jump_accel);
                m.consume_jump(r.grounded)
            }

            t.apply_movement(&self.calculate_movement(&input, &t.rot, m, delta.0));
        }
    }
}

impl PlayerInput {
    fn calculate_rotation(&self, x: f32, y: f32, last_x: Option<f32>, last_y: Option<f32>, m: &mut Movement) -> Option<UnitQuaternion<f32>> {
        let (last_x, last_y) = match (last_x, last_y) {
            (Some(x), Some(y)) => (x, y),
            (_, _) => (m.last_x, m.last_y)
        };

        let mouse_diff = (last_x - x, last_y - y);

        if mouse_diff != (0.0, 0.0) {
            let (dx, dy) = mouse_diff;

            m.last_x = x;
            m.last_y = y;

            m.yaw += dx * m.sensitivity;
            m.pitch = clamp(m.pitch + dy * m.sensitivity, -89.0, 89.0);

            if m.yaw > 360.0 {
                m.yaw -= 360.0;
            }
            else if m.yaw < 0.0 {
                m.yaw += 360.0;
            }

            // roll, pitch, yaw is actually x,y,z
            Some(UnitQuaternion::from_euler_angles(
                m.pitch.to_radians(), 
                m.yaw.to_radians(),
                0.0
            ))
        }
        else {
            None
        }
    }

    fn calculate_movement(&self, input: &Arc<WinitInputHelper>, rot: &UnitQuaternion<f32>, m: &Movement, delta: f32) -> Vector3<f32> {
        let forward = rot * Vector3::new(0.0, 0.0, -1.0);
        let right = rot * Vector3::new(1.0, 0.0, 0.0);

        let mut speed = m.speed;
        if input.held_shift() {
            speed += m.boost;
        }
        else if input.held_control() {
            speed -= m.slow;
        }

        let mut cum_move = Vector3::new(0.0, 0.0, 0.0);
        if input.key_held(VirtualKeyCode::W) {
            cum_move += forward * speed;
        }
        if input.key_held(VirtualKeyCode::S) {
            cum_move -= forward * speed;
        }
        if input.key_held(VirtualKeyCode::A) {
            cum_move -= right * speed;
        }
        if input.key_held(VirtualKeyCode::D) {
            cum_move += right * speed;
        }

        return cum_move * delta;
    }
}
