use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Instant;
use std::{net::SocketAddr, sync::Arc};

use log::{error, info, trace, warn};
use uuid::Uuid;

use crate::network::tokio::{Client, NetworkMessageType};
use crate::network::{constants::UDP_BUF_SIZE, tokio::NetworkMessagePacket};
use crate::{
    ecs::resources::network::{NetworkData, NetworkMessageData, NetworkPacket},
    network::tokio::NetworkMessage,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{
        tcp::{self, OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpStream, UdpSocket,
    },
    sync::{
        broadcast::{self, Receiver},
        futures,
        mpsc::{self, Sender},
        RwLock,
    },
};

async fn client_send_task(
    mut tx_socket: OwnedWriteHalf,
    mut game_to_tokio_receiver: mpsc::Receiver<NetworkMessage>,
) {
    loop {
        if let Some(data) = game_to_tokio_receiver.recv().await {
            trace!("Writing data: {:?}", data);
            match rmp_serde::to_vec(&data.packet) {
                Ok(v) => {
                    if let Err(e) = tx_socket.write_all(v.as_slice()).await {
                        error!("Could not write to socket: {:?}", e);
                    }
                }
                Err(e) => error!("Could not serialize data: {:?}", e),
            }
        } else {
            // the channel has been closed, exit
            trace!("The channel has closed, exiting loop");
            break;
        }
    }
}

async fn client_read_task(
    mut rx_socket: OwnedReadHalf,
    tokio_to_game_sender: mpsc::Sender<NetworkMessage>,
) {
    let mut buf = [0u8; 512];

    loop {
        if let Ok(num_bytes) = rx_socket.read(&mut buf[..]).await {
            trace!("Read n bytes: {:?}", num_bytes);
            match rmp_serde::from_slice::<NetworkMessagePacket>(&buf[..num_bytes]) {
                Ok(v) => {
                    if let Err(e) = tokio_to_game_sender
                        .send(NetworkMessage {
                            addr: rx_socket.peer_addr().unwrap(),
                            packet: v,
                        })
                        .await
                    {
                        error!("Error occurred while trying to pass packet from task, the queue might be full: {:?}", e);
                    }
                }
                Err(e) => error!("Error parsing received buffer: {:?}", e),
            }
        }
    }
}

async fn client_read_task_udp(
    addr: SocketAddr,
    socket: Arc<UdpSocket>,
    tokio_to_game_sender: mpsc::Sender<NetworkMessage>,
) {
    let mut buf = [0u8; 512];

    loop {
        match socket.recv(&mut buf[..]).await {
            Ok(num_bytes) => {
                trace!("Read n bytes: {:?}", num_bytes);

                match rmp_serde::from_slice::<NetworkMessagePacket>(&buf[..num_bytes]) {
                    Ok(v) => {
                        if let Err(e) = tokio_to_game_sender
                            .send(NetworkMessage {
                                addr: addr,
                                packet: v,
                            })
                            .await
                        {
                            error!("Error occurred while trying to pass packet from task, the queue might be full: {:?}", e);
                        }
                    }
                    Err(e) => error!("Error parsing received buffer: {:?}", e),
                }
            }
            Err(e) => error!("Error reading socket: {:?}", e),
        }
    }
}

async fn client_send_task_udp(
    socket: Arc<UdpSocket>,
    mut game_to_tokio_receiver: mpsc::Receiver<NetworkMessage>,
) {
    loop {
        match game_to_tokio_receiver.recv().await {
            Some(data) => {
                trace!("Writing data to {:?} (udp): {:?}", socket.peer_addr(), data);
                match rmp_serde::to_vec(&data.packet) {
                    Ok(v) => {
                        if let Err(e) = socket.send(v.as_slice()).await {
                            error!("Could not write to socket: {:?}", e);
                        }
                    }
                    Err(e) => error!("Could not serialize data: {:?}", e),
                }
            }
            None => error!("Error receiving data in async task (udp), the channel might be closed"),
        }
    }
}

pub async fn client_loop(
    addr: IpAddr,
    port: u16,
    sender: Sender<NetworkPacket>,
    mut receiver: Receiver<NetworkPacket>,
) {
    let tcp_stream = TcpStream::connect((addr, port))
        .await
        .expect("Could not connect to server");
    let mut udp_stream = UdpSocket::bind((addr, port))
        .await
        .expect("Could not connect to server over UDP");
    let udp_sock_arc = Arc::new(udp_stream);

    let mut client: Option<Client> = None;

    let (tokio_to_game_sender, mut tokio_to_game_receiver) = mpsc::channel::<NetworkMessage>(16384);
    let (game_to_tokio_sender, game_to_tokio_receiver) = mpsc::channel::<NetworkMessage>(16384);

    let (tokio_to_game_sender_udp, mut tokio_to_game_receiver_udp) =
        mpsc::channel::<NetworkMessage>(16384);
    let (game_to_tokio_sender_udp, game_to_tokio_receiver_udp) =
        mpsc::channel::<NetworkMessage>(16384);

    let local_addr = tcp_stream.local_addr().unwrap();
    let peer_addr = tcp_stream.peer_addr().unwrap();
    let (rx_socket, tx_socket) = tcp_stream.into_split();

    if let Err(e) = udp_sock_arc.connect(peer_addr).await {
        error!("Could not connect to server udp port: {:?}", e);
    }

    let rx_socket_udp = udp_sock_arc.clone();
    let tx_socket_udp = udp_sock_arc.clone();

    tokio::spawn(async move {
        client_send_task(tx_socket, game_to_tokio_receiver).await;
    });
    tokio::spawn(async move {
        client_send_task_udp(tx_socket_udp, game_to_tokio_receiver_udp).await;
    });

    tokio::spawn(async move {
        client_read_task(rx_socket, tokio_to_game_sender).await;
    });
    tokio::spawn(async move {
        client_read_task_udp(peer_addr, rx_socket_udp, tokio_to_game_sender_udp).await;
    });

    // NOTE: this would be run once per frame in the update loop
    loop {
        // collect all messages, up to a cap so we can't stall
        let mut n_recv = 0;
        while !tokio_to_game_receiver.is_empty() && n_recv < 10000 {
            println!("Trying to receive");
            if let Ok(data) = tokio_to_game_receiver.try_recv() {
                match client {
                    Some(c) => {
                        sender
                            .send(NetworkPacket {
                                net_id: c.client_id,
                                message_type: data.packet.message_type,
                                data: data.packet.payload,
                            })
                            .await;
                    }
                    None => error!("Receiving data before knowing who we are"),
                }
            }

            n_recv += 1;
        }

        // collect all messages, up to a cap so we can't stall
        let mut n_recv_udp = 0;
        while !tokio_to_game_receiver_udp.is_empty() && n_recv_udp < 10000 {
            println!("Trying to receive udp");
            if let Ok(data) = tokio_to_game_receiver_udp.try_recv() {
                match client {
                    Some(c) => {
                        sender
                            .send(NetworkPacket {
                                net_id: c.client_id,
                                message_type: data.packet.message_type,
                                data: data.packet.payload,
                            })
                            .await;
                    }
                    None => error!("Receiving udp data before knowing who we are"),
                }
            }

            n_recv_udp += 1;
        }

        // TODO: pass data from receivers to senders
    }
}
