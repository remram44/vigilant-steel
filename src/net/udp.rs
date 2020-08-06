use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::io;

use super::{Client, Server};

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

    fn send(&self, msg: &[u8], addr: &SocketAddr) -> io::Result<usize> {
        self.socket.send_to(msg, addr)
    }

    fn recv(&self, buffer: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.socket.recv_from(buffer)
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
    fn send(&self, msg: &[u8]) -> io::Result<usize> {
        self.socket.send_to(msg, self.server_address)
    }

    fn recv(&self, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            let (len, addr) = self.socket.recv_from(buffer)?;
            if addr != self.server_address {
                info!("Got message from invalid source {}", addr);
            } else {
                return Ok(len);
            }
        }
    }
}
