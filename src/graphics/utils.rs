use std::sync::Arc;

use vulkano::swapchain::Surface;
use winit::window::Window;

pub fn get_window_from_surface(surface: &Arc<Surface>) -> Option<&Window> {
    match surface.object() {
        Some(v) => v.downcast_ref::<Window>(),
        None => {
            println!("Failed to get surface object");
            return None
        }
    }
}
