use std::{collections::HashMap, net::IpAddr, sync::Arc, thread, time::Duration};

use log::error;
use tokio::{sync::mpsc::{self, Sender, Receiver}, net::{UdpSocket}, runtime::Runtime, time::timeout};

use crate::ecs::resources::network::{NetworkMessageData, NetworkData, NetworkPacket};

const UDP_BUF_SIZE: usize = 1432;

async fn server_loop(socket: UdpSocket, sender: Sender<NetworkMessageData>, mut receiver: Receiver<NetworkMessageData>) {
    let r = Arc::new(socket);
    let s = r.clone();
    let recv_task = tokio::spawn(async move {
        let mut buf = [0u8; UDP_BUF_SIZE];
        loop {
            // TODO: handle errors
            let (len, addr) = r.recv_from(&mut buf).await.unwrap();
            
            /*match sender.send(NetworkMessageData {addr, data: buf[..len].to_vec()}).await {
                Ok(_) => {},
                Err(e) => error!("Failed to send a network message from async to sync: {e}")
            }*/
        }
    });

    let send_task = tokio::spawn(async move {
        loop {
            let message = match receiver.recv().await {
                Some(v) => v,
                None => {
                    error!("Failed to receive message data on async from sync");
                    continue;
                }
            };
            
            /*match s.send_to(&message.data.into_boxed_slice(), message.addr).await {
                Ok(_) => {},
                Err(e) => {
                    error!("Failed to send data to address {:?}: {}", message.addr, e);
                }
            }*/
        }
    });

    send_task.await.unwrap_or_else(|e| error!("Failed to join send_task: {e}"));
    recv_task.await.unwrap_or_else(|e| error!("Failed to join recv_task: {e}"));
}

async fn client_loop(socket: UdpSocket, addr: IpAddr, port: u16, sender: Sender<NetworkMessageData>, mut receiver: Receiver<NetworkMessageData>) {
    socket.connect((addr, port)).await.unwrap();
    let r = Arc::new(socket);
    let s = r.clone();

    
    let recv_task = tokio::spawn(async move {
        let mut buf = [0u8; UDP_BUF_SIZE];
        loop {
            // TODO: handle errors
            let len = r.recv(&mut buf).await.unwrap();
            let network_message = rmp_serde::from_slice::<NetworkPacket>(&buf[..len]).unwrap();
            
            match sender.send(NetworkMessageData {addr: (addr, port).into(), packet: network_message}).await {
                Ok(_) => {},
                Err(e) => error!("Failed to send a network message from async to sync: {e}")
            } 
        }
    });

    let send_task = tokio::spawn(async move {
        loop {
            let message = match receiver.recv().await {
                Some(v) => v,
                None => {
                    error!("Failed to receive message data on async from sync");
                    continue;
                }
            };
            
            /*match s.send(&message.data.into_boxed_slice()).await {
                Ok(_) => {},
                Err(e) => {
                    error!("Failed to send data to address {:?}:{:?}: {}", addr, port, e);
                }
            }*/
        }
    });

    send_task.await.unwrap_or_else(|e| error!("Failed to join send_task: {e}"));
    recv_task.await.unwrap_or_else(|e| error!("Failed to join recv_task: {e}"));
}

/// If server is true, will use many-to-one style connection
/// otherwise connects to the specific address
async fn tokio_network_loop(addr: IpAddr, port: u16, server: bool, sender: Sender<NetworkMessageData>, receiver: Receiver<NetworkMessageData>) {
    // udp might never connect which would block this thread forever
    // thus using a 10s timeout
    let socket_res = timeout(Duration::from_secs(10), UdpSocket::bind(("0.0.0.0", port))).await;

    let socket = match socket_res {
        Ok(s) => match s {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to open socket to address {:?}:{:?}", addr, port);
                error!("{e}");
                return;
            }
        }
        Err(e) => {
            error!("Failed to connect socket to address {:?}:{:?}: timeout elapsed", addr, port);
            error!("{e}");
            return;
        }
    };

    if server {
        server_loop(socket, sender, receiver).await;
    }
    else {
        client_loop(socket, addr, port, sender, receiver).await;
    }
}

pub fn start_network_thread(address: &str, port: u16, server: bool) -> Option<NetworkData> {
    let (a2s_sender, a2s_receiver) = mpsc::channel::<NetworkMessageData>(16384);
    let (s2a_sender, s2a_receiver) = mpsc::channel::<NetworkMessageData>(16384);

    let addr_parsed= address.parse::<IpAddr>();
    
    let addr_ok = match addr_parsed {
        Ok(v) => v,
        Err(e) => {
            error!("failed to parse {:?} into a valid ip address!", address);
            error!("{e}");
            return None;
        }
    }; 

    thread::spawn(move || {
        let rt_res= Runtime::new();

        let rt = match rt_res {
            Ok(v) => v,
            Err(e) => {
                error!("Failed creating tokio runtime.");
                return;
            }
        }; 

        rt.block_on(async move {
            tokio_network_loop(addr_ok, port, server, a2s_sender, s2a_receiver).await;
        });
    });

    return Some(NetworkData {sender: s2a_sender, receiver: a2s_receiver, target_addr: (addr_ok, port).into(), net_id_ent: HashMap::new()});
}
