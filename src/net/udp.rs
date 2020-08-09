use log::{info, warn};
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

use super::{Message, NetError, Client, Server};

pub struct UdpServer {
    socket: UdpSocket,
}

impl UdpServer {
    pub fn new(port: u16) -> UdpServer {
        let unspec = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));
        let socket = match UdpSocket::bind(SocketAddr::new(unspec, port)) {
            Ok(s) => s,
            Err(e) => panic!("Couldn't listen on port {}: {}", port, e),
        };
        socket
            .set_nonblocking(true)
            .expect("Couldn't set socket nonblocking");
        info!("Listening on UDP port {}", port);
        UdpServer { socket }
    }
}

impl Server for UdpServer {
    type Address = SocketAddr;

    fn send(&self, msg: &Message, addr: &SocketAddr) -> Result<(), NetError> {
        match self.socket.send_to(&msg.bytes(), addr) {
            Ok(_) => Ok(()),
            Err(err) => {
                if err.kind() == io ::ErrorKind::WouldBlock {
                    Err(NetError::NoMore)
                } else {
                    Err(NetError::Error(Box::new(err)))
                }
            }
        }
    }

    fn recv(&mut self) -> Result<(Message, SocketAddr), NetError> {
        let mut buffer = [0; 1024];
        loop {
            let (len, addr) = match self.socket.recv_from(&mut buffer) {
                Ok(r) => r,
                Err(err) => {
                    if err.kind() == io::ErrorKind::WouldBlock {
                        return Err(NetError::NoMore);
                    } else {
                        return Err(NetError::Error(Box::new(err)));
                    }
                }
            };

            match Message::parse(&buffer[0..len]) {
                Some(msg) => return Ok((msg, addr)),
                None => warn!("Invalid message from {}", addr),
            }
        }
    }
}

pub struct UdpClient {
    socket: UdpSocket,
    server_address: SocketAddr,
}

impl UdpClient {
    pub fn new(address: SocketAddr) -> UdpClient {
        let unspec = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));
        let socket = match UdpSocket::bind(SocketAddr::new(unspec, 0)) {
            Ok(s) => s,
            Err(e) => panic!("Couldn't create a socket: {}", e),
        };
        socket
            .set_nonblocking(true)
            .expect("Couldn't set socket nonblocking");
        UdpClient {
            socket,
            server_address: address,
        }
    }
}

impl Client for UdpClient {
    fn send(&self, msg: &Message) -> Result<(), NetError> {
        match self.socket.send_to(&msg.bytes(), self.server_address) {
            Ok(_) => Ok(()),
            Err(err) => {
                if err.kind() == io ::ErrorKind::WouldBlock {
                    Err(NetError::NoMore)
                } else {
                    Err(NetError::Error(Box::new(err)))
                }
            }
        }
    }

    fn recv(&mut self) -> Result<Message, NetError> {
        let mut buffer = [0; 1024];
        loop {
            let (len, addr) = match self.socket.recv_from(&mut buffer) {
                Ok(r) => r,
                Err(err) => {
                    if err.kind() == io ::ErrorKind::WouldBlock {
                        return Err(NetError::NoMore);
                    } else {
                        return Err(NetError::Error(Box::new(err)));
                    }
                }
            };

            if addr != self.server_address {
                info!("Got message from invalid source {}", addr);
            } else {
                match Message::parse(&buffer[0..len]) {
                    Some(msg) => return Ok(msg),
                    None => warn!("Got invalid message"),
                }
            }
        }
    }
}
