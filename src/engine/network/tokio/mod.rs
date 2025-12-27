mod client;
mod server;

use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4},
    thread,
    time::{Duration, Instant},
};

use log::error;
use serde::{Deserialize, Serialize};
use tokio::{
    runtime::Runtime,
    sync::mpsc::{self, Receiver, Sender},
};
use uuid::Uuid;

use crate::{
    ecs::resources::network::{MessageType, NetworkData, NetworkPacketIn, NetworkPacketOut},
    network::tokio::{client::client_loop, server::server_loop},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawNetworkMessagePacket {
    message_type: MessageType,
    payload: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct RawNetworkMessage {
    addr: SocketAddr,
    packet: RawNetworkMessagePacket,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Client {
    pub client_id: Uuid,
    pub addr: SocketAddr,
}

impl Default for Client {
    fn default() -> Self {
        Client {
            client_id: Uuid::nil(),
            addr: SocketAddrV4::new(0.into(), 0).into(),
        }
    }
}

/// If server is true, will use many-to-one style connection
/// otherwise connects to the specific address
async fn tokio_network_loop(
    addr: IpAddr,
    port: u16,
    server: bool,
    sender: Sender<NetworkPacketIn>,
    receiver: Receiver<NetworkPacketOut>,
) {
    if server {
        server_loop(addr, port, sender, receiver).await;
    } else {
        client_loop(addr, port, sender, receiver).await;
    }
}

pub fn start_network_thread(address: &str, port: u16, server: bool) -> Option<NetworkData> {
    let (a2s_sender, a2s_receiver) = mpsc::channel::<NetworkPacketIn>(16384);
    let (s2a_sender, s2a_receiver) = mpsc::channel::<NetworkPacketOut>(16384);

    let addr_parsed = address.parse::<IpAddr>();

    let addr_ok = match addr_parsed {
        Ok(v) => v,
        Err(e) => {
            error!("failed to parse {:?} into a valid ip address!", address);
            error!("{e}");
            return None;
        }
    };

    thread::spawn(move || {
        let rt_res = Runtime::new();

        let rt = match rt_res {
            Ok(v) => v,
            Err(e) => {
                error!("Failed creating tokio runtime.\n{:?}", e);
                return;
            }
        };

        rt.block_on(async move {
            tokio_network_loop(addr_ok, port, server, a2s_sender, s2a_receiver).await;
        });
    });

    return Some(NetworkData {
        sender: s2a_sender,
        receiver: a2s_receiver,
        target_addr: (addr_ok, port).into(),
        net_id_ent: HashMap::new(),
        is_server: server,
        player_list: HashMap::new(),
        player_self: None,
        server_last_keep_alive: Instant::now(),
        client_connection_tried_last: Instant::now() - Duration::from_secs(10),
        local_addr: (Ipv4Addr::new(127, 0, 0, 1), port).into(),
    });
}
