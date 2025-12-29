use std::net::IpAddr;
use std::{net::SocketAddr, sync::Arc};

use log::{error, trace};
use tokio::sync::broadcast;

use crate::ecs::resources::network::{MessageType, NetworkProtocol};
use crate::ecs::resources::network::{NetworkPacketIn, NetworkPacketOut};
use crate::ecs::systems::network::connection_handler::ConnectionAcceptData;
use crate::network::tokio::{Client, RawNetworkMessage, RawNetworkMessagePacket};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream, UdpSocket,
    },
    sync::mpsc::{self, Receiver, Sender},
};

async fn client_send_task(
    mut tx_socket: OwnedWriteHalf,
    mut game_to_tokio_receiver: mpsc::Receiver<RawNetworkMessage>,
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
    tokio_to_game_sender: mpsc::Sender<RawNetworkMessage>,
) {
    let mut buf = [0u8; 512];

    loop {
        if let Ok(num_bytes) = rx_socket.read(&mut buf[..]).await {
            trace!("Read n bytes: {:?}", num_bytes);
            match rmp_serde::from_slice::<RawNetworkMessagePacket>(&buf[..num_bytes]) {
                Ok(v) => {
                    if let Err(e) = tokio_to_game_sender
                        .send(RawNetworkMessage {
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
    tokio_to_game_sender: mpsc::Sender<RawNetworkMessage>,
) {
    let mut buf = [0u8; 512];

    loop {
        match socket.recv(&mut buf[..]).await {
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

async fn client_send_task_udp(
    socket: Arc<UdpSocket>,
    mut game_to_tokio_receiver: mpsc::Receiver<RawNetworkMessage>,
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
    sender: broadcast::Sender<NetworkPacketIn>,
    mut receiver: mpsc::Receiver<NetworkPacketOut>,
) {
    let tcp_stream = TcpStream::connect((addr, port))
        .await
        .expect("Could not connect to server");
    let udp_stream = UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("Could not connect to server over UDP");
    let _ = udp_stream.connect((addr, port + 1)).await;
    let udp_sock_arc = Arc::new(udp_stream);

    let mut client: Client = Default::default();

    let (tokio_to_game_sender, mut tokio_to_game_receiver) =
        mpsc::channel::<RawNetworkMessage>(16384);
    let (game_to_tokio_sender, game_to_tokio_receiver) = mpsc::channel::<RawNetworkMessage>(16384);

    let (tokio_to_game_sender_udp, mut tokio_to_game_receiver_udp) =
        mpsc::channel::<RawNetworkMessage>(16384);
    let (game_to_tokio_sender_udp, game_to_tokio_receiver_udp) =
        mpsc::channel::<RawNetworkMessage>(16384);

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
                // NOTE: grabbing our assigned client id here, not ideal
                if data.packet.message_type == MessageType::ConnectionAccept {
                    match rmp_serde::from_slice::<ConnectionAcceptData>(&data.packet.payload) {
                        Ok(data) => {
                            client = Client {
                                client_id: data.uuid,
                                addr: local_addr,
                            };
                        }
                        Err(e) => {
                            error!("Could not deserialize ConnectionAcceptData: {:?}", e);
                        }
                    }
                }

                if let Err(e) = sender.send(NetworkPacketIn {
                    client: client.clone(),
                    message_type: data.packet.message_type,
                    protocol: NetworkProtocol::TCP,
                    data: data.packet.payload,
                }) {
                    error!("Could not pass packet to game: {:?}", e);
                }
            }

            n_recv += 1;
        }

        // collect all messages, up to a cap so we can't stall
        let mut n_recv_udp = 0;
        while !tokio_to_game_receiver_udp.is_empty() && n_recv_udp < 10000 {
            println!("Trying to receive udp");
            if let Ok(data) = tokio_to_game_receiver_udp.try_recv() {
                if let Err(e) = sender.send(NetworkPacketIn {
                    client: client.clone(),
                    message_type: data.packet.message_type,
                    protocol: NetworkProtocol::UDP,
                    data: data.packet.payload,
                }) {
                    error!("Could not pass udp packet to game: {:?}", e);
                }
            }

            n_recv_udp += 1;
        }

        while !receiver.is_empty() {
            trace!("sending our data");
            match receiver.try_recv() {
                Ok(packet) => {
                    match packet.protocol {
                        NetworkProtocol::TCP => {
                            if let Err(e) = game_to_tokio_sender
                                .send(RawNetworkMessage {
                                    addr: peer_addr,
                                    packet: RawNetworkMessagePacket {
                                        message_type: packet.message_type,
                                        payload: packet.data,
                                    },
                                })
                                .await
                            {
                                error!("Could not pass raw tcp packet to tokio: {:?}", e);
                            }
                        }
                        NetworkProtocol::UDP => {
                            if let Err(e) = game_to_tokio_sender_udp
                                .send(RawNetworkMessage {
                                    addr: peer_addr,
                                    packet: RawNetworkMessagePacket {
                                        message_type: packet.message_type,
                                        payload: packet.data,
                                    },
                                })
                                .await
                            {
                                error!("Could not pass raw udp packet to tokio: {:?}", e);
                            }
                        }
                    };
                }
                Err(e) => error!("Error trying to receive data to send out: {:?}", e),
            }
        }
    }
}
