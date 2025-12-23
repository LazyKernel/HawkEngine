use tokio::sync::broadcast::{Receiver, Sender};

use crate::ecs::resources::network::NetworkPacket;

pub mod general;
pub mod network;
pub mod physics;
pub mod render;

// systems wanting to have access to sending and receiving
// network messages should extend NetworkMessenger
pub struct NetworkMessenger {
    // broadcast sender
    sender: Sender<NetworkPacket>,
    // broadcast receiver, please iterate through all available
    // packets every frame
    receiver: Receiver<NetworkPacket>,
}
