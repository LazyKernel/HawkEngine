use std::{net::SocketAddr, collections::HashMap};
use serde::{Serialize, Deserialize};
use specs::Entity;
use tokio::sync::mpsc::{Sender, Receiver};
use uuid::Uuid;


pub struct NetworkMessageData {
    pub addr: SocketAddr,
    pub packet: NetworkPacket
}

#[derive(Serialize, Deserialize)]
pub enum MessageType {
    ComponentTransform,
    ComponentCustom(String)
}

#[derive(Serialize, Deserialize)]
pub struct NetworkPacket {
    pub net_id: Uuid,
    pub message_type: MessageType,
    pub data: Vec<u8>
}

pub struct NetworkData {
    pub sender: Sender<NetworkMessageData>,
    pub receiver: Receiver<NetworkMessageData>,
    pub target_addr: SocketAddr,
    pub net_id_ent: HashMap<Uuid, Entity>
}
