use std::{collections::HashMap, env, error::Error, net::SocketAddr, time::Instant};
use log::{error, info, log, warn};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{tcp, TcpListener, TcpStream, UdpSocket}};
use uuid::{uuid, Uuid};
use serde::{Serialize, Deserialize};

struct Client {
    client_id: Uuid,
    last_keep_alive: Instant
}

#[derive(Debug, Serialize, Deserialize)]
enum NetworkMessageType {
    Unknown = 0,
    ConnectionRequest,
    ConnectionAccept,
}

#[derive(Debug, Serialize, Deserialize)]
struct NetworkMessage {
    message_type: NetworkMessageType,
    payload: Vec<u8>
}

#[derive(Serialize, Deserialize)]
struct ConnectionAccepted {
    client_id: Uuid, 
    server_version: String,
}

impl Default for Client {
    fn default() -> Self {
        Self {
            client_id: Uuid::new_v4(),
            last_keep_alive: Instant::now()
        }
    }
}

fn build_network_message<T: Serialize>(message_type: NetworkMessageType, payload: Option<T>) -> Result<Vec<u8>, rmp_serde::encode::Error> {
    let net_msg = NetworkMessage {
        message_type: message_type,
        payload: match payload {
            Some(v) => rmp_serde::to_vec(&v)?,
            None => Vec::<u8>::default(),
        }
    };

    return rmp_serde::to_vec(&net_msg);
}

fn server_handle_connect(clients: &mut HashMap<SocketAddr, Client>, addr: SocketAddr) -> Vec<u8> {
    let client = Client::default();

    println!("New client connected with ID: {}", client.client_id);

    let conn_acc = ConnectionAccepted {client_id: client.client_id, server_version: "0.0.1".into()};
    let conn_acc_msg = build_network_message(NetworkMessageType::ConnectionAccept, Some(conn_acc)).expect("Could not serialize ConnectionAccept");
    clients.insert(addr, client);

    return conn_acc_msg;
}

fn client_handle_connect() {

}

async fn server() {
    let tcp_listener = TcpListener::bind("127.0.0.1:6782").await.unwrap();
    let udp_socket = UdpSocket::bind("0.0.0.0:6782").await.unwrap();

    let mut clients: HashMap<SocketAddr, Client> = Default::default();

    loop {
        let (mut socket, addr) = tcp_listener.accept().await.unwrap();

        tokio::spawn(async move {
            let mut buf = [0u8; 512];
            if let Ok(num_bytes) = socket.read(&mut buf[..]).await {
                println!("Read n bytes: {:?}", num_bytes);
                match rmp_serde::from_slice::<NetworkMessage>(&buf[..num_bytes]) {
                    Ok(v) => {
                        match v.message_type {
                            NetworkMessageType::ConnectionRequest => {
                                let conn_acc_msg = server_handle_connect(&mut clients, addr);
                                let _ = socket.write_all(conn_acc_msg.as_slice()).await;
                            },
                            _ => warn!("Unsupported message type: {:?}", v.message_type),
                        }
                    },
                    Err(e) => error!("Error parsing received buffer: {:?}", e),
                }
            }
        });
    }
}

async fn client() {
    let mut tcp_stream = TcpStream::connect("127.0.0.1:6782").await.expect("Could not connect to server");
    //let mut udp_stream = UdpSocket::bind("0.0.0.0:6782").await.expect("Could not connect to server over UDP");

    let _ = tcp_stream.write_all(b"connect").await;
    
    let mut client: Option<Client> = None;

    loop {
        let mut buf = [0u8; 512];
        if let Ok(num_bytes) = tcp_stream.read(&mut buf[..]).await {

            if num_bytes > 0 {
                println!("Read n bytes: {:?}", num_bytes);
                match rmp_serde::from_slice::<ConnectionAccepted>(&buf[..]) {
                    Ok(v) => {
                        client = Some(Client { client_id: v.client_id, last_keep_alive: Instant::now() });
                        println!("Connected with ID: {}", v.client_id);
                    },
                    Err(e) => error!("Could not deserialize:\n{}", e),
                }
            }
        }
    }
}


#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.contains(&"--server".to_string()) || args.contains(&"-s".to_string()) {
        server().await;
    }
    else {
        client().await;
    }
}
