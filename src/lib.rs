//! vigilant steel game.
//!
//! This crate contains all the game logic, while the frontend lives
//! separately.
//!
//! # List of modules
//!
//! * `lib.rs`: this file. Contains the `Game` structure, and the constructors
//! that initialize the game.
//! * `physics.rs`: base components and logic for the physic simulation:
//! `Position`, `Velocity`, `Hits`... Integrates positions, finds collisions.
//! * `asteroid.rs`: system spawning asteroids, deleting them when they fall
//! off.

extern crate byteorder;
#[macro_use]
extern crate log;
extern crate rand;
extern crate specs;
extern crate vecmath;

pub mod asteroid;
pub mod blocks;
pub mod guns;
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
use guns::{Projectile, SysProjectile};
use input::Input;
use particles::{Effect, Particle, SysParticles};
use physics::{DeltaTime, DetectCollision, Hits, LocalControl, Position,
              SysCollision, SysSimu, Velocity};
use ship::{Ship, SysShip};
use specs::{Dispatcher, DispatcherBuilder, Entity, Join, LazyUpdate, World,
            WorldExt};
use std::collections::HashMap;
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

impl Default for Role {
    fn default() -> Role {
        Role::Standalone
    }
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

/// Game clock, available as a resource.
///
/// This is used to trigger and time things around the game. It wraps so as to
/// preserve resolution, be aware of it when doing computations (or use
/// `seconds_since()`).
#[derive(Default)]
pub struct Clock {
    time_wrapping: f64,
}

impl Clock {
    /// Called by `Game` to move to the next frame.
    fn advance_frame(&mut self, dt: f64) {
        self.time_wrapping += dt;
        if self.time_wrapping > 1024.0 {
            self.time_wrapping -= 1024.0;
        }
    }

    /// Compute the difference between two points in time, aware of
    /// wrapping.
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

        world.insert::<DeltaTime>(Default::default());
        world.insert::<Clock>(Default::default());
        world.insert::<Input>(Default::default());
        world.insert(role);

        let dispatcher = if role.authoritative() {
            DispatcherBuilder::new()
                .with(SysSimu, "simu", &[])
                .with(SysProjectile, "projectile", &[])
                .with(SysAsteroid, "asteroid", &[])
                .with(SysShip, "ship", &[])
                .with(SysParticles, "particles", &[])
                .with(
                    SysCollision,
                    "collision",
                    &["projectile", "asteroid", "ship"],
                )
        } else {
            DispatcherBuilder::new()
                .with(SysSimu, "simu", &[])
                .with(SysShip, "ship", &[])
                .with(SysParticles, "particles", &[])
        };

        (world, dispatcher)
    }

    pub fn new_standalone() -> Game {
        let (world, dispatcher) = Self::new_common(Role::Standalone);

        let ship = Ship::create(
            &world.entities(),
            &world.read_resource::<LazyUpdate>().into(),
        );
        world
            .write_component::<LocalControl>()
            .insert(ship, LocalControl).unwrap();

        Game {
            world: world,
            dispatcher: dispatcher.build(),
        }
    }

    #[cfg(feature = "network")]
    pub fn new_server(port: u16) -> Game {
        let (world, mut dispatcher) = Self::new_common(Role::Server);

        dispatcher =
            dispatcher.with(net::SysNetServer::new(port), "netserver", &[]);

        Game {
            world: world,
            dispatcher: dispatcher.build(),
        }
    }

    #[cfg(feature = "network")]
    pub fn new_client(address: SocketAddr) -> Game {
        let (world, mut dispatcher) = Self::new_common(Role::Client);

        dispatcher = dispatcher.with(
            net::SysNetClient::new(address),
            "netclient",
            &[],
        );

        Game {
            world: world,
            dispatcher: dispatcher.build(),
        }
    }

    /// Update the world using `specs`.
    pub fn update(&mut self, dt: f64) {
        {
            let mut r_dt = self.world.write_resource::<DeltaTime>();
            *r_dt = DeltaTime(dt);
            let mut r_clock = self.world.write_resource::<Clock>();
            r_clock.advance_frame(dt);
        }
        self.dispatcher.dispatch(&self.world);
        self.world.maintain();

        let mut input = self.world.write_resource::<Input>();
        input.update();
    }

    /// Print out entity counts as `INFO`.
    pub fn profile(&self) {
        macro_rules! component_check {
            ($x:ident) => {
                (stringify!($x), {
                    let s = self.world.read_component::<$x>();
                    Box::new(move |e| s.get(e).is_some())
                        as Box<Fn(Entity) -> bool>
                })
            };
        }
        let components = &[
            component_check!(Position),
            component_check!(Velocity),
            component_check!(Blocky),
            component_check!(DetectCollision),
            component_check!(Hits),
            component_check!(LocalControl),
            component_check!(Ship),
            component_check!(Projectile),
            component_check!(Asteroid),
            component_check!(Particle),
            component_check!(Effect),
        ];
        let mut counts = HashMap::new();
        for ent in (&*self.world.entities()).join() {
            let mut i = 1;
            let mut f = 0;
            for &(_, ref comp) in components {
                if comp(ent) {
                    f |= i;
                }
                i = i << 1;
            }
            *counts.entry(f).or_insert(0) += 1;
        }
        for (f, c) in &counts {
            let mut comp = String::new();
            let mut i = 1;
            for &(name, _) in components {
                if f & i != 0 {
                    if !comp.is_empty() {
                        comp.push_str(", ");
                    }
                    comp.push_str(name);
                }
                i = i << 1;
            }
            info!("{:>4} | {}", c, comp);
        }
    }
}
