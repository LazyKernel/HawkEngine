use serde::{Deserialize, Serialize};
use specs::Entity;
use std::{collections::HashMap, net::SocketAddr, time::Instant};
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::network::tokio::Client;

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Unknown,
    ConnectionRequest,
    ConnectionAccept,
    ConnectionKeepAlive,
    NewClient,
    NewReplicated,
    ComponentTransform,
    ComponentCustom(String),
    ChatMessage,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum NetworkProtocol {
    TCP,
    UDP,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct NetworkPacketOut {
    pub net_id: Uuid,
    pub message_type: MessageType,
    pub protocol: NetworkProtocol,
    pub data: Vec<u8>,
}

impl Default for NetworkPacketOut {
    fn default() -> Self {
        NetworkPacketOut {
            net_id: Uuid::nil(),
            message_type: MessageType::Unknown,
            protocol: NetworkProtocol::TCP,
            data: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPacketIn {
    pub client: Client,
    pub message_type: MessageType,
    pub protocol: NetworkProtocol,
    pub data: Vec<u8>,
}

pub struct Player {
    pub client_id: Uuid,
    pub last_keep_alive: Instant,
}

pub struct NetworkData {
    pub is_server: bool,
    pub sender: mpsc::Sender<NetworkPacketOut>,
    // used to generate receivers for network packets
    pub in_packets_sender: broadcast::Sender<NetworkPacketIn>,
    pub target_addr: SocketAddr,
    pub local_addr: SocketAddr,
    pub net_id_ent: HashMap<Uuid, Entity>,
    pub player_list: HashMap<Uuid, Player>,
    pub player_self: Option<Player>,
    pub server_last_keep_alive: Instant,
    pub client_connection_tried_last: Instant,
}
