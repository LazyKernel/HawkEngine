use serde::{Deserialize, Serialize};
use specs::Entity;
use std::{collections::HashMap, net::SocketAddr};
use tokio::sync::mpsc::{Receiver, Sender};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Unknown,
    ConnectionRequest,
    ConnectionAccept,
    ConnectionKeepAlive,
    ComponentTransform,
    ComponentCustom(String),
    ChatMessage,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum NetworkProtocol {
    TCP,
    UDP,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct NetworkPacket {
    pub net_id: Uuid,
    pub message_type: MessageType,
    pub protocol: NetworkProtocol,
    pub data: Vec<u8>,
}

pub struct NetworkData {
    pub is_server: bool,
    pub sender: Sender<NetworkPacket>,
    pub receiver: Receiver<NetworkPacket>,
    pub target_addr: SocketAddr,
    pub net_id_ent: HashMap<Uuid, Entity>,
}
