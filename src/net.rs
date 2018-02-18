//! Network code.

use byteorder::{self, ReadBytesExt, WriteBytesExt};
use physics::{Position, Velocity};
use ship::Ship;
use specs::{Component, Entities, Fetch, HashMapStorage, Join, LazyUpdate,
            NullStorage, ReadStorage, System, VecStorage, WriteStorage};
use std::io::{self, Cursor};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_MESSAGES_PER_FRAME: u16 = 5;

type ORDER = byteorder::BigEndian;

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
    Ping(u32),
    /// Pong reply, with the bytes from the Ping request.
    Pong(u32),
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
                if msg.len() != 12 {
                    info!("Invalid Ping length");
                    None
                } else {
                    let buf = rdr.read_u32::<ORDER>().unwrap();
                    Some(Message::Ping(buf))
                }
            }
            b"po" => {
                if msg.len() != 12 {
                    info!("Invalid Pong length");
                    None
                } else {
                    let buf = rdr.read_u32::<ORDER>().unwrap();
                    Some(Message::Pong(buf))
                }
            }
            b"eu" => {
                if msg.len() < 16 {
                    info!("Invalid EntityUpdate length");
                    None
                } else {
                    Some(Message::EntityUpdate(
                        rdr.read_u64::<ORDER>().unwrap(),
                        msg[16..].into(),
                    ))
                }
            }
            b"er" => {
                if msg.len() != 16 {
                    info!("Invalid EntityRemove length");
                    None
                } else {
                    Some(Message::EntityRemove(
                        rdr.read_u64::<ORDER>().unwrap(),
                    ))
                }
            }
            _ => None,
        }
    }

    /// Write a message into a vector of bytes.
    fn to_bytes(&self, msg: &mut Vec<u8>) {
        msg.extend_from_slice(b"SPAC\x00\x01");
        match self {
            &Message::ClientHello => msg.extend_from_slice(b"hc"),
            &Message::ServerHello(id, secret) => {
                msg.extend_from_slice(b"hs");
                msg.write_u64::<ORDER>(id).unwrap();
                msg.write_u64::<ORDER>(secret).unwrap();
                assert_eq!(msg.len(), 8 + 8 + 8);
            }
            &Message::Ping(buf) => {
                msg.extend_from_slice(b"pi");
                msg.write_u32::<ORDER>(buf).unwrap();
            }
            &Message::Pong(buf) => {
                msg.extend_from_slice(b"po");
                msg.write_u32::<ORDER>(buf).unwrap();
            }
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
    }

    /// Turn a message into bytes.
    fn bytes(&self) -> Vec<u8> {
        let mut msg: Vec<u8> = Vec::with_capacity(20);
        self.to_bytes(&mut msg);
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

impl Replicated {
    pub fn new() -> Replicated {
        Replicated {
            id: 0,
            last_send: 0,
        }
    }
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
    client_id: u64,
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
    next_client: u64,
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
            next_client: 1,
        }
    }

    /// Sends a message.
    fn send(&self, msg: Message, addr: &SocketAddr) -> io::Result<usize> {
        self.socket.send_to(&msg.bytes(), addr)
    }
}

impl<'a> System<'a> for SysNetServer {
    type SystemData = (
        Fetch<'a, LazyUpdate>,
        Entities<'a>,
        ReadStorage<'a, ConnectedClient>,
        WriteStorage<'a, Replicated>,
        WriteStorage<'a, Dirty>,
        ReadStorage<'a, Position>,
        ReadStorage<'a, Velocity>,
        WriteStorage<'a, Ship>,
    );

    fn run(
        &mut self,
        (
            lazy,
            entities,
            client,
            replicated,
            mut dirty,
            position,
            velocity,
            mut ship,
        ): Self::SystemData,
    ) {
        self.frame += 1;

        // Receive messages
        let mut messages = Vec::new();
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
            if len < 8 + 8 {
                info!("Invalid message from {}", src);
                continue;
            }
            let client_id = (&buffer[0..]).read_u64::<ORDER>().unwrap();

            if let Some(msg) = Message::parse(&buffer[8..len]) {
                match msg {
                    Message::ClientHello => {
                        warn!("Got ClientHello from {}", src);

                        // Create a ship for the new player
                        let newship = Ship::create(&entities, &lazy);
                        let client_id = self.next_client;
                        self.next_client += 1;
                        let now = SystemTime::now();
                        lazy.insert(
                            newship,
                            ConnectedClient {
                                address: src,
                                client_id: client_id,
                                ping: 0.0,
                                last_ping: now,
                                quota: 0,
                            },
                        );

                        // Compute replicated ID and send ServerHello
                        let ship_id = (newship.gen().id() as u64) << 32
                            | newship.id() as u64;
                        chk(self.send(
                            Message::ServerHello(client_id, ship_id),
                            &src,
                        ));

                        // Send initial Ping message
                        let d = now.duration_since(UNIX_EPOCH).unwrap();
                        let d = (d.as_secs() as u32).wrapping_shl(10)
                            | d.subsec_nanos().wrapping_shr(22);
                        chk(self.send(Message::Ping(d), &src));
                    }
                    Message::Ping(buf) => {
                        chk(self.send(Message::Pong(buf), &src))
                    }
                    Message::Pong(_) | Message::EntityUpdate(_, _) => {
                        messages.push((client_id, msg))
                    }
                    Message::ServerHello(_, _) | Message::EntityRemove(_) => {
                        info!("Invalid message from {}", src)
                    }
                }
            } else {
                info!("Invalid message from {}", src);
                continue;
            }
        }

        if messages.is_empty() {
            return;
        }

        // Handle messages
        for (ship, client) in (&mut ship, &client).join() {
            for &(ref client_id, ref msg) in messages.iter() {
                if client_id == &client.client_id {
                    // TODO: Update entity from message
                    ship.want_thrust[1] = 1.0;
                }
            }
        }

        // TODO: Go over Replicated+(Dirty), send messages
        for (ent, ship, pos, vel) in
            (&*entities, &ship, &position, &velocity).join()
        {
            // Send an update if dirty, or if it hasn't been updated in a while
            if dirty.get(ent).is_some() {}
        }

        dirty.clear();
    }
}

/// Network client system.
///
/// Sends controls to server and gets game updates.
pub struct SysNetClient {
    socket: UdpSocket,
    server_address: SocketAddr,
    client_id: u64,
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
            client_id: 0,
        };
        client.send(Message::ClientHello).unwrap();
        client
    }

    /// Sends a message
    fn send(&self, msg: Message) -> io::Result<usize> {
        let mut bytes = Vec::new();
        bytes.write_u64::<ORDER>(self.client_id).unwrap();
        msg.to_bytes(&mut bytes);
        self.socket.send_to(&bytes, &self.server_address)
    }
}

impl<'a> System<'a> for SysNetClient {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, Replicated>,
        WriteStorage<'a, Dirty>,
        WriteStorage<'a, Position>,
        WriteStorage<'a, Velocity>,
        WriteStorage<'a, Ship>,
    );

    fn run(
        &mut self,
        (
            entities,
            replicated,
            mut dirty,
            mut position,
            mut velocity,
            mut ship,
        ): Self::SystemData,
    ) {
        // Receive messages
        let mut messages = Vec::new();
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
                    Message::ServerHello(client_id, ship_id) => {}
                    Message::Ping(buf) => chk(self.send(Message::Pong(buf))),
                    Message::Pong(_)
                    | Message::EntityUpdate(_, _)
                    | Message::EntityRemove(_) => messages.push(msg),
                    Message::ClientHello => warn!("Invalid message"),
                }
            } else {
                warn!("Invalid message");
            }
        }

        // TODO: Go over Replicated, update with messages
        for (ent, repli) in (&*entities, &replicated).join() {
            for msg in messages.iter() {
                // TODO: Update entity from message
                if let Some(ship) = ship.get_mut(ent) {
                    ship.thrust[1] = 1.0;
                }
            }
        }

        // TODO: Go over Dirty, send messages
        for (ship, _) in (&ship, &dirty).join() {
            // TODO: Send message
        }

        dirty.clear();
    }
}
