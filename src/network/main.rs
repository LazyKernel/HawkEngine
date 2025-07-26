use std::{collections::HashMap, env, error::Error, net::SocketAddr, sync::Arc, time::Instant};
use log::{error, info, log, warn};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{tcp, TcpListener, TcpStream, UdpSocket}, sync::{broadcast, mpsc}};
use uuid::{uuid, Uuid};
use serde::{Serialize, Deserialize};

struct Client {
    client_id: Uuid,
    addr: SocketAddr,
    last_keep_alive: Instant
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum NetworkMessageType {
    Unknown = 0,
    ConnectionRequest,
    ConnectionAccept,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NetworkMessagePacket {
    message_type: NetworkMessageType,
    payload: Vec<u8>
}

#[derive(Clone, Debug)]
struct NetworkMessage {
    addr: SocketAddr,
    packet: NetworkMessagePacket
}

#[derive(Serialize, Deserialize)]
struct ConnectionAccepted {
    client_id: Uuid, 
    server_version: String,
}

impl Client {
    fn new(addr: SocketAddr) -> Self {
        Self {
            client_id: Uuid::new_v4(),
            addr: addr,
            last_keep_alive: Instant::now()
        }
    }
}

fn build_network_message<T: Serialize>(message_type: NetworkMessageType, payload: Option<T>) -> Result<NetworkMessagePacket, rmp_serde::encode::Error> {
    Ok(NetworkMessagePacket {
        message_type: message_type,
        payload: match payload {
            Some(v) => rmp_serde::to_vec(&v)?,
            None => Vec::<u8>::default(),
        }
    })
}

fn server_handle_connect(clients: &mut HashMap<SocketAddr, Client>, addr: SocketAddr) -> NetworkMessagePacket {
    let client = Client::new(addr);

    println!("New client connected with ID: {}", client.client_id);

    let conn_acc = ConnectionAccepted {client_id: client.client_id, server_version: "0.0.1".into()};
    let conn_acc_msg = build_network_message(NetworkMessageType::ConnectionAccept, Some(conn_acc)).expect("Could not serialize ConnectionAccept");
    clients.insert(addr, client);

    return conn_acc_msg;
}


async fn server() {
    let tcp_listener = TcpListener::bind("127.0.0.1:6782").await.unwrap();
    let udp_socket = UdpSocket::bind("0.0.0.0:6782").await.unwrap();

    let mut clients: HashMap<SocketAddr, Client> = Default::default();

    let (a2s_sender, mut a2s_receiver) = mpsc::channel::<NetworkMessage>(16384);
    let (s2a_sender, s2a_receiver) = broadcast::channel::<NetworkMessage>(16384);


    tokio::spawn(async move {
        loop {
            let (socket, addr) = tcp_listener.accept().await.unwrap();

            let (mut rx_socket, mut tx_socket) = socket.into_split();
            let sender = a2s_sender.clone();

            // receiving from this client
            tokio::spawn(async move {
                let mut buf = [0u8; 512];

                loop {
                    if let Ok(num_bytes) = rx_socket.read(&mut buf[..]).await {
                        println!("Read n bytes: {:?}", num_bytes);
                        match rmp_serde::from_slice::<NetworkMessagePacket>(&buf[..num_bytes]) {
                            Ok(v) => {
                                if let Err(e) = sender.send(NetworkMessage { addr: addr, packet: v }).await {
                                    error!("Error occurred while trying to pass packet from task, the queue might be full: {:?}", e);
                                }
                            },
                            Err(e) => error!("Error parsing received buffer: {:?}", e),
                        }
                    }
                }
            });

            // sending to this client
            let mut rx = s2a_receiver.resubscribe();
            tokio::spawn(async move {

                loop {
                    if let Ok(data) = rx.recv().await {
                        if data.addr == addr {
                            // this is for us
                            match rmp_serde::to_vec(&data.packet) {
                                Ok(v) => {
                                    if let Err(e) = tx_socket.write_all(v.as_slice()).await {
                                        error!("Could not write to socket: {:?}", e);
                                    }
                                },
                                Err(e) => error!("Could not serialize data: {:?}", e),
                            }
                        }
                    }
                }
            });
        }
    });

    // NOTE: this would be run once per frame in the update loop
    loop {

        // collect all messages, up to a cap so we can't stall
        let mut n_recv = 0;
        while !a2s_receiver.is_empty() && n_recv < 10000 {
            if let Ok(data) = a2s_receiver.try_recv() {
                match data.packet.message_type {
                    NetworkMessageType::ConnectionRequest => {
                        let conn_acc_msg = server_handle_connect(&mut clients, data.addr);
                        if let Err(e) = s2a_sender.send(NetworkMessage { addr: data.addr, packet: conn_acc_msg }) {
                            error!("Could not send message to broadcast queue, might be full: {:?}", e);
                        }
                    },
                    _ => warn!("Unsupported message type: {:?}", data.packet.message_type),
                }
            }

            n_recv += 1;
        }
    }
}


fn client_handle_connect(local_addr: SocketAddr, packet: &NetworkMessagePacket) -> Result<Client, rmp_serde::decode::Error> {
    let accept_data = rmp_serde::from_slice::<ConnectionAccepted>(&packet.payload)?;
    Ok(Client { addr: local_addr, client_id: accept_data.client_id, last_keep_alive: Instant::now() })
}

async fn client() {
    let tcp_stream = TcpStream::connect("127.0.0.1:6782").await.expect("Could not connect to server");
    //let mut udp_stream = UdpSocket::bind("0.0.0.0:6782").await.expect("Could not connect to server over UDP");

    
    let mut client: Option<Client> = None;

    let (a2s_sender, mut a2s_receiver) = mpsc::channel::<NetworkMessage>(16384);
    let (s2a_sender, mut s2a_receiver) = mpsc::channel::<NetworkMessage>(16384);

    let local_addr = tcp_stream.local_addr().unwrap();
    let peer_addr = tcp_stream.peer_addr().unwrap();
    let (mut rx_socket, mut tx_socket) = tcp_stream.into_split();

    // receiving from server
    tokio::spawn(async move {
        let mut buf = [0u8; 512];

        loop {
            if let Ok(num_bytes) = rx_socket.read(&mut buf[..]).await {
                println!("Read n bytes: {:?}", num_bytes);
                match rmp_serde::from_slice::<NetworkMessagePacket>(&buf[..num_bytes]) {
                    Ok(v) => {
                        if let Err(e) = a2s_sender.send(NetworkMessage { addr: rx_socket.peer_addr().unwrap(), packet: v }).await {
                            error!("Error occurred while trying to pass packet from task, the queue might be full: {:?}", e);
                        }
                    },
                    Err(e) => error!("Error parsing received buffer: {:?}", e),
                }
            }
        }
    });

    // sending to this client
    tokio::spawn(async move {

        loop {
            if let Some(data) = s2a_receiver.recv().await {
                match rmp_serde::to_vec(&data.packet) {
                    Ok(v) => {
                        if let Err(e) = tx_socket.write_all(v.as_slice()).await {
                            error!("Could not write to socket: {:?}", e);
                        }
                    },
                    Err(e) => error!("Could not serialize data: {:?}", e),
                }
            }
            else {
                // the channel has been closed, exit
                break;
            }
        }
    });


    let msg = NetworkMessagePacket {message_type: NetworkMessageType::ConnectionRequest, payload: vec![]};
    let _ = s2a_sender.send(NetworkMessage { addr: peer_addr, packet: msg });


    // NOTE: this would be run once per frame in the update loop
    loop {

        // collect all messages, up to a cap so we can't stall
        let mut n_recv = 0;
        while !a2s_receiver.is_empty() && n_recv < 10000 {
            if let Ok(data) = a2s_receiver.try_recv() {
                match data.packet.message_type {
                    NetworkMessageType::ConnectionAccept => {
                        match client_handle_connect(local_addr, &data.packet) {
                            Ok(v) => {
                                client = Some(v);
                                println!("Connected with ID: {}", client.unwrap().client_id);
                            },
                            Err(e) => error!("Failed to parse ConnectionAccept payload: {:?}", e),
                        }
                    },
                    _ => warn!("Unsupported message type: {:?}", data.packet.message_type),
                }
            }

            n_recv += 1;
        }
    }
}


#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let args: Vec<String> = env::args().collect();

    if args.contains(&"--server".to_string()) || args.contains(&"-s".to_string()) {
        server().await;
    }
    else {
        client().await;
    }
}
