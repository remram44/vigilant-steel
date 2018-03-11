//! Network code.

use asteroid::Asteroid;
use byteorder::{self, ReadBytesExt, WriteBytesExt};
use particles::Effect;
use physics::{LocalControl, Position, Velocity};
use ship::{Projectile, ProjectileType, Ship};
use specs::{Component, Entities, Fetch, HashMapStorage, Join, LazyUpdate,
            NullStorage, ReadStorage, System, VecStorage, WriteStorage};
use std::collections::{HashMap, HashSet};
use std::io::{self, Cursor, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

type ORDER = byteorder::BigEndian;

fn time_encode(d: Duration) -> u32 {
    (d.as_secs() as u32).wrapping_shl(10) | d.subsec_nanos().wrapping_shr(22)
}

fn time_decode(b: u32) -> Duration {
    let secs = (b as u64).wrapping_shr(10);
    let nanos = b.wrapping_shl(22);
    Duration::new(secs, nanos)
}

fn write_float<W: io::Write>(mut writer: W, v: f64) {
    let v = v as f32;
    assert_eq!(
        writer
            .write(&unsafe { ::std::mem::transmute::<f32, [u8; 4]>(v) })
            .unwrap(),
        4
    );
}

fn read_float<R: io::Read>(mut reader: R) -> f64 {
    let mut v = [0u8; 4];
    assert_eq!(reader.read(&mut v).unwrap(), 4);
    let v = unsafe { ::std::mem::transmute::<[u8; 4], f32>(v) };
    v as f64
}

/// The message exchanged by server and clients.
enum Message {
    /// Message sent by a client to introduce itself.
    ///
    /// The server will reply with ServerHello.
    ClientHello,
    /// Message sent by the server to accept a client, and assign it a client
    /// ID.
    ServerHello(u64),
    /// Ping request, other side should send bytes back as Pong.
    Ping(u32),
    /// Pong reply, with the bytes from the Ping request.
    Pong(u32),
    /// Message sent by the server to give the client an entity to
    /// control.
    StartEntityControl(u64),
    /// Entity update, from either side.
    ///
    /// The server sends full entity updates that the client applies. The
    /// client sends update to the controls, preceded by its secret.
    EntityUpdate(u64, Vec<u8>),
    /// Entity deleted, from server.
    EntityDelete(u64),
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
                if msg.len() != 8 + 8 {
                    info!("Invalid ServerHello length");
                    None
                } else {
                    Some(Message::ServerHello(
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
            b"ec" => {
                if msg.len() != 8 + 8 {
                    info!("Invalid StartEntityControl length");
                    None
                } else {
                    Some(Message::StartEntityControl(
                        rdr.read_u64::<ORDER>().unwrap(),
                    ))
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
                    info!("Invalid EntityDelete length");
                    None
                } else {
                    Some(Message::EntityDelete(
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
        match *self {
            Message::ClientHello => msg.extend_from_slice(b"hc"),
            Message::ServerHello(id) => {
                msg.extend_from_slice(b"hs");
                msg.write_u64::<ORDER>(id).unwrap();
                assert_eq!(msg.len(), 8 + 8);
            }
            Message::Ping(buf) => {
                msg.extend_from_slice(b"pi");
                msg.write_u32::<ORDER>(buf).unwrap();
            }
            Message::Pong(buf) => {
                msg.extend_from_slice(b"po");
                msg.write_u32::<ORDER>(buf).unwrap();
            }
            Message::StartEntityControl(id) => {
                msg.extend_from_slice(b"ec");
                msg.write_u64::<ORDER>(id).unwrap();
            }
            Message::EntityUpdate(id, ref bytes) => {
                msg.extend_from_slice(b"eu");
                msg.write_u64::<ORDER>(id).unwrap();
                msg.extend_from_slice(bytes);
            }
            Message::EntityDelete(id) => {
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
    last_update: u32,
}

impl Replicated {
    pub fn new() -> Replicated {
        Replicated {
            id: 0,
            last_update: 0,
        }
    }
}

impl Component for Replicated {
    type Storage = VecStorage<Self>;
}

/// Mark an entity to be deleted everywhere.
#[derive(Default)]
pub struct Delete;

impl Component for Delete {
    type Storage = NullStorage<Self>;
}

/// Flag that marks an entity as dirty, eg needs to be sent to clients.
#[derive(Default)]
pub struct Dirty;

impl Component for Dirty {
    type Storage = NullStorage<Self>;
}

pub struct ConnectedClient {
    address: SocketAddr,
    client_id: u64,
    ping: f64,
    last_pong: SystemTime,
}

/// Server component attached to entities controlled by clients.
///
/// Multiple entities can be controlled by the same client, and that's fine.
pub struct ClientControlled {
    client_id: u64,
}

impl Component for ClientControlled {
    type Storage = HashMapStorage<Self>;
}

/// Network server system.
///
/// Gets controls from clients and sends game updates.
pub struct SysNetServer {
    socket: UdpSocket,
    frame: u32,
    next_client: u64,
    clients: HashMap<u64, ConnectedClient>,
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
            clients: HashMap::new(),
        }
    }

    /// Sends a message.
    fn send(&self, msg: &Message, addr: &SocketAddr) -> io::Result<usize> {
        self.socket.send_to(&msg.bytes(), addr)
    }
}

impl<'a> System<'a> for SysNetServer {
    type SystemData = (
        Fetch<'a, LazyUpdate>,
        Entities<'a>,
        ReadStorage<'a, ClientControlled>,
        WriteStorage<'a, Replicated>,
        WriteStorage<'a, Dirty>,
        ReadStorage<'a, Delete>,
        ReadStorage<'a, Position>,
        ReadStorage<'a, Velocity>,
        WriteStorage<'a, Ship>,
        ReadStorage<'a, Asteroid>,
        ReadStorage<'a, Projectile>,
        ReadStorage<'a, Effect>,
    );

    fn run(
        &mut self,
        (
            lazy,
            entities,
            ctrl,
            mut replicated,
            mut dirty,
            delete,
            position,
            velocity,
            mut ship,
            asteroid,
            projectile,
            effects,
        ): Self::SystemData,
    ) {
        self.frame = self.frame.wrapping_add(1);

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

                        // Create a client
                        let client_id = self.next_client;
                        self.next_client += 1;
                        let now = SystemTime::now();
                        self.clients.insert(
                            client_id,
                            ConnectedClient {
                                address: src,
                                client_id: client_id,
                                ping: 0.0,
                                last_pong: now,
                            },
                        );

                        // Send ServerHello
                        chk(self.send(&Message::ServerHello(client_id), &src));

                        // Create a ship for the new player
                        let newship = Ship::create(&entities, &lazy);
                        lazy.insert(
                            newship,
                            ClientControlled {
                                client_id: client_id,
                            },
                        );
                        let ship_id = (newship.gen().id() as u64) << 32
                            | newship.id() as u64;
                        chk(self.send(
                            &Message::StartEntityControl(ship_id),
                            &src,
                        ));

                        info!(
                            "Created Ship {} for new client {}",
                            ship_id, client_id
                        );

                        // Send initial Ping message
                        let d = now.duration_since(UNIX_EPOCH).unwrap();
                        let d = time_encode(d);
                        chk(self.send(&Message::Ping(d), &src));
                    }
                    Message::Ping(buf) => {
                        chk(self.send(&Message::Pong(buf), &src))
                    }
                    Message::Pong(_) | Message::EntityUpdate(_, _) => {
                        messages.push((client_id, msg))
                    }
                    Message::ServerHello(_)
                    | Message::StartEntityControl(_)
                    | Message::EntityDelete(_) => {
                        info!("Invalid message from {}", src)
                    }
                }
            } else {
                info!("Invalid message from {}", src);
                continue;
            }
        }

        // Handle Pong from clients
        for client in self.clients.values_mut() {
            for &(ref client_id, ref msg) in &messages {
                if client_id != &client.client_id {
                    continue;
                }

                if let Message::Pong(d) = *msg {
                    let d = time_decode(d);
                    let now = SystemTime::now();
                    let now_d = now.duration_since(UNIX_EPOCH).unwrap();
                    if let Some(d) = now_d.checked_sub(d) {
                        client.last_pong = now;
                        client.ping = d.as_secs() as f64
                            + d.subsec_nanos() as f64 / 0.000_000_001;
                    }
                }
            }

            // TODO: Drop old clients
        }

        // Go over entities, send updates
        for (ent, mut repli) in (&*entities, &mut replicated).join() {
            // Assign replicated object ID
            if repli.id == 0 {
                repli.id = (ent.gen().id() as u64) << 32 | ent.id() as u64;
            }

            // Deleted?
            if delete.get(ent).is_some() {
                let message = Message::EntityDelete(repli.id).bytes();
                for client in self.clients.values_mut() {
                    chk(self.socket.send_to(&message, &client.address));
                }
                entities.delete(ent).unwrap();
                continue;
            }

            // Send an update if dirty, or if it hasn't been updated in a while
            if dirty.get(ent).is_none()
                && self.frame.wrapping_sub(repli.last_update) < 200
            {
                continue;
            }

            // Send entity update
            let mut data;
            if let Some(ship) = ship.get(ent) {
                let pos = position.get(ent).unwrap();
                let vel = velocity.get(ent).unwrap();
                data = Vec::with_capacity(48);
                write_float(&mut data, pos.pos[0]);
                write_float(&mut data, pos.pos[1]);
                write_float(&mut data, pos.rot);
                write_float(&mut data, vel.vel[0]);
                write_float(&mut data, vel.vel[1]);
                write_float(&mut data, vel.rot);
                write_float(&mut data, ship.want_thrust[0]);
                write_float(&mut data, ship.want_thrust[1]);
                write_float(&mut data, ship.want_thrust_rot);
                write_float(&mut data, ship.thrust[0]);
                write_float(&mut data, ship.thrust[1]);
                write_float(&mut data, ship.thrust_rot);
                assert_eq!(data.len(), 48);
            } else if asteroid.get(ent).is_some() {
                let pos = position.get(ent).unwrap();
                let vel = velocity.get(ent).unwrap();
                data = Vec::with_capacity(24);
                write_float(&mut data, pos.pos[0]);
                write_float(&mut data, pos.pos[1]);
                write_float(&mut data, pos.rot);
                write_float(&mut data, vel.vel[0]);
                write_float(&mut data, vel.vel[1]);
                write_float(&mut data, vel.rot);
                assert_eq!(data.len(), 24);
            } else if let Some(proj) = projectile.get(ent) {
                let pos = position.get(ent).unwrap();
                let vel = velocity.get(ent).unwrap();
                data = Vec::with_capacity(25);
                write_float(&mut data, pos.pos[0]);
                write_float(&mut data, pos.pos[1]);
                write_float(&mut data, pos.rot);
                write_float(&mut data, vel.vel[0]);
                write_float(&mut data, vel.vel[1]);
                write_float(&mut data, vel.rot);
                let kind = match proj.0 {
                    ProjectileType::Plasma => 1,
                    ProjectileType::Rail => 2,
                };
                assert_eq!(data.write(&[0u8]).unwrap(), kind);
                assert_eq!(data.len(), 25);
            } else {
                panic!("Need to send update for unknown entity!");
            }
            let update = Message::EntityUpdate(repli.id, data).bytes();
            for client in self.clients.values_mut() {
                chk(self.socket.send_to(&update, &client.address));
            }

            repli.last_update = self.frame;
        }

        // Send particle effects
        for (effect, _) in (&effects, &dirty).join() {
            // TODO: Send particle effects
        }

        dirty.clear();

        // Handle messages
        for (ent, ship, repli, ctrl) in
            (&*entities, &mut ship, &mut replicated, &ctrl).join()
        {
            for &(ref client_id, ref msg) in &messages {
                if let Message::EntityUpdate(id, ref data) = *msg {
                    if repli.id == id && client_id == &ctrl.client_id {
                        repli.last_update = self.frame;

                        // Update entity from message data
                        if data.len() != 1 {
                            info!("Invalid ship control update");
                            continue;
                        }
                        let data = data[0];
                        ship.want_fire = data & 0x01 == 0x01;
                        ship.want_thrust[0] = match data & 0x06 {
                            0x02 => 1.0,
                            0x04 => -1.0,
                            _ => 0.0,
                        };
                        ship.want_thrust[1] =
                            if data & 0x08 == 0x08 { 1.0 } else { 0.0 };
                        ship.want_thrust_rot = match data & 0x30 {
                            0x10 => 1.0,
                            0x20 => -1.0,
                            _ => 0.0,
                        };

                        lazy.insert(ent, Dirty);
                    }
                }
            }
        }
    }
}

/// Network client system.
///
/// Sends controls to server and gets game updates.
pub struct SysNetClient {
    socket: UdpSocket,
    server_address: SocketAddr,
    client_id: u64,
    last_pong: SystemTime,
    ping: f64,
    controlled_entities: HashSet<u64>,
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
            last_pong: SystemTime::now(),
            ping: 0.0,
            controlled_entities: HashSet::new(),
        };
        client.send(&Message::ClientHello).unwrap();
        client
    }

    /// Sends a message
    fn send(&self, msg: &Message) -> io::Result<usize> {
        let mut bytes = Vec::new();
        bytes.write_u64::<ORDER>(self.client_id).unwrap();
        msg.to_bytes(&mut bytes);
        self.socket.send_to(&bytes, &self.server_address)
    }
}

impl<'a> System<'a> for SysNetClient {
    type SystemData = (
        Entities<'a>,
        Fetch<'a, LazyUpdate>,
        ReadStorage<'a, Replicated>,
        WriteStorage<'a, Dirty>,
        WriteStorage<'a, Position>,
        WriteStorage<'a, Velocity>,
        WriteStorage<'a, Ship>,
        ReadStorage<'a, Asteroid>,
        ReadStorage<'a, Projectile>,
    );

    fn run(
        &mut self,
        (
            entities,
            lazy,
            replicated,
            mut dirty,
            mut position,
            mut velocity,
            mut ship,
            asteroid,
            projectile,
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
                    Message::ServerHello(client_id) => {
                        warn!("Got ServerHello, our ID is {}", client_id);
                        self.client_id = client_id;
                    }
                    Message::Ping(buf) => chk(self.send(&Message::Pong(buf))),
                    Message::Pong(d) => {
                        let d = time_decode(d);
                        let now = SystemTime::now();
                        let now_d = now.duration_since(UNIX_EPOCH).unwrap();
                        if let Some(d) = now_d.checked_sub(d) {
                            self.last_pong = now;
                            self.ping = d.as_secs() as f64
                                + d.subsec_nanos() as f64 / 0.000_000_001;
                        }
                    }
                    Message::StartEntityControl(id) => {
                        self.controlled_entities.insert(id);
                    }
                    Message::EntityUpdate(_, _) | Message::EntityDelete(_) => {
                        messages.push((msg, false))
                    }
                    Message::ClientHello => warn!("Invalid message"),
                }
            } else {
                warn!("Invalid message");
            }
        }

        // Update entities from messages
        for (ent, repli, mut pos, mut vel) in
            (&*entities, &replicated, &mut position, &mut velocity).join()
        {
            for &mut (ref msg, ref mut handled) in &mut messages {
                if let Message::EntityUpdate(id, ref data) = *msg {
                    if id != repli.id {
                        continue;
                    }

                    *handled = true;

                    // Update entity from message
                    if let Some(ship) = ship.get_mut(ent) {
                        assert_eq!(data.len(), 48);
                        let mut data = Cursor::new(data);
                        pos.pos[0] = read_float(&mut data);
                        pos.pos[1] = read_float(&mut data);
                        pos.rot = read_float(&mut data);
                        vel.vel[0] = read_float(&mut data);
                        vel.vel[1] = read_float(&mut data);
                        vel.rot = read_float(&mut data);
                        ship.want_thrust[0] = read_float(&mut data);
                        ship.want_thrust[1] = read_float(&mut data);
                        ship.want_thrust_rot = read_float(&mut data);
                        ship.thrust[0] = read_float(&mut data);
                        ship.thrust[1] = read_float(&mut data);
                        ship.thrust_rot = read_float(&mut data);
                        assert_eq!(data.position(), 48);
                    } else if asteroid.get(ent).is_some() {
                        assert_eq!(data.len(), 24);
                        let mut data = Cursor::new(data);
                        pos.pos[0] = read_float(&mut data);
                        pos.pos[1] = read_float(&mut data);
                        pos.rot = read_float(&mut data);
                        vel.vel[0] = read_float(&mut data);
                        vel.vel[1] = read_float(&mut data);
                        vel.rot = read_float(&mut data);
                        assert_eq!(data.position(), 24);
                    } else if projectile.get(ent).is_some() {
                        assert_eq!(data.len(), 25);
                        let mut data = Cursor::new(data);
                        pos.pos[0] = read_float(&mut data);
                        pos.pos[1] = read_float(&mut data);
                        pos.rot = read_float(&mut data);
                        vel.vel[0] = read_float(&mut data);
                        vel.vel[1] = read_float(&mut data);
                        vel.rot = read_float(&mut data);
                        assert_eq!(data.position(), 24);
                    } else {
                        panic!("Got update for unknown entity!");
                    }
                } else if let Message::EntityDelete(id) = *msg {
                    if id != repli.id {
                        continue;
                    }

                    // Delete entity
                    entities.delete(ent).unwrap();
                }
            }
        }

        // Create new entities
        for &(ref msg, handled) in &messages {
            if handled {
                continue;
            }
            if let Message::EntityUpdate(id, ref data) = *msg {
                if data.len() == 48 {
                    let mut data = Cursor::new(data);
                    let pos = Position {
                        pos: [read_float(&mut data), read_float(&mut data)],
                        rot: read_float(&mut data),
                    };
                    let vel = Velocity {
                        vel: [read_float(&mut data), read_float(&mut data)],
                        rot: read_float(&mut data),
                    };
                    let ship = Ship {
                        want_fire: false,
                        want_thrust: [
                            read_float(&mut data),
                            read_float(&mut data),
                        ],
                        want_thrust_rot: read_float(&mut data),
                        thrust: [read_float(&mut data), read_float(&mut data)],
                        thrust_rot: read_float(&mut data),
                    };
                    assert_eq!(data.position(), 48);

                    let entity = entities.create();
                    lazy.insert(entity, pos);
                    lazy.insert(entity, vel);
                    lazy.insert(entity, ship);
                    lazy.insert(
                        entity,
                        Replicated {
                            id: id,
                            last_update: 0,
                        },
                    );

                    // Maybe we control this?
                    if self.controlled_entities.contains(&id) {
                        warn!("Created locally-controlled ship {}", id);
                        lazy.insert(entity, LocalControl);
                    }
                } else if data.len() == 24 {
                    let mut data = Cursor::new(data);
                    let pos = Position {
                        pos: [read_float(&mut data), read_float(&mut data)],
                        rot: read_float(&mut data),
                    };
                    let vel = Velocity {
                        vel: [read_float(&mut data), read_float(&mut data)],
                        rot: read_float(&mut data),
                    };
                    assert_eq!(data.position(), 24);

                    let entity = entities.create();
                    lazy.insert(entity, pos);
                    lazy.insert(entity, vel);
                    lazy.insert(entity, Asteroid);
                    lazy.insert(
                        entity,
                        Replicated {
                            id: id,
                            last_update: 0,
                        },
                    );
                } else if data.len() == 25 {
                    let mut data = Cursor::new(data);
                    let pos = Position {
                        pos: [read_float(&mut data), read_float(&mut data)],
                        rot: read_float(&mut data),
                    };
                    let vel = Velocity {
                        vel: [read_float(&mut data), read_float(&mut data)],
                        rot: read_float(&mut data),
                    };
                    let kind = match data.read_u8().unwrap() {
                        1 => ProjectileType::Plasma,
                        2 => ProjectileType::Rail,
                        _ => panic!("Got unknown projectile type"),
                    };
                    assert_eq!(data.position(), 25);

                    let entity = entities.create();
                    lazy.insert(entity, pos);
                    lazy.insert(entity, vel);
                    lazy.insert(entity, Projectile(kind));
                    lazy.insert(
                        entity,
                        Replicated {
                            id: id,
                            last_update: 0,
                        },
                    );
                } else {
                    panic!("Need to create unknown entity!");
                }
            }
        }

        // TODO: Materialize particle effects

        // Go over Dirty, send messages
        for (ship, repli, _) in (&ship, &replicated, &dirty).join() {
            let mut data = 0;
            if ship.want_fire {
                data |= 0x01;
            }
            if ship.want_thrust[0] > 0.5 {
                data |= 0x02;
            } else if ship.want_thrust[0] < -0.5 {
                data |= 0x04;
            }
            if ship.want_thrust[1] > 0.5 {
                data |= 0x08;
            }
            if ship.want_thrust_rot > 0.5 {
                data |= 0x10;
            } else if ship.want_thrust_rot < -0.5 {
                data |= 0x20;
            }
            chk(self.send(&Message::EntityUpdate(repli.id, vec![data])))
        }

        dirty.clear();
    }
}
