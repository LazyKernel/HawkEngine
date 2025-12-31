use tokio::sync::broadcast::{Receiver, Sender};

use crate::ecs::resources::network::{NetworkPacketIn, NetworkPacketOut};

pub mod general;
pub mod network;
pub mod physics;
pub mod render;
