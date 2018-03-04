//! Asteroid objects, floating around for the user to collide with or shoot.

use Role;
use blocks::{Block, BlockInner, Blocky};
#[cfg(feature = "network")]
use net;
use particles::{Effect, EffectInner};
use physics::{delete_entity, Collided, DeltaTime, Position, Velocity};
use rand::{self, Rng};
use ship::Projectile;
use specs::{Component, Entities, Fetch, Join, LazyUpdate, NullStorage,
            ReadStorage, System};
use std::f64::consts::PI;

/// An asteroid
#[derive(Default)]
pub struct Asteroid;

impl Component for Asteroid {
    type Storage = NullStorage<Self>;
}

/// Asteroid spawning and removing.
///
/// Asteroids are spawned after a delay when not enough exist, and removed on
/// collision or when outside the screen.
pub struct SysAsteroid {
    spawn_delay: Option<f64>,
}

impl SysAsteroid {
    pub fn new() -> SysAsteroid {
        SysAsteroid { spawn_delay: None }
    }
}

impl<'a> System<'a> for SysAsteroid {
    type SystemData = (
        Fetch<'a, DeltaTime>,
        Fetch<'a, Role>,
        Fetch<'a, LazyUpdate>,
        Entities<'a>,
        ReadStorage<'a, Collided>,
        ReadStorage<'a, Position>,
        ReadStorage<'a, Asteroid>,
        ReadStorage<'a, Projectile>,
    );

    fn run(
        &mut self,
        (
            dt, role, lazy, entities, collided, pos, asteroid, projectile,
        ): Self::SystemData,
){
        assert!(role.authoritative());

        let dt = dt.0;

        // Remove asteroids gone from the screen or hit
        let mut count = 0;
        for (entity, pos, _) in (&*entities, &pos, &asteroid).join() {
            count += 1;

            let pos = pos.pos;
            if pos[0] < -50.0 || pos[0] > 50.0 || pos[1] < -50.0
                || pos[1] > 50.0
            {
                let new_effect = entities.create();
                lazy.insert(new_effect, Position { pos: pos, rot: 0.0 });
                lazy.insert(
                    new_effect,
                    Effect {
                        effect: EffectInner::Explosion(4.0),
                        lifetime: -1.0,
                    },
                );
                #[cfg(feature = "network")]
                lazy.insert(new_effect, net::Dirty);
                delete_entity(*role, &entities, &lazy, entity);
                continue;
            }

            // Get collision info
            if let Some(col) = collided.get(entity) {
                for hit in &col.hits {
                    if projectile.get(hit.entity).is_some() {
                        // Remove this entity
                        let new_effect = entities.create();
                        lazy.insert(
                            new_effect,
                            Position { pos: pos, rot: 0.0 },
                        );
                        lazy.insert(
                            new_effect,
                            Effect {
                                effect: EffectInner::Explosion(4.0),
                                lifetime: -1.0,
                            },
                        );
                        #[cfg(feature = "network")]
                        lazy.insert(new_effect, net::Dirty);
                        delete_entity(*role, &entities, &lazy, entity);
                        break;
                    }
                }
            }
        }

        self.spawn_delay = if let Some(d) = self.spawn_delay.take() {
            if d <= 0.0 {
                let mut rng = rand::thread_rng();
                let &(xpos, ypos) = rng.choose(&[
                    (-1.0, 0.0), // left
                    (1.0, 0.0),  // right
                    (0.0, -1.0), // bottom
                    (0.0, 1.0),  // top
                ]).unwrap();
                let entity = entities.create();
                lazy.insert(
                    entity,
                    Position {
                        pos: [
                            xpos * 45.0 + ypos * rng.gen_range(-35.0, 35.0),
                            ypos * 45.0 + xpos * rng.gen_range(-35.0, 35.0),
                        ],
                        rot: rng.gen_range(0.0, 2.0 * PI),
                    },
                );
                lazy.insert(
                    entity,
                    Velocity {
                        vel: [
                            rng.gen_range(-4.0, 4.0) - xpos * 10.0,
                            rng.gen_range(-4.0, 4.0) - ypos * 10.0,
                        ],
                        rot: rng.gen_range(-2.0, 2.0),
                    },
                );
                lazy.insert(entity, Asteroid);
                let mut blocks = Vec::new();
                let a = rng.gen_range(3.0, 4.0);
                let ai = a as i32 + 1;
                let b = rng.gen_range(2.0, 3.0);
                let bi = b as i32 + 1;
                for y in -ai..ai {
                    for x in -bi..bi {
                        let x = x as f64;
                        let y = y as f64;
                        if x * x * a * a + y * y * b * b <= a * a * b * b {
                            blocks
                                .push(([x, y], Block::new(BlockInner::Rock)));
                        }
                    }
                }
                lazy.insert(entity, Blocky::new(blocks));
                #[cfg(feature = "network")]
                {
                    lazy.insert(entity, net::Replicated::new());
                    lazy.insert(entity, net::Dirty);
                }
                None
            } else {
                Some(d - dt)
            }
        } else if count < 30 {
            let delay = 2.0 - 0.2 * (20 - count) as f64;
            Some(delay)
        } else {
            None
        };
    }
}
