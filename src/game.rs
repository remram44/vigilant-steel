//! Global game code, not specific to a renderer or window system.

use specs::{Dispatcher, DispatcherBuilder, World, LazyUpdate};

use asteroid::{Asteroid, SysAsteroid};
use input::{Input, Press};
use physics::{DeltaTime, Position, Velocity, Collision, Collided,
              LocalControl, SysCollision, SysSimu};
use ship::{Ship, SysShip, Projectile, SysProjectile};

/// Global resource storing the player's health points.
pub struct Health(pub i32);

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

        let ship = Ship::create(&world.entities(),
                                &world.read_resource::<LazyUpdate>());
        world.write::<LocalControl>().insert(ship, LocalControl);

        world.add_resource(DeltaTime(0.0));
        world.add_resource(Input::new());
        world.add_resource(Health(8));

        let dispatcher = DispatcherBuilder::new()
            .add(SysSimu, "simu", &[])
            .add(SysCollision, "collision", &[])
            .add(SysShip, "ship", &[])
            .add(SysProjectile, "projectile", &[])
            .add(SysAsteroid::new(), "asteroid", &[])
            .build();

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

        if self.world.read_resource::<Health>().0 <= 0 {
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
