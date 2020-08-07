//! Network code.

mod base;
pub mod udp;
#[cfg(feature = "websocket")]
pub mod websocket;

use byteorder::{self, ReadBytesExt, WriteBytesExt};
use log::{info, warn};
use specs::{Entities, Read, Join, LazyUpdate, ReadStorage, System,
            WriteStorage};
use std::collections::{HashMap, HashSet};
use std::fmt::{self, Display};
use std::hash::Hash;
use std::io::{self, Cursor, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::asteroid::Asteroid;
use crate::guns::{Projectile, ProjectileType};
use crate::particles::Effect;
use crate::physics::{LocalControl, Position, Velocity};
use crate::ship::Ship;

pub use self::base::{Replicated, Delete, Dirty, ClientControlled};

type ORDER = byteorder::BigEndian;

fn time_encode(d: Duration) -> u32 {
    (d.as_secs() as u32).wrapping_shl(10) | d.subsec_nanos().wrapping_shr(22)
}

fn time_decode(b: u32) -> Duration {
    let secs = (b as u64).wrapping_shr(10);
    let nanos = b.wrapping_shl(22);
    Duration::new(secs, nanos)
}

fn write_float<W: io::Write>(mut writer: W, v: f32) {
    let v = v as f32;
    assert_eq!(
        writer
            .write(&unsafe { ::std::mem::transmute::<f32, [u8; 4]>(v) })
            .unwrap(),
        4
    );
}

fn read_float<R: io::Read>(mut reader: R) -> f32 {
    let mut v = [0u8; 4];
    assert_eq!(reader.read(&mut v).unwrap(), 4);
    let v = unsafe { ::std::mem::transmute::<[u8; 4], f32>(v) };
    v as f32
}

/// The message exchanged by server and clients.
enum Message {
    /// Message sent by a client to introduce itself.
    ///
    /// The server will reply with ServerHello.
    ClientHello,
    /// Message sent by the server to accept a client.
    ServerHello,
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
    /// client sends update to the controls.
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
                if msg.len() != 8 {
                    info!("Invalid ServerHello length");
                    None
                } else {
                    Some(Message::ServerHello)
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
            Message::ServerHello => {
                msg.extend_from_slice(b"hs");
                assert_eq!(msg.len(), 8);
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

// TODO: Get rid of that, log somewhere else and drop connection
/// Warns if a Result is an error.
fn chk<T>(res: Result<T, NetError>) {
    match res {
        Ok(_) => {}
        Err(e) => warn!("Network error: {:?}", e),
    }
}

pub enum NetError {
    Disconnected,
    FlowControl,
}

impl fmt::Debug for NetError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            NetError::Disconnected => write!(f, "Disconnected"),
            NetError::FlowControl => write!(f, "Flow control"),
        }
    }
}

pub trait Server: Send + 'static {
    type Address: Clone + Display + Eq + Hash + Send;

    fn send(&self, msg: &[u8], addr: &Self::Address) -> Result<(), NetError>;
    fn recv(&mut self, buffer: &mut [u8]) -> Result<(usize, Self::Address), NetError>;
}

pub trait Client: Send + 'static {
    fn send(&self, msg: &[u8]) -> Result<(), NetError>;
    fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, NetError>;
}

pub struct ConnectedClient<A: Eq> {
    address: A,
    client_id: u64,
    ping: f32,
    last_pong: SystemTime,
}

/// Network server system.
///
/// Gets controls from clients and sends game updates.
pub struct SysNetServer<S: Server> {
    server: S,
    frame: u32,
    next_client: u64,
    clients: HashMap<S::Address, ConnectedClient<S::Address>>,
}

impl<S: Server> SysNetServer<S> {
    /// Create a server, listening on the given port.
    pub fn new(server: S) -> SysNetServer<S> {
        SysNetServer {
            server,
            frame: 0,
            next_client: 1,
            clients: HashMap::new(),
        }
    }

    /// Sends a message.
    fn send(&self, msg: &Message, addr: &S::Address) -> Result<(), NetError> {
        self.server.send(&msg.bytes(), addr)
    }
}

impl<'a, S: Server> System<'a> for SysNetServer<S> {
    type SystemData = (
        Read<'a, LazyUpdate>,
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
            let (len, src) = match self.server.recv(&mut buffer) {
                Ok(r) => r,
                Err(NetError::FlowControl) => break,
                Err(_) => {
                    warn!("Error reading from socket");
                    break;
                }
            };
            if len < 8 {
                info!("Invalid message from {}", src);
                continue;
            }

            if let Some(msg) = Message::parse(&buffer[0..len]) {
                match msg {
                    Message::ClientHello => {
                        warn!("Got ClientHello from {}", src);

                        // Create a client
                        let client_id = self.next_client;
                        self.next_client += 1;
                        let now = SystemTime::now();
                        self.clients.insert(
                            src.clone(),
                            ConnectedClient {
                                address: src.clone(),
                                client_id: client_id,
                                ping: 0.0,
                                last_pong: now,
                            },
                        );

                        // Send ServerHello
                        chk(self.send(&Message::ServerHello, &src));

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

                        warn!(
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
                    Message::Pong(_) => {
                        if let Some(client) = self.clients.get_mut(&src) {
                            if let Message::Pong(d) = msg {
                                let d = time_decode(d);
                                let now = SystemTime::now();
                                let now_d = now.duration_since(UNIX_EPOCH).unwrap();
                                if let Some(d) = now_d.checked_sub(d) {
                                    client.last_pong = now;
                                    client.ping = d.as_secs() as f32
                                        + d.subsec_nanos() as f32 / 0.000_000_001;
                                }
                            }
                        }
                    }
                    Message::EntityUpdate(_, _) => {
                        if let Some(client) = self.clients.get(&src) {
                            messages.push((client.client_id, msg));
                        }
                    }
                    Message::ServerHello
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

        // TODO: Drop old clients

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
                    chk(self.server.send(&message, &client.address));
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
                data = Vec::with_capacity(56);
                write_float(&mut data, pos.pos[0]);
                write_float(&mut data, pos.pos[1]);
                write_float(&mut data, pos.rot);
                write_float(&mut data, vel.vel[0]);
                write_float(&mut data, vel.vel[1]);
                write_float(&mut data, vel.rot);
                write_float(&mut data, ship.want_thrust[0]);
                write_float(&mut data, ship.want_thrust[1]);
                write_float(&mut data, ship.want_thrust_rot);
                write_float(&mut data, ship.want_target[0]);
                write_float(&mut data, ship.want_target[1]);
                write_float(&mut data, ship.thrust[0]);
                write_float(&mut data, ship.thrust[1]);
                write_float(&mut data, ship.thrust_rot);
                assert_eq!(data.len(), 56);
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
                let kind = match proj.kind {
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
                chk(self.server.send(&update, &client.address));
            }

            repli.last_update = self.frame;
        }

        // Send particle effects
        for (_effect, _) in (&effects, &dirty).join() {
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
                        if data.len() != 9 {
                            info!("Invalid ship control update");
                            continue;
                        }
                        let flags = data[0];
                        ship.want_fire = flags & 0x01 == 0x01;
                        ship.want_thrust[0] = match flags & 0x06 {
                            0x02 => 1.0,
                            0x04 => -1.0,
                            _ => 0.0,
                        };
                        ship.want_thrust[1] = if flags & 0x08 == 0x08 {
                            1.0
                        } else {
                            0.0
                        };
                        ship.want_thrust_rot = match flags & 0x30 {
                            0x10 => 1.0,
                            0x20 => -1.0,
                            _ => 0.0,
                        };
                        let mut data = Cursor::new(&data[1..]);
                        ship.want_target[0] = read_float(&mut data);
                        ship.want_target[1] = read_float(&mut data);
                        dirty.insert(ent, Dirty).unwrap();
                    }
                }
            }
        }
    }
}

/// Network client system.
///
/// Sends controls to server and gets game updates.
pub struct SysNetClient<C: Client> {
    client: C,
    last_pong: SystemTime,
    ping: f32,
    controlled_entities: HashSet<u64>,
}

impl<C: Client> SysNetClient<C> {
    /// Create a client, connected to the specified server.
    pub fn new(client: C) -> SysNetClient<C> {
        let client = SysNetClient {
            client,
            last_pong: SystemTime::now(),
            ping: 0.0,
            controlled_entities: HashSet::new(),
        };
        client.send(&Message::ClientHello).unwrap();
        client
    }

    /// Sends a message
    fn send(&self, msg: &Message) -> Result<(), NetError> {
        let mut bytes = Vec::new();
        msg.to_bytes(&mut bytes);
        self.client.send(&bytes)
    }
}

impl<'a, C: Client> System<'a> for SysNetClient<C> {
    type SystemData = (
        Entities<'a>,
        Read<'a, LazyUpdate>,
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
            let len = match self.client.recv(&mut buffer) {
                Ok(r) => r,
                Err(NetError::FlowControl) => break,
                Err(_) => {
                    warn!("Error reading from socket");
                    break;
                }
            };

            if let Some(msg) = Message::parse(&buffer[..len]) {
                match msg {
                    Message::ServerHello => warn!("Got ServerHello"),
                    Message::Ping(buf) => chk(self.send(&Message::Pong(buf))),
                    Message::Pong(d) => {
                        let d = time_decode(d);
                        let now = SystemTime::now();
                        let now_d = now.duration_since(UNIX_EPOCH).unwrap();
                        if let Some(d) = now_d.checked_sub(d) {
                            self.last_pong = now;
                            self.ping = d.as_secs() as f32
                                + d.subsec_nanos() as f32 / 0.000_000_001;
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
        for (ent, repli, mut pos, mut vel) in (
            &*entities,
            &replicated,
            &mut position,
            &mut velocity,
        ).join()
        {
            for &mut (ref msg, ref mut handled) in &mut messages {
                if let Message::EntityUpdate(id, ref data) = *msg {
                    if id != repli.id {
                        continue;
                    }

                    *handled = true;

                    // Update entity from message
                    if let Some(ship) = ship.get_mut(ent) {
                        assert_eq!(data.len(), 56);
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
                        ship.want_target[0] = read_float(&mut data);
                        ship.want_target[1] = read_float(&mut data);
                        ship.thrust[0] = read_float(&mut data);
                        ship.thrust[1] = read_float(&mut data);
                        ship.thrust_rot = read_float(&mut data);
                        assert_eq!(data.position(), 56);
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
                if data.len() == 56 {
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
                        want_target: [
                            read_float(&mut data),
                            read_float(&mut data),
                        ],
                        thrust: [read_float(&mut data), read_float(&mut data)],
                        thrust_rot: read_float(&mut data),
                    };
                    assert_eq!(data.position(), 56);

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
                    lazy.insert(
                        entity,
                        Projectile {
                            kind,
                            shooter: entity,
                        },
                    );
                    lazy.insert(
                        entity,
                        Replicated {
                            id: id,
                            last_update: 0,
                        },
                    );
                } else {
                    panic!(
                        "Need to create unknown entity! data {:?} (len {})",
                        &data[0..50],
                        data.len(),
                    );
                }
            }
        }

        // TODO: Materialize particle effects

        // Go over Dirty, send messages
        for (ship, repli, _) in (&ship, &replicated, &dirty).join() {
            let mut flags = 0;
            if ship.want_fire {
                flags |= 0x01;
            }
            if ship.want_thrust[0] > 0.5 {
                flags |= 0x02;
            } else if ship.want_thrust[0] < -0.5 {
                flags |= 0x04;
            }
            if ship.want_thrust[1] > 0.5 {
                flags |= 0x08;
            }
            if ship.want_thrust_rot > 0.5 {
                flags |= 0x10;
            } else if ship.want_thrust_rot < -0.5 {
                flags |= 0x20;
            }
            let mut data = Vec::with_capacity(9);
            data.write_u8(flags).unwrap();
            write_float(&mut data, ship.want_target[0]);
            write_float(&mut data, ship.want_target[1]);
            assert_eq!(data.len(), 9);
            chk(self.send(&Message::EntityUpdate(repli.id, data)))
        }

        dirty.clear();
    }
}
