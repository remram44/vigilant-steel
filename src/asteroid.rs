use std::f64::consts::PI;

use rand::{self, Rng};
use specs::{Component, System,
            Entities, ReadStorage, WriteStorage, Join,
            Fetch, NullStorage};

use physics::{DeltaTime, Position, Velocity, Collision, Collided};

// An asteroid
#[derive(Default)]
pub struct Asteroid;

impl Component for Asteroid {
    type Storage = NullStorage<Self>;
}

// Asteroid spawning and removing
pub struct SysAsteroid {
    spawn_delay: Option<f64>,
}

impl SysAsteroid {
    pub fn new() -> SysAsteroid {
        SysAsteroid { spawn_delay: None }
    }
}

impl<'a> System<'a> for SysAsteroid {
    type SystemData = (Fetch<'a, DeltaTime>,
                       Entities<'a>,
                       WriteStorage<'a, Collision>,
                       ReadStorage<'a, Collided>,
                       WriteStorage<'a, Position>,
                       WriteStorage<'a, Velocity>,
                       WriteStorage<'a, Asteroid>);

    fn run(
        &mut self,
        (dt, entities, mut collision, collided,
         mut pos, mut vel, mut asteroid): Self::SystemData
    ) {
        let dt = dt.0;

        // Remove asteroids gone from the screen or hit
        let mut count = 0;
        for (entity, pos, _) in (&*entities, &pos, &asteroid).join() {
            if let Some(col) = collided.get(entity) {
                if asteroid.get(col.entity).is_none() {
                    info!("Deleting hit asteroid");
                    entities.delete(entity).unwrap();
                    continue;
                }
            }

            let pos = pos.pos;
            if pos[0] < -550.0 || pos[0] > 550.0 ||
                pos[1] < -550.0 || pos[1] > 550.0
            {
                info!("Deleting asteroid");
                entities.delete(entity).unwrap();
            }
            count += 1;
        }

        self.spawn_delay = if let Some(d) = self.spawn_delay.take() {
            if d <= 0.0 {
                info!("Spawning asteroid now");
                let mut rng = rand::thread_rng();
                let &(xpos, ypos) = rng.choose(&[
                    (-1.0,  0.0), // left
                    ( 1.0,  0.0), // right
                    ( 0.0, -1.0), // bottom
                    ( 0.0,  1.0), // top
                ]).unwrap();
                let entity = entities.create();
                pos.insert(
                    entity,
                    Position {
                        pos: [
                            xpos * 500.0 + ypos * rng.gen_range(-500.0, 500.0),
                            ypos * 500.0 + xpos * rng.gen_range(-500.0, 500.0),
                        ],
                        rot: rng.gen_range(0.0, 2.0 * PI),
                    },
                );
                vel.insert(
                    entity,
                    Velocity {
                        vel: [
                            rng.gen_range(-0.3, 0.3) - xpos * 0.4,
                            rng.gen_range(-0.3, 0.3) - ypos * 0.4,
                        ],
                        rot: rng.gen_range(-2.0, 2.0),
                    },
                );
                collision.insert(entity,
                                 Collision { bounding_box: [40.0, 40.0] });
                asteroid.insert(entity, Asteroid);
                None
            } else {
                Some(d - dt)
            }
        } else if count < 10 {
            info!("Currently {} asteroids", count);
            let delay = 3.0 - 0.4 * (10 - count) as f64;
            info!("Spawning asteroid in {} seconds", delay);
            Some(delay)
        } else {
            None
        };
    }
}
