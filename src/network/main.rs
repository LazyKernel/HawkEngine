use std::env;
use log::error;
use tokio::{io::AsyncReadExt, io::AsyncWriteExt, net::{TcpListener, TcpStream}};


async fn server() {
    let listener = TcpListener::bind("127.0.0.1:6782").await.unwrap();

    loop {
        let (mut socket, _) = listener.accept().await.unwrap();

        tokio::spawn(async move {
            let mut buf = [0u8; 512];
            if let Ok(num_bytes) = socket.read(&mut buf[..]).await {
                println!("Read n bytes: {:?}", num_bytes);
                match str::from_utf8(&buf[..num_bytes]) {
                    Ok(v) => println!("{v}"),
                    Err(e) => error!("Error parsing received buffer: {:?}", e),
                }
            }
        });
    }
}

async fn client() {
    let mut stream = TcpStream::connect("127.0.0.1:6782").await.unwrap();

    let _ = stream.write_all(b"hello server, how it goin?").await;
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
