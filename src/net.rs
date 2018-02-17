//! Network code.

use byteorder::{self, ReadBytesExt, WriteBytesExt};
use physics::Position;
use specs::{Component, HashMapStorage, Join, NullStorage, ReadStorage,
            System, VecStorage, WriteStorage};
use std::io::{self, Cursor};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::SystemTime;

const MAX_MESSAGES_PER_FRAME: u16 = 5;

type ORDER = byteorder::BigEndian;

/// This describes the role of the local machine in the game.
///
/// This is available as a specs Resource and can be used to decide what to
/// simulate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Standalone,
    Server,
    Client,
}

impl Role {
    /// Whether the local machine is authoritative over the world.
    ///
    /// If this is false, the local machine should delegate important decisions
    /// to the server, and only interpolate the game state.
    pub fn authoritative(&self) -> bool {
        match self {
            &Role::Standalone => true,
            &Role::Server => true,
            &Role::Client => false,
        }
    }

    /// Whether the local machine is showing the world graphically.
    ///
    /// If this is false, there is no point bothering about animations or
    /// particles that don't affect the game, since no one will see them.
    pub fn graphical(&self) -> bool {
        match self {
            &Role::Standalone => true,
            &Role::Server => false,
            &Role::Client => true,
        }
    }
}

/// The message exchanged by server and clients.
enum Message {
    /// Message sent by a client to introduce itself.
    ///
    /// The server will reply with ServerHello.
    ClientHello,
    /// Message sent by the server to accept a client, and assign it an entity
    /// to control.
    ServerHello(u64, u64),
    /// Ping request, other side should send bytes back as Pong.
    Ping([u8; 12]),
    /// Pong reply, with the bytes from the Ping request.
    Pong([u8; 12]),
    /// Entity update, from either side.
    ///
    /// The server sends full entity updates that the client applies. The
    /// client sends update to the controls, preceded by its secret.
    EntityUpdate(u64, Vec<u8>),
    /// Entity removed, from server.
    EntityRemove(u64),
}

impl Message {
    /// Parse a message from some bytes.
    fn parse(msg: &[u8]) -> Option<Message> {
        if msg.len() < 8 || &msg[..6] != b"SPAC\x00\x01" {
            return None;
        }
        let mut rdr = Cursor::new(&msg[8..]);
        match &msg[6..8] {
            b"hc" => {
                if msg.len() != 8 {
                    info!("Invalid ClientHello length");
                    None
                } else {
                    Some(Message::ClientHello)
                }
            }
            b"hs" => {
                if msg.len() != 8 + 8 + 8 {
                    info!("Invalid ServerHello length");
                    None
                } else {
                    Some(Message::ServerHello(
                        rdr.read_u64::<ORDER>().unwrap(),
                        rdr.read_u64::<ORDER>().unwrap(),
                    ))
                }
            }
            b"pi" => {
                if msg.len() != 20 {
                    info!("Invalid Ping length");
                    None
                } else {
                    let mut buf = [0; 12];
                    buf.clone_from_slice(&msg[8..]);
                    Some(Message::Ping(buf))
                }
            }
            b"po" => {
                if msg.len() != 20 {
                    info!("Invalid Pong length");
                    None
                } else {
                    let mut buf = [0; 12];
                    buf.clone_from_slice(&msg[8..]);
                    Some(Message::Pong(buf))
                }
            }
            b"eu" => {
                if msg.len() < 16 {
                    info!("Invalid EntityUpdate length");
                    None
                } else {
                    Some(Message::EntityUpdate(rdr.read_u64::<ORDER>().unwrap(),
                        msg[16..].into()))
                }
            }
            b"er" => {
                if msg.len() != 16 {
                    info!("Invalid EntityRemove length");
                    None
                } else {
                    Some(Message::EntityRemove(rdr.read_u64::<ORDER>().unwrap()))
                }
            }
            _ => None,
        }
    }

    /// Turn a message into bytes.
    fn bytes(&self) -> Vec<u8> {
        let mut msg: Vec<u8> = Vec::with_capacity(20);
        msg.extend_from_slice(b"SPAC\x00\x01");
        match self {
            &Message::ClientHello => msg.extend_from_slice(b"hc"),
            &Message::ServerHello(id, secret) => {
                msg.extend_from_slice(b"hs");
                msg.write_u64::<ORDER>(id).unwrap();
                msg.write_u64::<ORDER>(secret).unwrap();
                assert_eq!(msg.len(), 8 + 8 + 8);
            }
            &Message::Ping(bytes) => {
                msg.extend_from_slice(b"pi");
                msg.extend_from_slice(&bytes);
            },
            &Message::Pong(bytes) => {
                msg.extend_from_slice(b"po");
                msg.extend_from_slice(&bytes);
            },
            &Message::EntityUpdate(id, ref bytes) => {
                msg.extend_from_slice(b"eu");
                msg.write_u64::<ORDER>(id).unwrap();
                msg.extend_from_slice(&bytes);
            }
            &Message::EntityRemove(id) => {
                msg.extend_from_slice(b"er");
                msg.write_u64::<ORDER>(id).unwrap();
            }
        }
        msg
    }
}

/// Warns if a Result is an error.
fn chk<T>(res: Result<T, io::Error>) {
    match res {
        Ok(_) => {}
        Err(e) => warn!("Network error: {}", e),
    }
}

/// Replicated entities have an id to match them on multiple machines.
pub struct Replicated {
    id: u64,
    last_send: u32,
}

impl Component for Replicated {
    type Storage = VecStorage<Self>;
}

/// Flag that marks an entity as dirty, eg needs to be sent to clients.
#[derive(Default)]
pub struct Dirty;

impl Component for Dirty {
    type Storage = NullStorage<Self>;
}

/// Server component attached to entities controlled by clients.
///
/// Multiple entities can be controlled by the same client, and that's fine.
pub struct ConnectedClient {
    address: SocketAddr,
    secret: u64,
    ping: f64,
    last_ping: SystemTime,
    quota: u32,
}

impl Component for ConnectedClient {
    type Storage = HashMapStorage<Self>;
}

/// Network server system.
///
/// Gets controls from clients and sends game updates.
pub struct SysNetServer {
    socket: UdpSocket,
    frame: u32,
}

impl SysNetServer {
    /// Create a server, listening on the given port.
    pub fn new(port: u16) -> SysNetServer {
        let unspec = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));
        let socket = match UdpSocket::bind(SocketAddr::new(unspec, port)) {
            Ok(s) => s,
            Err(e) => panic!("Couldn't listen on port {}: {}", port, e),
        };
        socket
            .set_nonblocking(true)
            .expect("Couldn't set socket nonblocking");
        SysNetServer {
            socket: socket,
            frame: 0,
        }
    }

    /// Sends a message.
    fn send(&self, msg: Message, addr: &SocketAddr) -> io::Result<usize> {
        self.socket.send_to(&msg.bytes(), addr)
    }

}

impl<'a> System<'a> for SysNetServer {
    type SystemData = (
        ReadStorage<'a, Position>,
        WriteStorage<'a, Replicated>,
        WriteStorage<'a, ConnectedClient>,
    );

    fn run(&mut self, (position, replicated, client): Self::SystemData) {
        self.frame += 1;

        // Receive messages
        let mut buffer = [0; 1024];
        loop {
            let (len, src) = match self.socket.recv_from(&mut buffer) {
                Ok(r) => r,
                Err(e) => {
                    if e.kind() != io::ErrorKind::WouldBlock {
                        warn!("Error reading from socket: {}", e);
                    }
                    break;
                }
            };

            if let Some(msg) = Message::parse(&buffer[..len]) {
                match msg {
                    Message::ClientHello => unimplemented!(),
                    Message::Ping(bytes) => chk(self.send(Message::Pong(bytes), &src)),
                    Message::Pong(bytes) => unimplemented!(),
                    Message::EntityUpdate(id, bytes) => unimplemented!(),
                    Message::ServerHello(_, _)
                    | Message::EntityRemove(_) => {
                        info!("Invalid message from {}", src)
                    }
                }
            } else {
                info!("Invalid message from {}", src);
                continue;
            }
        }

        for (pos, repli, cl) in (&position, &replicated, &client).join() {}
    }
}

/// Network client system.
///
/// Sends controls to server and gets game updates.
pub struct SysNetClient {
    socket: UdpSocket,
    server_address: SocketAddr,
}

impl SysNetClient {
    /// Create a client, connected to the specified server.
    pub fn new(address: SocketAddr) -> SysNetClient {
        let unspec = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));
        let socket = match UdpSocket::bind(SocketAddr::new(unspec, 0)) {
            Ok(s) => s,
            Err(e) => panic!("Couldn't create a socket: {}", e),
        };
        socket
            .set_nonblocking(true)
            .expect("Couldn't set socket nonblocking");
        let client = SysNetClient {
            socket: socket,
            server_address: address,
        };
        client.send(Message::ClientHello).unwrap();
        client
    }

    /// Sends a message
    fn send(&self, msg: Message) -> io::Result<usize> {
        self.socket.send_to(&msg.bytes(), &self.server_address)
    }
}

impl<'a> System<'a> for SysNetClient {
    type SystemData = ();

    fn run(&mut self, _: Self::SystemData) {
        // Receive messages
        let mut buffer = [0; 1024];
        loop {
            let (len, src) = match self.socket.recv_from(&mut buffer) {
                Ok(r) => r,
                Err(e) => {
                    if e.kind() != io::ErrorKind::WouldBlock {
                        warn!("Error reading from socket: {}", e);
                    }
                    break;
                }
            };
            if src != self.server_address {
                info!("Got message from invalid source {}", src);
                continue;
            }

            if let Some(msg) = Message::parse(&buffer[..len]) {
                match msg {
                    Message::ServerHello(id, secret) => unimplemented!(),
                    Message::Ping(bytes) => chk(self.send(Message::Pong(bytes))),
                    Message::Pong(bytes) => unimplemented!(),
                    Message::EntityUpdate(id, bytes) => unimplemented!(),
                    Message::EntityRemove(id) => unimplemented!(),
                    Message::ClientHello => warn!("Invalid message"),
                }
            } else {
                warn!("Invalid message");
            }
        }
    }
}
