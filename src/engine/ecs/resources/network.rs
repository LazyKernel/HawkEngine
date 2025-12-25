use serde::{Deserialize, Serialize};
use specs::Entity;
use std::{collections::HashMap, net::SocketAddr, time::Instant};
use tokio::sync::mpsc::{Receiver, Sender};
use uuid::Uuid;

use crate::network::tokio::Client;

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Unknown,
    ConnectionRequest,
    ConnectionAccept,
    ConnectionKeepAlive,
    NewClient,
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
    pub addr: Option<SocketAddr>,
    pub message_type: MessageType,
    pub protocol: NetworkProtocol,
    pub data: Vec<u8>,
}

impl Default for NetworkPacket {
    fn default() -> Self {
        NetworkPacket {
            net_id: Uuid::nil(),
            addr: None,
            message_type: MessageType::Unknown,
            protocol: NetworkProtocol::TCP,
            data: vec![],
        }
    }
}

pub struct NetworkData {
    pub is_server: bool,
    pub sender: Sender<NetworkPacket>,
    pub receiver: Receiver<NetworkPacket>,
    pub target_addr: SocketAddr,
    pub local_addr: SocketAddr,
    pub net_id_ent: HashMap<Uuid, Entity>,
    pub player_list: HashMap<Uuid, Client>,
    pub player_self: Option<Client>,
    pub server_last_keep_alive: Instant,
    pub client_connection_tried_last: Instant,
}
