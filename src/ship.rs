//! Ships and projectiles.

use {Clock, Role};
use blocks::{Block, BlockInner, Blocky};
use input::{Input, Press};
#[cfg(feature = "network")]
use net;
use particles::{Effect, EffectInner, Particle, ParticleType};
use physics::{affect_area, delete_entity, AABox, DeltaTime, DetectCollision,
              HitEffect, Hits, LocalControl, Position, Velocity};
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
}

impl Ship {
    pub fn new() -> Ship {
        Ship {
            want_fire: false,
            want_thrust: [0.0, 0.0],
            thrust: [0.0, 0.0],
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
        lazy.insert(entity, Ship::new());
        let blocks = vec![
            (
                [-1.0, -1.0],
                Block::new(BlockInner::Thruster { angle: 0.7 }),
            ),
            ([-1.0, 0.0], Block::new(BlockInner::Thruster { angle: 0.0 })),
            (
                [-1.0, 1.0],
                Block::new(BlockInner::Thruster { angle: -0.7 }),
            ),
            ([0.0, -1.0], Block::new(BlockInner::Armor)),
            ([0.0, 1.0], Block::new(BlockInner::Armor)),
            ([0.0, 0.0], Block::new(BlockInner::Cockpit)),
            (
                [1.0, 0.0],
                Block::new(BlockInner::Gun {
                    angle: 0.0,
                    cooldown: -1.0,
                }),
            ),
        ];
        lazy.insert(entity, Blocky::new(blocks));
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
        Fetch<'a, Clock>,
        Entities<'a>,
        ReadStorage<'a, Position>,
        WriteStorage<'a, Velocity>,
        ReadStorage<'a, Hits>,
        WriteStorage<'a, Ship>,
        WriteStorage<'a, Blocky>,
        ReadStorage<'a, LocalControl>,
    );

    fn run(
        &mut self,
        (
            dt,
            role,
            lazy,
            input,
            clock,
            entities,
            pos,
            mut vel,
            hits,
            mut ship,
            mut blocky,
            local,
        ): Self::SystemData,
    ) {
        let dt = dt.0;
        let mut rng = rand::thread_rng();

        if role.authoritative() {
            // Handle collisions
            for (ent, pos, mut blk, hits) in
                (&*entities, &pos, &mut blocky, &hits).join()
            {
                let mut deleted = false;
                for hit in &**hits {
                    match hit.effect {
                        HitEffect::Collision(_) => {}
                        HitEffect::Explosion(size) => {
                            // Hurt some blocks
                            for &mut (loc, ref mut block) in &mut blk.blocks {
                                let sq_dist = vec2_square_len(vec2_sub(
                                    hit.rel_location,
                                    loc,
                                ));
                                if sq_dist <= size {
                                    block.health -=
                                        1.0 - sq_dist / (size * size);
                                    if block.health < 0.0 {
                                        deleted = true;
                                    }
                                }
                            }
                        }
                    }
                }

                if deleted {
                    let (s, c) = pos.rot.sin_cos();
                    for (loc, _) in blk.pop_dead_blocks() {
                        // Add particle effect
                        let new_effect = entities.create();
                        lazy.insert(
                            new_effect,
                            Position {
                                pos: vec2_add(
                                    pos.pos,
                                    [
                                        c * loc[0] - s * loc[1],
                                        s * loc[0] + c * loc[1],
                                    ],
                                ),
                                rot: 0.0,
                            },
                        );
                        lazy.insert(
                            new_effect,
                            Effect {
                                effect: EffectInner::Explosion(0.4),
                                lifetime: -1.0,
                            },
                        );
                    }
                }

                #[cfg(feature = "network")]
                lazy.insert(ent, net::Dirty);
            }

            // Prevent leaving the screen
            for (ent, pos, vel, _) in
                (&*entities, &pos, &mut vel, &ship).join()
            {
                if pos.pos[0] < -35.0 || pos.pos[0] > 35.0
                    || pos.pos[1] < -35.0
                    || pos.pos[1] > 35.0
                {
                    vel.vel = vec2_sub([0.0, 0.0], pos.pos);
                    vel.vel =
                        vec2_scale(vel.vel, 60.0 * vec2_inv_len(vel.vel));
                    #[cfg(feature = "network")]
                    lazy.insert(ent, net::Dirty);
                }
            }
        }

        // Set ship controls from local input
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

        // Action thrusters from controls
        if role.authoritative() {
            for mut ship in (&mut ship).join() {
                ship.thrust[0] = ship.want_thrust[0].min(1.0).max(-1.0);
                ship.thrust[1] = ship.want_thrust[1].min(1.0).max(0.0)
            }
        }

        for (ent, pos, mut vel, mut ship, mut blocky) in
            (&*entities, &pos, &mut vel, &mut ship, &mut blocky).join()
        {
            // TODO: Death if no Cockpit
            if role.authoritative() && false {
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
            let (s, c) = pos.rot.sin_cos();
            let dir = [c, s];
            vel.vel =
                vec2_add(vel.vel, vec2_scale(dir, ship.thrust[1] * 10.0 * dt));

            // Spawn Exhaust particles
            if role.graphical() && ship.thrust[1] > 0.3 {
                for &(ref rel, ref block) in &blocky.blocks {
                    let angle = match block.inner {
                        BlockInner::Thruster { angle } => angle,
                        _ => continue,
                    };
                    let rate = 1.0 / (angle.cos() * ship.thrust[1] * 40.0);
                    let num = (**clock / rate) as i32
                        - ((**clock - dt) / rate) as i32;
                    for _ in 0..num {
                        let thrust_dir = {
                            let (ts, tc) = (pos.rot + angle).sin_cos();
                            [tc, ts]
                        };
                        let thrust_pos = [
                            pos.pos[0] + rel[0] * c - rel[1] * s,
                            pos.pos[1] + rel[0] * s + rel[1] * c,
                        ];
                        let thrust_pos =
                            vec2_sub(thrust_pos, vec2_scale(thrust_dir, 0.6));
                        let thrust_vel = vec2_scale(thrust_dir, -10.0);
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
            }

            // Apply friction
            vel.vel = vec2_add(
                vel.vel,
                vec2_scale(vel.vel, -0.04 * dt * vec2_len(vel.vel)),
            );

            // Fire
            if role.authoritative() {
                let mut fired = false;
                for &mut (rel, ref mut block) in &mut blocky.blocks {
                    let (angle, cooldown) = match block.inner {
                        BlockInner::Gun { angle, cooldown } => {
                            (angle, cooldown)
                        }
                        _ => continue,
                    };
                    if ship.want_fire && cooldown <= 0.0 {
                        let fire_dir = {
                            let (fs, fc) = (pos.rot + angle).sin_cos();
                            [fc, fs]
                        };
                        let fire_pos = [
                            pos.pos[0] + rel[0] * c - rel[1] * s,
                            pos.pos[1] + rel[0] * s + rel[1] * c,
                        ];
                        Projectile::create(
                            &entities,
                            &lazy,
                            vec2_add(fire_pos, vec2_scale(fire_dir, 1.6)),
                            pos.rot + angle,
                        );
                        // Recoil
                        vel.vel =
                            vec2_add(vel.vel, vec2_scale(fire_dir, -6.0));
                        block.inner = BlockInner::Gun {
                            angle: angle,
                            cooldown: rng.gen_range(1.4, 1.6),
                        };
                        fired = true;
                    } else if cooldown > 0.0 {
                        block.inner = BlockInner::Gun {
                            angle: angle,
                            cooldown: cooldown - dt,
                        };
                    }
                }
                #[cfg(feature = "network")]
                {
                    if fired {
                        lazy.insert(ent, net::Dirty);
                    }
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
            DetectCollision {
                bounding_box: AABox {
                    xmin: -0.8,
                    xmax: 0.8,
                    ymin: 0.1,
                    ymax: 0.1,
                },
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
        WriteStorage<'a, Hits>,
        ReadStorage<'a, Position>,
        ReadStorage<'a, Blocky>,
        ReadStorage<'a, Projectile>,
    );

    fn run(
        &mut self,
            (
                role,
                lazy,
                entities,
                mut
                hits,
                position,
                blocky,
                projectile,
            ): Self::SystemData,
){
        assert!(role.authoritative());

        for (entity, pos, _) in (&*entities, &position, &projectile).join() {
            // Hit projectiles go off and affect an area
            if hits.get(entity).is_some() {
                // Affect entities in range with an Explosion
                affect_area(
                    &entities,
                    &position,
                    &blocky,
                    &mut hits,
                    pos.pos,
                    3.0,
                    HitEffect::Explosion(3.0),
                );

                delete_entity(*role, &entities, &lazy, entity);
                continue;
            }

            // Remove projectiles gone from the screen
            let pos = pos.pos;
            if pos[0] < -50.0 || pos[0] > 50.0 || pos[1] < -50.0
                || pos[1] > 50.0
            {
                delete_entity(*role, &entities, &lazy, entity);
            }
        }
    }
}
