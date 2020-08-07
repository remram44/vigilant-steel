use log::{info, warn};
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

use super::{NetError, Client, Server};

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
        UdpServer { socket }
    }
}

impl Server for UdpServer {
    type Address = SocketAddr;

    fn send(&self, msg: &[u8], addr: &SocketAddr) -> Result<(), NetError> {
        match self.socket.send_to(msg, addr) {
            Ok(_) => Ok(()),
            Err(err) => {
                if err.kind() == io ::ErrorKind::WouldBlock {
                    Err(NetError::FlowControl)
                } else {
                    warn!("Send error: {}", err);
                    Err(NetError::Disconnected)
                }
            }
        }
    }

    fn recv(&mut self, buffer: &mut [u8]) -> Result<(usize, SocketAddr), NetError> {
        match self.socket.recv_from(buffer) {
            Ok(r) => Ok(r),
            Err(err) => {
                if err.kind() == io ::ErrorKind::WouldBlock {
                    Err(NetError::FlowControl)
                } else {
                    warn!("Send error: {}", err);
                    Err(NetError::Disconnected)
                }
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
    fn send(&self, msg: &[u8]) -> Result<(), NetError> {
        match self.socket.send_to(msg, self.server_address) {
            Ok(_) => Ok(()),
            Err(err) => {
                if err.kind() == io ::ErrorKind::WouldBlock {
                    Err(NetError::FlowControl)
                } else {
                    warn!("Send error: {}", err);
                    Err(NetError::Disconnected)
                }
            }
        }
    }

    fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, NetError> {
        loop {
            let (len, addr) = match self.socket.recv_from(buffer) {
                Ok(r) => r,
                Err(err) => {
                    if err.kind() == io ::ErrorKind::WouldBlock {
                        return Err(NetError::FlowControl);
                    } else {
                        warn!("Send error: {}", err);
                        return Err(NetError::Disconnected);
                    }
                }
            };

            if addr != self.server_address {
                info!("Got message from invalid source {}", addr);
            } else {
                return Ok(len);
            }
        }
    }
}
