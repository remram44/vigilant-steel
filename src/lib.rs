extern crate byteorder;
#[macro_use]
extern crate log;
extern crate rand;
extern crate specs;
extern crate vecmath;

pub mod asteroid;
pub mod input;
#[cfg(feature = "network")]
pub mod net;
pub mod physics;
pub mod ship;
pub mod utils;

use asteroid::{Asteroid, SysAsteroid};
use input::{Input, Press};
use physics::{Collided, Collision, DeltaTime, LocalControl, Position,
              SysCollision, SysSimu, Velocity};
use ship::{Projectile, Ship, SysProjectile, SysShip};
use specs::{Dispatcher, DispatcherBuilder, LazyUpdate, World};
#[cfg(feature = "network")]
use std::net::SocketAddr;

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

    /// Whether the game is networked.
    ///
    /// If this is false, there is no need for any networking.
    pub fn networked(&self) -> bool {
        match self {
            &Role::Standalone => false,
            &Role::Server => true,
            &Role::Client => true,
        }
    }
}

/// The game structure, containing globals not specific to frontend.
pub struct Game {
    pub world: World,
    pub dispatcher: Dispatcher<'static, 'static>,
    /// Indicates that the game has been lost, input should no longer be
    /// accepted.
    pub game_over: bool,
}

impl Game {
    fn new_common<'a, 'b>(role: Role) -> (World, DispatcherBuilder<'a, 'b>) {
        let mut world = World::new();
        world.register::<Position>();
        world.register::<Velocity>();
        world.register::<Collision>();
        world.register::<Collided>();
        world.register::<LocalControl>();
        world.register::<Ship>();
        world.register::<Projectile>();
        world.register::<Asteroid>();

        if role.networked() {
            #[cfg(feature = "network")]
            fn add_network_components(world: &mut World, role: Role) {
                world.register::<net::Replicated>();
                world.register::<net::Dirty>();
                world.register::<net::ConnectedClient>();
            }
            #[cfg(not(feature = "network"))]
            fn add_network_components(_world: &mut World, _role: Role) {
                panic!("Role is {:?} but networking is not compiled in");
            }

            add_network_components(&mut world, role);
        }

        world.add_resource(DeltaTime(0.0));
        world.add_resource(Input::new());
        world.add_resource(role);

        let dispatcher = DispatcherBuilder::new()
            .add(SysSimu, "simu", &[])
            .add(SysCollision, "collision", &[])
            .add(SysShip, "ship", &[])
            .add(SysProjectile, "projectile", &[])
            .add(SysAsteroid::new(), "asteroid", &[]);

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
            game_over: false,
        }
    }

    #[cfg(feature = "network")]
    pub fn new_server(port: u16) -> Game {
        let (mut world, mut dispatcher) = Self::new_common(Role::Server);

        dispatcher =
            dispatcher.add(net::SysNetServer::new(port), "netserver", &[]);

        Game {
            world: world,
            dispatcher: dispatcher.build(),
            game_over: false,
        }
    }

    #[cfg(feature = "network")]
    pub fn new_client(address: SocketAddr) -> Game {
        let (mut world, mut dispatcher) = Self::new_common(Role::Client);

        dispatcher =
            dispatcher.add(net::SysNetClient::new(address), "netclient", &[]);

        world.maintain();

        Game {
            world: world,
            dispatcher: dispatcher.build(),
            game_over: false,
        }
    }

    pub fn update(&mut self, dt: f64) {
        {
            let mut r_dt = self.world.write_resource::<DeltaTime>();
            *r_dt = DeltaTime(dt);
        }
        self.dispatcher.dispatch(&mut self.world.res);
        self.world.maintain();

        if !self.game_over
            && self.world.read::<LocalControl>().join().next().is_none()
        {
            warn!("GAME OVER");
            self.game_over = true;
            let mut input = self.world.write_resource::<Input>();
            *input = Input::new();
        }

        let mut input = self.world.write_resource::<Input>();
        if input.fire == Press::PRESSED {
            input.fire = Press::KEPT;
        }
    }
}
