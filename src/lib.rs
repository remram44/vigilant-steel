extern crate byteorder;
#[macro_use]
extern crate log;
extern crate rand;
extern crate specs;
extern crate vecmath;

pub mod asteroid;
pub mod blocks;
pub mod input;
#[cfg(feature = "network")]
pub mod net;
pub mod particles;
pub mod physics;
mod sat;
pub mod ship;
mod tree;
pub mod utils;

use asteroid::{Asteroid, SysAsteroid};
use blocks::Blocky;
use input::{Input, Press};
use particles::{Effect, Particle, SysParticles};
use physics::{DeltaTime, DetectCollision, Hits, LocalControl, Position,
              SysCollision, SysSimu, Velocity};
use ship::{Projectile, Ship, SysProjectile, SysShip};
use specs::{Dispatcher, DispatcherBuilder, LazyUpdate, World};
#[cfg(feature = "network")]
use std::net::SocketAddr;
use std::ops::Deref;

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
        match *self {
            Role::Standalone => true,
            Role::Server => true,
            Role::Client => false,
        }
    }

    /// Whether the local machine is showing the world graphically.
    ///
    /// If this is false, there is no point bothering about animations or
    /// particles that don't affect the game, since no one will see them.
    pub fn graphical(&self) -> bool {
        match *self {
            Role::Standalone => true,
            Role::Server => false,
            Role::Client => true,
        }
    }

    /// Whether the game is networked.
    ///
    /// If this is false, there is no need for any networking.
    pub fn networked(&self) -> bool {
        match *self {
            Role::Standalone => false,
            Role::Server => true,
            Role::Client => true,
        }
    }
}

pub struct Clock {
    time_wrapping: f64,
}

impl Clock {
    fn new() -> Clock {
        Clock { time_wrapping: 0.0 }
    }

    fn advance_frame(&mut self, dt: f64) {
        self.time_wrapping += dt;
        if self.time_wrapping > 1024.0 {
            self.time_wrapping -= 1024.0;
        }
    }

    pub fn seconds_since(&self, past: &Clock) -> f64 {
        let d = self.time_wrapping - past.time_wrapping;
        if d < 0.0 {
            d + 1024.0
        } else {
            d
        }
    }
}

impl Deref for Clock {
    type Target = f64;

    fn deref(&self) -> &f64 {
        &self.time_wrapping
    }
}

/// The game structure, containing globals not specific to frontend.
pub struct Game {
    pub world: World,
    pub dispatcher: Dispatcher<'static, 'static>,
}

impl Game {
    fn new_common<'a, 'b>(role: Role) -> (World, DispatcherBuilder<'a, 'b>) {
        let mut world = World::new();
        world.register::<Position>();
        world.register::<Velocity>();
        world.register::<Blocky>();
        world.register::<DetectCollision>();
        world.register::<Hits>();
        world.register::<LocalControl>();
        world.register::<Ship>();
        world.register::<Projectile>();
        world.register::<Asteroid>();
        world.register::<Particle>();
        world.register::<Effect>();
        #[cfg(feature = "network")]
        {
            world.register::<net::Replicated>();
            world.register::<net::Dirty>();
            world.register::<net::Delete>();
            world.register::<net::ClientControlled>();
        }

        world.add_resource(DeltaTime(0.0));
        world.add_resource(Clock::new());
        world.add_resource(Input::new());
        world.add_resource(role);

        let mut dispatcher =
            DispatcherBuilder::new().add(SysSimu, "simu", &[]);
        if role.authoritative() {
            dispatcher = dispatcher
                .add(SysCollision, "collision", &[])
                .add(SysAsteroid::new(), "asteroid", &[])
                .add(SysProjectile, "projectile", &[])
                .add(SysParticles, "particles", &[]);
        }
        dispatcher = dispatcher.add(SysShip, "ship", &[]);

        (world, dispatcher)
    }

    pub fn new_standalone() -> Game {
        let (world, dispatcher) = Self::new_common(Role::Standalone);

        let ship = Ship::create(
            &world.entities(),
            &world.read_resource::<LazyUpdate>(),
        );
        world.write::<LocalControl>().insert(ship, LocalControl);

        Game {
            world: world,
            dispatcher: dispatcher.build(),
        }
    }

    #[cfg(feature = "network")]
    pub fn new_server(port: u16) -> Game {
        let (world, mut dispatcher) = Self::new_common(Role::Server);

        dispatcher =
            dispatcher.add(net::SysNetServer::new(port), "netserver", &[]);

        Game {
            world: world,
            dispatcher: dispatcher.build(),
        }
    }

    #[cfg(feature = "network")]
    pub fn new_client(address: SocketAddr) -> Game {
        let (world, mut dispatcher) = Self::new_common(Role::Client);

        dispatcher =
            dispatcher.add(net::SysNetClient::new(address), "netclient", &[]);

        Game {
            world: world,
            dispatcher: dispatcher.build(),
        }
    }

    pub fn update(&mut self, dt: f64) {
        {
            let mut r_dt = self.world.write_resource::<DeltaTime>();
            *r_dt = DeltaTime(dt);
            let mut r_clock = self.world.write_resource::<Clock>();
            r_clock.advance_frame(dt);
        }
        self.dispatcher.dispatch(&self.world.res);
        self.world.maintain();

        let mut input = self.world.write_resource::<Input>();
        if input.fire == Press::PRESSED {
            input.fire = Press::KEPT;
        }
    }
}
