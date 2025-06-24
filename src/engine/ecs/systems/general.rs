use std::sync::Arc;

use log::{error, debug, warn};
use nalgebra::{clamp, UnitQuaternion, Vector3};
use rapier3d::prelude::RigidBody;
use specs::{System, Read, ReadStorage, WriteStorage, Write};
use vulkano::swapchain::Surface;
use winit::{dpi::PhysicalPosition, event::{MouseButton}, keyboard::KeyCode, window::CursorGrabMode};
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

        let mut diff_x: Option<f32> = None;
        let mut diff_y: Option<f32> = None;
        if input.mouse_pressed(MouseButton::Left) && !cursor_grabbed.grabbed {
            let mut mode = CursorGrabMode::Confined;
            let result = window.set_cursor_grab(CursorGrabMode::Confined)
                .or_else(|_e| {
                    mode = CursorGrabMode::Locked;
                    window.set_cursor_grab(CursorGrabMode::Locked)
                });

            match result {
                Ok(_) => (),
                Err(e) => {
                    mode = CursorGrabMode::None;
                    debug!("Failed to grab cursor, probably not a problem: {:?}", e)
                }
            }

            //window.set_cursor_visible(false);
            cursor_grabbed.grabbed = true;
            cursor_grabbed.mode = mode;
        }

        if input.key_pressed(KeyCode::Escape) {
            let result = window.set_cursor_grab(CursorGrabMode::None);

            match result {
                Ok(_) => (),
                Err(e) => debug!("Failed to ungrab cursor, this is weird: {:?}", e)
            }

            window.set_cursor_visible(true);
            cursor_grabbed.grabbed = false;
            cursor_grabbed.mode = CursorGrabMode::None;
        }

        if cursor_grabbed.grabbed {
            let (dx, dy) = input.mouse_diff();
            diff_x = Some(dx);
            diff_y = Some(dy);
        }
        else {
            return
        }

        for (_, r, m, t) in (&camera, &rigid_body, &mut movement, &mut transform).join() {
            if !r.has_character_controller() {
                error!("Entity has movement but rigid body component does not have a character controller. Movement will not be applied!");
                continue;
            }

            t.rot = match self.calculate_rotation(diff_x, diff_y, m) {
                Some(v) => v,
                None => t.rot
            };

            if m.can_jump(r.grounded) && input.key_pressed(KeyCode::Space) {
                let jump_accel = Vector3::y() * m.jump;
                t.apply_acceleration(&jump_accel);
                m.consume_jump(r.grounded)
            }

            t.apply_movement(&self.calculate_movement(&input, &t.rot, m, delta.0));
        }
    }
}

impl PlayerInput {
    fn calculate_rotation(&self, diff_x: Option<f32>, diff_y: Option<f32>, m: &mut Movement) -> Option<UnitQuaternion<f32>> {
        let mouse_diff = (diff_x.unwrap_or(0.0), diff_y.unwrap_or(0.0));

        if mouse_diff != (0.0, 0.0) {
            let (dx, dy) = mouse_diff;

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
        if input.key_held(KeyCode::KeyW) {
            cum_move += forward * speed;
        }
        if input.key_held(KeyCode::KeyS) {
            cum_move -= forward * speed;
        }
        if input.key_held(KeyCode::KeyA) {
            cum_move -= right * speed;
        }
        if input.key_held(KeyCode::KeyD) {
            cum_move += right * speed;
        }

        return cum_move * delta;
    }
}
