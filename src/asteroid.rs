use std::f64::consts::PI;

use rand::{self, Rng};
use specs::{Component, System,
            Entities, ReadStorage, Join,
            Fetch, NullStorage, LazyUpdate};

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
                       Fetch<'a, LazyUpdate>,
                       Entities<'a>,
                       ReadStorage<'a, Collided>,
                       ReadStorage<'a, Position>,
                       ReadStorage<'a, Asteroid>);

    fn run(
        &mut self,
        (dt, lazy, entities, collided,
         pos, asteroid): Self::SystemData
    ) {
        let dt = dt.0;

        // Remove asteroids gone from the screen or hit
        let mut count = 0;
        for (entity, pos, _) in (&*entities, &pos, &asteroid).join() {
            count += 1;

            let pos = pos.pos;
            if pos[0] < -500.0 || pos[0] > 500.0 ||
                pos[1] < -500.0 || pos[1] > 500.0
            {
                info!("Deleting asteroid");
                entities.delete(entity).unwrap();
                continue;
            }

            // Get collision info
            if let Some(col) = collided.get(entity) {
                for ent in col.entities.iter() {
                    // If collision is not with an asteroid
                    if asteroid.get(*ent).is_none() {
                        // Remove this entity
                        info!("Deleting hit asteroid");
                        entities.delete(entity).unwrap();
                        break;
                    }
                }
            }
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
                lazy.insert(
                    entity,
                    Position {
                        pos: [
                            xpos * 450.0 + ypos * rng.gen_range(-350.0, 350.0),
                            ypos * 450.0 + xpos * rng.gen_range(-350.0, 350.0),
                        ],
                        rot: rng.gen_range(0.0, 2.0 * PI),
                    },
                );
                lazy.insert(
                    entity,
                    Velocity {
                        vel: [
                            rng.gen_range(-0.2, 0.2) - xpos * 0.5,
                            rng.gen_range(-0.2, 0.2) - ypos * 0.5,
                        ],
                        rot: rng.gen_range(-2.0, 2.0),
                    },
                );
                lazy.insert(entity,
                            Collision { bounding_box: [40.0, 40.0] });
                lazy.insert(entity, Asteroid);
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
