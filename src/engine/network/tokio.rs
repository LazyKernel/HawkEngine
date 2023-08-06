use std::{sync::mpsc::{self, Sender, Receiver}, thread, net::IpAddr};

use log::error;
use tokio::{net::{UdpSocket}, runtime::Runtime};

use crate::ecs::resources::network::{NetworkMessageData, NetworkData};

async fn tokio_network_loop(addr: IpAddr, port: u16, sender: Sender<NetworkMessageData>, receiver: Receiver<NetworkMessageData>) {
    loop {
        let socket_res = UdpSocket::bind((addr, port)).await;

        let socket = match socket_res {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to open socket to address {:?}:{:?}", addr, port);
                error!("{e}");
                break;
            }
        };

        
    }
}

pub fn start_network_thread(address: &str, port: u16) -> Option<NetworkData> {
    let (a2s_sender, a2s_receiver) = mpsc::channel::<NetworkMessageData>();
    let (s2a_sender, s2a_receiver) = mpsc::channel::<NetworkMessageData>();

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
            tokio_network_loop(addr_ok, port, a2s_sender, s2a_receiver).await;
        });
    });

    return Some(NetworkData {sender: s2a_sender, receiver: a2s_receiver});
}
