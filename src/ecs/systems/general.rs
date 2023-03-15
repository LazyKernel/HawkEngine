use std::sync::Arc;

use nalgebra_glm::clamp_scalar;
use specs::{System, Read, ReadStorage, WriteStorage, Write};
use vulkano::swapchain::Surface;
use winit::{event::VirtualKeyCode, window::{Window, CursorGrabMode}, dpi::PhysicalPosition};
use winit_input_helper::WinitInputHelper;

use crate::{ecs::{components::general::{Camera, Transform, Movement}, resources::CursorGrab}, graphics::utils::get_window_from_surface};

pub struct PlayerInput;

impl<'a> System<'a> for PlayerInput {
    type SystemData = (
        Option<Read<'a, Arc<WinitInputHelper>>>,
        Option<Read<'a, Arc<Surface>>>,
        Write<'a, CursorGrab>,
        ReadStorage<'a, Camera>,
        WriteStorage<'a, Movement>,
        WriteStorage<'a, Transform>,
    );

    fn run(&mut self, (input, surface, mut cursor_grabbed, camera, mut movement, mut transform): Self::SystemData) {
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

        let surface = match surface {
            Some(v) => v,
            None => {
                eprintln!("Input helper was none");
                return
            }
        };

        let window = match get_window_from_surface(&surface) {
            Some(v) => v,
            None => return eprintln!("Could not get window in PlayerInput")
        };

        if input.mouse_pressed(0) {
            let result = window.set_cursor_grab(CursorGrabMode::Confined)
                .or_else(|_e| window.set_cursor_grab(CursorGrabMode::Locked));

            match result {
                Ok(_) => (),
                Err(e) => println!("Failed to grab cursor, probably not a problem: {:?}", e)
            }

            window.set_cursor_visible(false);
            cursor_grabbed.0 = true;
        }

        if input.key_pressed(VirtualKeyCode::Escape) {
            let result = window.set_cursor_grab(CursorGrabMode::None);

            match result {
                Ok(_) => (),
                Err(e) => println!("Failed to ungrab cursor, this is weird: {:?}", e)
            }

            window.set_cursor_visible(true);
            cursor_grabbed.0 = false;
        }

        if cursor_grabbed.0 {
            let size = window.inner_size();
            let result = window.set_cursor_position(PhysicalPosition { x: size.width / 2, y: size.height / 2 });
            match result {
                Ok(_) => (),
                Err(e) => println!("Failed to set cursor position, not available on some platforms: {:?}", e)
            }
        }
        else {
            return
        }

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
