use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::mpsc::{Sender, Receiver, channel};
use std::thread;
use tokio::net::{TcpListener, TcpStream};

use super::Server;

async fn handle_connection(
    sender: Sender<(Vec<u8>, u64)>,
    stream: TcpStream,
    addr: SocketAddr,
) {
}

async fn server(port: u16, sender: Sender<(Vec<u8>, u64)>) {
    let unspec = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));
    let mut listener = match TcpListener::bind(SocketAddr::new(
        unspec,
        port,
    ))
    .await
    {
        Ok(s) => s,
        Err(e) => panic!("Couldn't listen on port {}: {}", port, e),
    };
    while let Ok((stream, addr)) = listener.accept().await {
        tokio::spawn(handle_connection(sender.clone(), stream, addr));
    }
}

pub struct WebsocketServer {
    queue: Receiver<(Vec<u8>, u64)>,
}

impl WebsocketServer {
    pub fn new(port: u16) -> WebsocketServer {
        let (sender, receiver) = channel();
        thread::spawn(move || {
            let mut rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                server(port, sender).await;
            });
        });
        unimplemented!()
    }
}
