use std::sync::Arc;

use log::error;
use vulkano::swapchain::Surface;
use winit::window::Window;

pub fn get_window_from_surface(surface: &Arc<Surface>) -> Option<&Window> {
    match surface.object() {
        Some(v) => v.downcast_ref::<Window>(),
        None => {
            error!("Failed to get surface object");
            return None
        }
    }
}
