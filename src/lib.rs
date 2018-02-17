extern crate byteorder;
#[macro_use]
extern crate log;
extern crate rand;
extern crate specs;
extern crate vecmath;

pub mod asteroid;
pub mod input;
#[cfg(not(target_os = "emscripten"))]
pub mod net;
pub mod physics;
pub mod ship;
pub mod utils;

use asteroid::{Asteroid, SysAsteroid};
use input::{Input, Press};
use physics::{Collided, Collision, DeltaTime, LocalControl, Position,
              SysCollision, SysSimu, Velocity};
use ship::{Projectile, Ship, SysProjectile, SysShip};
use specs::{Dispatcher, DispatcherBuilder, Join, LazyUpdate, World};

/// The game structure, containing globals not specific to frontend.
pub struct Game {
    pub world: World,
    pub dispatcher: Dispatcher<'static, 'static>,
    /// Indicates that the game has been lost, input should no longer be
    /// accepted.
    pub game_over: bool,
}

impl Game {
    pub fn new() -> Game {
        let mut world = World::new();
        world.register::<Position>();
        world.register::<Velocity>();
        world.register::<Collision>();
        world.register::<Collided>();
        world.register::<LocalControl>();
        world.register::<Ship>();
        world.register::<Projectile>();
        world.register::<Asteroid>();

        let ship = Ship::create(
            &world.entities(),
            &world.read_resource::<LazyUpdate>(),
        );
        world.write::<LocalControl>().insert(ship, LocalControl);

        world.add_resource(DeltaTime(0.0));
        world.add_resource(Input::new());

        let dispatcher = DispatcherBuilder::new()
            .add(SysSimu, "simu", &[])
            .add(SysCollision, "collision", &[])
            .add(SysShip, "ship", &[])
            .add(SysProjectile, "projectile", &[])
            .add(SysAsteroid::new(), "asteroid", &[])
            .build();

        world.maintain();

        Game {
            world: world,
            dispatcher: dispatcher,
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
