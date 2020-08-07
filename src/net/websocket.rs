use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::mpsc::{Sender, Receiver, TryRecvError, channel};
use std::thread;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use super::{NetError, Server};

async fn handle_connection(
    sender: Sender<(Vec<u8>, SocketAddr)>,
    stream: TcpStream,
    addr: SocketAddr,
) {
    // TODO: Websocket stuff
}

async fn server(
    port: u16,
    sender: Sender<(Vec<u8>, SocketAddr)>,
    write_queue: UnboundedReceiver<(Vec<u8>, SocketAddr)>,
) {
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
    // TODO: Send messages from write_queue
}

pub struct WebsocketServer {
    recv_queue: Receiver<(Vec<u8>, SocketAddr)>,
    write_queue: UnboundedSender<(Vec<u8>, SocketAddr)>,
}

impl WebsocketServer {
    pub fn new(port: u16) -> WebsocketServer {
        let (recv_sender, recv_recv) = channel();
        let (write_send, write_recv) = unbounded_channel();
        thread::spawn(move || {
            let mut rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                server(port, recv_sender, write_recv).await;
            });
        });
        WebsocketServer {
            recv_queue: recv_recv,
            write_queue: write_send,
        }
    }
}

impl Server for WebsocketServer {
    type Address = SocketAddr;

    fn send(&self, msg: &[u8], addr: &SocketAddr) -> Result<(), NetError> {
        self.write_queue.send((msg.to_owned(), addr.clone())).unwrap();
        Ok(())
    }

    fn recv(&self, buffer: &mut [u8]) -> Result<(usize, SocketAddr), NetError> {
        match self.recv_queue.try_recv() {
            Err(TryRecvError::Empty) => Err(NetError::FlowControl),
            Err(TryRecvError::Disconnected) => panic!("Network thread error"),
            Ok((data, src)) => {
                buffer[0..data.len()].clone_from_slice(&data);
                Ok((data.len(), src))
            }
        }
    }
}
