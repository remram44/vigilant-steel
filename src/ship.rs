//! Ships and projectiles.

use Role;
use blocks::{Block, BlockInner};
use input::{Input, Press};
#[cfg(feature = "network")]
use net;
use particles::{Effect, EffectInner, Particle, ParticleType};
use physics::{delete_entity, Blocky, Collided, Collision, DeltaTime,
              LocalControl, Position, Velocity};
use rand::{self, Rng};
use specs::{Component, Entities, Entity, Fetch, Join, LazyUpdate,
            NullStorage, ReadStorage, System, VecStorage, WriteStorage};
use vecmath::*;

/// A ship.
///
/// A ship has thrusters allowing it to rotate and move forward, and can fire
/// projectiles.
pub struct Ship {
    pub want_fire: bool,
    pub want_thrust: [f64; 2],
    pub thrust: [f64; 2],
    pub reload: f64,
    pub health: i32,
}

impl Ship {
    pub fn new() -> Ship {
        Ship {
            want_fire: false,
            want_thrust: [0.0, 0.0],
            thrust: [0.0, 0.0],
            reload: 0.0,
            health: 8,
        }
    }

    pub fn create(entities: &Entities, lazy: &Fetch<LazyUpdate>) -> Entity {
        let entity = entities.create();
        lazy.insert(
            entity,
            Position {
                pos: [0.0, 0.0],
                rot: 0.0,
            },
        );
        lazy.insert(
            entity,
            Velocity {
                vel: [0.0, 0.0],
                rot: 0.0,
            },
        );
        lazy.insert(
            entity,
            Collision {
                bounding_box: [1.5, 1.1],
                mass: 1.0,
                inertia: 0.3,
            },
        );
        lazy.insert(entity, Ship::new());
        let blocks = vec![
            ([-1.0, -1.0], Block::new(BlockInner::Thruster(0.0))),
            ([-1.0, 0.0], Block::new(BlockInner::Thruster(0.0))),
            ([-1.0, 1.0], Block::new(BlockInner::Thruster(0.0))),
            ([0.0, -1.0], Block::new(BlockInner::Armor)),
            ([0.0, 1.0], Block::new(BlockInner::Armor)),
            ([0.0, 0.0], Block::new(BlockInner::Cockpit)),
            ([1.0, 0.0], Block::new(BlockInner::Gun(0.0, -1.0))),
        ];
        lazy.insert(entity, Blocky { blocks: blocks });
        #[cfg(feature = "network")]
        {
            lazy.insert(entity, net::Replicated::new());
            lazy.insert(entity, net::Dirty);
        }
        entity
    }
}

impl Component for Ship {
    type Storage = VecStorage<Self>;
}

/// Ship physics and keyboard control.
///
/// This computes the ship's state from the keyboard if `LocalControl`, updates
/// the ship's speed, and fires projectiles.
pub struct SysShip;

impl<'a> System<'a> for SysShip {
    type SystemData = (
        Fetch<'a, DeltaTime>,
        Fetch<'a, Role>,
        Fetch<'a, LazyUpdate>,
        Fetch<'a, Input>,
        Entities<'a>,
        ReadStorage<'a, Position>,
        WriteStorage<'a, Velocity>,
        ReadStorage<'a, Collided>,
        WriteStorage<'a, Ship>,
        ReadStorage<'a, LocalControl>,
    );

    fn run(
        &mut self,
        (
            dt,
            role,
            lazy,
            input,
            entities,
            pos,
            mut vel,
            collided,
            mut ship,
            local,
        ): Self::SystemData,
    ) {
        let dt = dt.0;

        if role.authoritative() {
            // Handle collisions
            for (ent, col, mut ship) in
                (&*entities, &collided, &mut ship).join()
            {
                for hit in &col.hits {
                    if hit.impulse > 2.0 {
                        ship.health -= 1;
                        warn!("Ship collided! Health now {}", ship.health);
                        #[cfg(feature = "network")]
                        lazy.insert(ent, net::Dirty);
                    }
                }
            }

            // Prevent leaving the screen
            for (ent, pos, vel, mut ship) in
                (&*entities, &pos, &mut vel, &mut ship).join()
            {
                if pos.pos[0] < -35.0 || pos.pos[0] > 35.0
                    || pos.pos[1] < -35.0
                    || pos.pos[1] > 35.0
                {
                    ship.health -= 1;
                    vel.vel = vec2_sub([0.0, 0.0], pos.pos);
                    vel.vel =
                        vec2_scale(vel.vel, 60.0 * vec2_inv_len(vel.vel));
                    #[cfg(feature = "network")]
                    lazy.insert(ent, net::Dirty);
                }
            }
        }

        // Control ship thrusters from local input
        for (ent, mut ship, _) in (&*entities, &mut ship, &local).join() {
            ship.want_thrust[0] = -input.movement[0];
            ship.want_thrust[1] = input.movement[1];
            match input.fire {
                Press::UP => ship.want_fire = false,
                Press::PRESSED => ship.want_fire = true,
                _ => {}
            }
            #[cfg(feature = "network")]
            lazy.insert(ent, net::Dirty);
        }

        // Control ships from input
        if role.authoritative() {
            for mut ship in (&mut ship).join() {
                ship.thrust[0] = ship.want_thrust[0].min(1.0).max(-1.0);
                ship.thrust[1] = ship.want_thrust[1].min(1.0).max(0.0)
            }
        }

        for (ent, pos, mut vel, mut ship) in
            (&*entities, &pos, &mut vel, &mut ship).join()
        {
            // Death
            if role.authoritative() && ship.health <= 0 {
                let new_effect = entities.create();
                lazy.insert(
                    new_effect,
                    Position {
                        pos: pos.pos,
                        rot: 0.0,
                    },
                );
                lazy.insert(
                    new_effect,
                    Effect {
                        effect: EffectInner::Explosion(2.0),
                        lifetime: -1.0,
                    },
                );
                #[cfg(feature = "network")]
                lazy.insert(new_effect, net::Dirty);
                delete_entity(*role, &entities, &lazy, ent);
                continue;
            }

            // Apply thrust
            // Update orientation
            vel.rot = ship.thrust[0] * 5.0;
            // Update velocity
            let dir = {
                let (s, c) = pos.rot.sin_cos();
                [c, s]
            };
            vel.vel =
                vec2_add(vel.vel, vec2_scale(dir, ship.thrust[1] * 10.0 * dt));

            // Spawn Exhaust particles
            if role.graphical() {
                if ship.thrust[1] > 0.3 {
                    let mut rng = rand::thread_rng();
                    let thrust_pos = vec2_add(pos.pos, vec2_scale(dir, -1.0));
                    let thrust_vel = vec2_scale(dir, -10.0);
                    let p = entities.create();
                    lazy.insert(
                        p,
                        Position {
                            pos: thrust_pos,
                            rot: 0.0,
                        },
                    );
                    lazy.insert(
                        p,
                        Velocity {
                            vel: [
                                thrust_vel[0] + rng.gen_range(-6.0, 6.0),
                                thrust_vel[1] + rng.gen_range(-6.0, 6.0),
                            ],
                            rot: rng.gen_range(-5.0, 5.0),
                        },
                    );
                    lazy.insert(
                        p,
                        Particle {
                            lifetime: 0.5,
                            which: ParticleType::Exhaust,
                        },
                    );
                }
            }

            // Apply friction
            vel.vel = vec2_add(
                vel.vel,
                vec2_scale(vel.vel, -0.04 * dt * vec2_len(vel.vel)),
            );

            // Fire
            if role.authoritative() {
                if ship.want_fire && ship.reload <= 0.0 {
                    ship.reload = 1.5;

                    Projectile::create(
                        &entities,
                        &lazy,
                        vec2_add(pos.pos, vec2_scale(dir, 2.2)),
                        pos.rot,
                    );
                    // Recoil
                    vel.vel = vec2_add(vel.vel, vec2_scale(dir, -6.0));
                    #[cfg(feature = "network")]
                    lazy.insert(ent, net::Dirty);
                } else if ship.reload > 0.0 {
                    ship.reload -= dt;
                }
            }
        }
    }
}

/// A projectile.
///
/// This is a simple segment that goes in a straight line, and gets removed
/// when it hits something or exits the screen.
#[derive(Default)]
pub struct Projectile;

impl Projectile {
    pub fn create(
        entities: &Entities,
        lazy: &Fetch<LazyUpdate>,
        pos: [f64; 2],
        rot: f64,
    ) -> Entity {
        let entity = entities.create();
        let (s, c) = rot.sin_cos();
        lazy.insert(entity, Position { pos: pos, rot: rot });
        lazy.insert(
            entity,
            Velocity {
                vel: [60.0 * c, 60.0 * s],
                rot: 0.0,
            },
        );
        lazy.insert(
            entity,
            Collision {
                bounding_box: [0.8, 0.1],
                mass: 1000.0,
                inertia: 1.0,
            },
        );
        lazy.insert(entity, Projectile);
        #[cfg(feature = "network")]
        {
            lazy.insert(entity, net::Replicated::new());
            lazy.insert(entity, net::Dirty);
        }
        entity
    }
}

impl Component for Projectile {
    type Storage = NullStorage<Self>;
}

/// Deletes projectiles when they fall off.
pub struct SysProjectile;

impl<'a> System<'a> for SysProjectile {
    type SystemData = (
        Fetch<'a, Role>,
        Fetch<'a, LazyUpdate>,
        Entities<'a>,
        ReadStorage<'a, Collided>,
        ReadStorage<'a, Position>,
        ReadStorage<'a, Projectile>,
    );

    fn run(
        &mut self,
        (role, lazy, entities, collided, pos, projectile): Self::SystemData,
    ) {
        assert!(role.authoritative());

        // Remove projectiles gone from the screen or hit
        for (entity, pos, _) in (&*entities, &pos, &projectile).join() {
            if collided.get(entity).is_some() {
                delete_entity(*role, &entities, &lazy, entity);
                continue;
            }

            let pos = pos.pos;
            if pos[0] < -50.0 || pos[0] > 50.0 || pos[1] < -50.0
                || pos[1] > 50.0
            {
                delete_entity(*role, &entities, &lazy, entity);
            }
        }
    }
}
