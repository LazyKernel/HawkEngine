use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Instant;
use std::{net::SocketAddr, sync::Arc};

use log::{error, info, trace, warn};
use uuid::Uuid;

use crate::ecs::resources::network::{MessageType, NetworkProtocol};
use crate::network::tokio::Client;
use crate::network::{constants::UDP_BUF_SIZE, tokio::RawNetworkMessagePacket};
use crate::{
    ecs::resources::network::{NetworkData, NetworkPacket},
    network::tokio::RawNetworkMessage,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{
        tcp::{self, OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpStream, UdpSocket,
    },
    sync::{
        broadcast::{self},
        futures,
        mpsc::{self, Receiver, Sender},
        RwLock,
    },
};

async fn server_read_task(
    addr: SocketAddr,
    mut rx_socket: OwnedReadHalf,
    tokio_to_game_sender: mpsc::Sender<RawNetworkMessage>,
) {
    let mut buf = [0u8; 512];

    loop {
        match rx_socket.read(&mut buf[..]).await {
            Ok(num_bytes) => {
                trace!("Read n bytes: {:?}", num_bytes);
                match rmp_serde::from_slice::<RawNetworkMessagePacket>(&buf[..num_bytes]) {
                    Ok(v) => {
                        if let Err(e) = tokio_to_game_sender
                            .send(RawNetworkMessage {
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

async fn server_send_task(
    addr: SocketAddr,
    mut tx_socket: OwnedWriteHalf,
    mut game_to_tokio_receiver: broadcast::Receiver<RawNetworkMessage>,
) {
    loop {
        match game_to_tokio_receiver.recv().await {
            Ok(data) => {
                if data.addr == addr {
                    // this is for us
                    trace!("Writing data: {:?}", data);
                    match rmp_serde::to_vec(&data.packet) {
                        Ok(v) => {
                            if let Err(e) = tx_socket.write_all(v.as_slice()).await {
                                error!("Could not write to socket: {:?}", e);
                            }
                        }
                        Err(e) => error!("Could not serialize data: {:?}", e),
                    }
                }
            }
            Err(e) => error!("Error receiving data in async task: {:?}", e),
        }
    }
}

async fn server_read_task_udp(
    clients: Arc<RwLock<HashMap<IpAddr, Client>>>,
    socket: Arc<UdpSocket>,
    tokio_to_game_sender: mpsc::Sender<RawNetworkMessage>,
) {
    let mut buf = [0u8; 512];

    loop {
        match socket.recv_from(&mut buf[..]).await {
            Ok((num_bytes, addr)) => {
                trace!("Read n bytes from {:?}: {:?}", addr, num_bytes);

                // ignore if the client isn't connected
                // TODO: need to encrypt udp traffic at some point
                if !clients.read().await.contains_key(&addr.ip()) {
                    continue;
                }

                match rmp_serde::from_slice::<RawNetworkMessagePacket>(&buf[..num_bytes]) {
                    Ok(v) => {
                        if let Err(e) = tokio_to_game_sender
                            .send(RawNetworkMessage {
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

async fn server_send_task_udp(
    socket: Arc<UdpSocket>,
    mut game_to_tokio_receiver: mpsc::Receiver<RawNetworkMessage>,
) {
    loop {
        match game_to_tokio_receiver.recv().await {
            Some(data) => {
                trace!("Writing data to {:?} (udp): {:?}", data.addr, data);
                match rmp_serde::to_vec(&data.packet) {
                    Ok(v) => {
                        if let Err(e) = socket.send_to(v.as_slice(), data.addr).await {
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

pub async fn server_loop(
    addr: IpAddr,
    port: u16,
    sender: Sender<NetworkPacket>,
    mut receiver: Receiver<NetworkPacket>,
) {
    let tcp_listener = TcpListener::bind((addr, port)).await.unwrap();
    let udp_socket = UdpSocket::bind((addr, port)).await.unwrap();
    let udp_socket_arc = Arc::new(udp_socket);

    let mut clients: Arc<RwLock<HashMap<IpAddr, Client>>> = Default::default();
    let mut clients_net_id: Arc<RwLock<HashMap<Uuid, Client>>> = Default::default();

    let (tokio_to_game_sender, mut tokio_to_game_receiver) =
        mpsc::channel::<RawNetworkMessage>(16384);
    let (game_to_tokio_sender, game_to_tokio_receiver) =
        broadcast::channel::<RawNetworkMessage>(16384);

    let (tokio_to_game_sender_udp, mut tokio_to_game_receiver_udp) =
        mpsc::channel::<RawNetworkMessage>(16384);
    let (game_to_tokio_sender_udp, game_to_tokio_receiver_udp) =
        mpsc::channel::<RawNetworkMessage>(16384);

    let receiver_generator = game_to_tokio_sender.clone();
    drop(game_to_tokio_receiver);

    tokio::spawn(async move {
        loop {
            let (socket, addr) = tcp_listener.accept().await.unwrap();

            info!("Got a connection from {:?}", addr);

            let (rx_socket, tx_socket) = socket.into_split();
            let sender = tokio_to_game_sender.clone();

            // receiving from this client
            tokio::spawn(async move { server_read_task(addr, rx_socket, sender).await });

            // sending to this client
            let rx = receiver_generator.subscribe();
            tokio::spawn(async move { server_send_task(addr, tx_socket, rx).await });
        }
    });

    let udp_sock_rx = udp_socket_arc.clone();
    let udp_sock_tx = udp_socket_arc.clone();
    let clients_ref = clients.clone();
    tokio::spawn(async move {
        server_read_task_udp(clients_ref, udp_sock_rx, tokio_to_game_sender_udp).await
    });
    tokio::spawn(
        async move { server_send_task_udp(udp_sock_tx, game_to_tokio_receiver_udp).await },
    );

    // NOTE: this would be run once per frame in the update loop
    loop {
        // collect all messages, up to a cap so we can't stall
        let mut n_recv = 0;
        while !tokio_to_game_receiver.is_empty() && n_recv < 10000 {
            trace!("Trying to receive");
            match tokio_to_game_receiver.try_recv() {
                Ok(data) => {
                    let client = clients.read().await.get(&data.addr.ip());
                    match client {
                        Some(c) => {
                            sender
                                .send(NetworkPacket {
                                    net_id: c.client_id,
                                    message_type: data.packet.message_type,
                                    protocol: NetworkProtocol::TCP,
                                    data: data.packet.payload,
                                })
                                .await;
                        }
                        None => error!("Unknown client: {:?}", data.addr),
                    }
                }
                Err(e) => error!("Error trying to receive from tokio: {:?}", e),
            }

            n_recv += 1;
        }

        let mut n_recv_udp = 0;
        while !tokio_to_game_receiver_udp.is_empty() && n_recv_udp < 10000 {
            trace!("Trying to receive udp");
            match tokio_to_game_receiver_udp.try_recv() {
                Ok(data) => {
                    let client = clients.read().await.get(&data.addr.ip());
                    match client {
                        Some(c) => {
                            sender
                                .send(NetworkPacket {
                                    net_id: c.client_id,
                                    message_type: data.packet.message_type,
                                    protocol: NetworkProtocol::UDP,
                                    data: data.packet.payload,
                                })
                                .await;
                        }
                        None => error!("Unknown client: {:?}", data.addr),
                    }
                }
                Err(e) => error!("Error tryingto receive from tokio (udp): {:?}", e),
            }

            n_recv_udp += 1;
        }

        // passing our data to server
        while !receiver.is_empty() {
            trace!("sending our data");
            match receiver.try_recv() {
                Ok(packet) => {
                    if let Some(client) = clients_net_id.read().await.get(&packet.net_id) {
                        match packet.protocol {
                            NetworkProtocol::TCP => {
                                game_to_tokio_sender.send(RawNetworkMessage {
                                    addr: client.addr,
                                    packet: RawNetworkMessagePacket {
                                        message_type: packet.message_type,
                                        payload: packet.data,
                                    },
                                });
                            }
                            NetworkProtocol::UDP => {
                                game_to_tokio_sender_udp
                                    .send(RawNetworkMessage {
                                        addr: client.addr,
                                        packet: RawNetworkMessagePacket {
                                            message_type: packet.message_type,
                                            payload: packet.data,
                                        },
                                    })
                                    .await;
                            }
                        };
                    } else {
                        error!("Client with net id {:?} does not exist!", packet.net_id);
                    }
                }
                Err(e) => error!("Error trying to receive data to send out: {:?}", e),
            }
        }
    }
}
