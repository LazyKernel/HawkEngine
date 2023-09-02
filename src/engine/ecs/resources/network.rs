use std::{net::SocketAddr, collections::HashMap};
use specs::Entity;
use tokio::sync::mpsc::{Sender, Receiver};
use uuid::Uuid;


pub struct NetworkMessageData {
    pub addr: SocketAddr,
    pub net_id: Uuid,
    pub data: Vec<u8>
}

pub struct NetworkData {
    pub sender: Sender<NetworkMessageData>,
    pub receiver: Receiver<NetworkMessageData>,
    pub target_addr: SocketAddr,
    pub net_id_ent: HashMap<Uuid, Entity>
}
