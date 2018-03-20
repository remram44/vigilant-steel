//! Ships and projectiles.
//!
//! A ship is just a `Blocky` object that has a cockpit. A `Ship` component
//! gets tacked on to store control and thruster state.
// TODO: Take some behavior out of SysShip and into blocks.rs

use {Clock, Role};
use asteroid::Asteroid;
use blocks::{Block, BlockInner, Blocky};
use input::{Input, Press};
#[cfg(feature = "network")]
use net;
use particles::{Effect, EffectInner, Particle, ParticleType};
use physics::{affect_area, delete_entity, find_collision_tree_ray, AABox,
              DeltaTime, DetectCollision, HitEffect, Hits, LocalControl,
              Position, Velocity};
use rand::{self, Rng};
use specs::{Component, Entities, Entity, Fetch, Join, LazyUpdate,
            ReadStorage, System, VecStorage, WriteStorage};
use std::f64::consts::PI;
use utils::angle_wrap;
use vecmath::*;

/// A ship.
///
/// A ship has thrusters allowing it to rotate and move forward, and can fire
/// projectiles.
pub struct Ship {
    pub want_fire: bool,
    pub want_thrust: [f64; 2],
    pub want_thrust_rot: f64,
    pub want_target: [f64; 2],
    pub thrust: [f64; 2],
    pub thrust_rot: f64,
}

impl Ship {
    pub fn new() -> Ship {
        Ship {
            want_fire: false,
            want_thrust: [0.0, 0.0],
            want_thrust_rot: 0.0,
            want_target: [0.0, 0.0],
            thrust: [0.0, 0.0],
            thrust_rot: 0.0,
        }
    }

    pub fn create(entities: &Entities, lazy: &Fetch<LazyUpdate>) -> Entity {
        use self::BlockInner::*;
        let blocks = &[
            ([0, 0], Cockpit),
            ([-3, -2], Armor),
            ([-3, -1], Thruster { angle: 0.0 }),
            ([-3, 0], Thruster { angle: 0.0 }),
            ([-3, 1], Thruster { angle: 0.0 }),
            ([-3, 2], Armor),
            ([-2, -2], Thruster { angle: 0.5 * PI }),
            ([-2, -1], Armor),
            ([-2, 0], Armor),
            ([-2, 1], Armor),
            ([-2, 2], Thruster { angle: -0.5 * PI }),
            ([-1, -2], Thruster { angle: PI }),
            ([-1, -1], Armor),
            ([-1, 0], Armor),
            ([-1, 1], Armor),
            ([-1, 2], Thruster { angle: PI }),
            ([-0, -1], Armor),
            ([-0, 1], Armor),
            ([1, -1], Armor),
            ([1, 0], Armor),
            ([1, 1], Armor),
            ([2, -1], Thruster { angle: 0.5 * PI }),
            ([2, 0], Armor),
            ([2, 1], Thruster { angle: -0.5 * PI }),
            (
                [3, -1],
                PlasmaGun {
                    angle: 0.0,
                    cooldown: -1.0,
                },
            ),
            (
                [3, 0],
                RailGun {
                    angle: 0.0,
                    cooldown: -1.0,
                },
            ),
            (
                [3, 1],
                PlasmaGun {
                    angle: 0.0,
                    cooldown: -1.0,
                },
            ),
        ];
        let blocks = blocks
            .iter()
            .map(|&(ref p, ref b)| {
                ([p[0] as f64, p[1] as f64], Block::new(b.clone()))
            })
            .collect();
        let (blocky, center) = Blocky::new(blocks);
        let entity = entities.create();
        let angle: f64 = 0.0;
        let (s, c) = angle.sin_cos();
        let center =
            [center[0] * c - center[1] * s, center[0] * s + center[1] * s];
        lazy.insert(
            entity,
            Position {
                pos: center,
                rot: angle,
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
        lazy.insert(entity, blocky);
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
        WriteStorage<'a, Position>,
        WriteStorage<'a, Velocity>,
        ReadStorage<'a, Hits>,
        WriteStorage<'a, Ship>,
        WriteStorage<'a, Blocky>,
        ReadStorage<'a, Asteroid>,
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
            mut pos,
            mut vel,
            hits,
            mut ship,
            mut blocky,
            asteroid,
            local,
        ): Self::SystemData,
    ) {
        let dt = dt.0;
        let mut rng = rand::thread_rng();

        if role.authoritative() {
            // Handle collisions
            for (ent, mut pos, mut blk, hits) in
                (&*entities, &mut pos, &mut blocky, &hits).join()
            {
                let (s, c) = pos.rot.sin_cos();
                let mut deleted = false;
                for hit in &**hits {
                    match hit.effect {
                        HitEffect::Collision(_, _) => {}
                        HitEffect::Explosion(size) => {
                            let mut impulse = [0.0, 0.0];
                            let mut rot = 0.0;

                            // Hurt some blocks
                            for &mut (loc, ref mut block) in &mut blk.blocks {
                                let diff = vec2_sub(hit.rel_location, loc);
                                let sq_dist = vec2_square_len(diff);
                                if sq_dist <= size {
                                    block.health -=
                                        1.0 - sq_dist / (size * size);
                                    if block.health < 0.0 {
                                        deleted = true;
                                    }
                                    let impulse_blk =
                                        vec2_scale(diff, -10.0 / sq_dist);
                                    impulse = vec2_add(impulse, impulse_blk);
                                    rot += loc[0] * impulse_blk[1]
                                        - loc[1] * impulse_blk[0];
                                }
                            }

                            // Push object back
                            impulse = [
                                impulse[0] * c - impulse[1] * s,
                                impulse[1] * s + impulse[1] * c,
                            ];
                            let vel = vel.get_mut(ent).unwrap();
                            vel.vel = vec2_add(
                                vel.vel,
                                vec2_scale(impulse, 1.0 / blk.mass),
                            );
                            vel.rot += rot / blk.inertia;
                        }
                    }
                }

                if deleted {
                    let (dead_blocks, center, pieces) = blk.maintain();

                    for (loc, blk) in dead_blocks {
                        // Spawn particle effects for dead blocks
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

                        // If a cockpit died then this is no longer a ship
                        if let BlockInner::Cockpit = blk.inner {
                            lazy.remove::<Ship>(ent);
                        }
                    }

                    // If there is no block remaining, delete the entity
                    if blk.blocks.is_empty() {
                        entities.delete(ent).unwrap();
                        continue;
                    }

                    // Create entities from pieces that broke off
                    let vel = vel.get(ent).unwrap();
                    let is_asteroid = asteroid.get(ent).is_some();
                    for (blocky, center) in pieces {
                        let center = [
                            center[0] * c - center[1] * s,
                            center[0] * s + center[1] * c,
                        ];
                        let newent = entities.create();
                        lazy.insert(
                            newent,
                            Position {
                                pos: vec2_add(pos.pos, center),
                                rot: pos.rot,
                            },
                        );
                        lazy.insert(
                            newent,
                            Velocity {
                                vel: vel.vel,
                                rot: vel.rot,
                            },
                        );
                        lazy.insert(newent, blocky);
                        // Asteroids stay asteroids
                        if is_asteroid {
                            lazy.insert(newent, Asteroid);
                        }
                        #[cfg(feature = "network")]
                        {
                            lazy.insert(newent, net::Replicated::new());
                            lazy.insert(newent, net::Dirty);
                        }
                    }

                    // Update position for new center of mass
                    let center = [
                        center[0] * c - center[1] * s,
                        center[0] * s + center[1] * c,
                    ];
                    pos.pos = vec2_add(pos.pos, center);
                }

                #[cfg(feature = "network")]
                lazy.insert(ent, net::Dirty);
            }

            // Prevent leaving the screen
            for (ent, pos, vel, _) in
                (&*entities, &pos, &mut vel, &ship).join()
            {
                if pos.pos[0] < -100.0 || pos.pos[0] > 100.0
                    || pos.pos[1] < -100.0
                    || pos.pos[1] > 100.0
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
            ship.want_thrust = input.movement;
            ship.want_thrust_rot = input.rotation;
            ship.want_target = input.mouse;
            match input.fire {
                Press::UP => ship.want_fire = false,
                Press::PRESSED => ship.want_fire = true,
                _ => {}
            }
            #[cfg(feature = "network")]
            lazy.insert(ent, net::Dirty);
        }

        for (ent, pos, mut vel, mut ship, mut blocky) in
            (&*entities, &pos, &mut vel, &mut ship, &mut blocky).join()
        {
            let (s, c) = pos.rot.sin_cos();

            // Action thrusters from controls
            if role.authoritative() {
                let (thrust, rot) = compute_thrust(
                    blocky.blocks.iter().enumerate(),
                    |_, _| {},
                    ship.want_thrust,
                    ship.want_thrust_rot,
                );
                ship.thrust = thrust;
                ship.thrust_rot = rot;
            }

            // Update blocks
            let target_rel = [
                ship.want_target[0] * c + ship.want_target[1] * s,
                -ship.want_target[0] * s + ship.want_target[1] * c,
            ];
            for &mut (rel, ref mut block) in &mut blocky.blocks {
                match &mut block.inner {
                    &mut BlockInner::PlasmaGun { ref mut angle, .. } => {
                        let target_rel = vec2_sub(target_rel, rel);
                        let bearing = target_rel[1].atan2(target_rel[0]);
                        let chg = angle_wrap(bearing - *angle);
                        *angle += angle_wrap(chg.min(3.0 * dt).max(-3.0 * dt));
                    }
                    _ => {}
                }
            }

            // Apply thrust
            // Update orientation
            vel.rot += ship.thrust_rot * dt / blocky.inertia;
            // Update velocity
            vel.vel = vec2_add(
                vel.vel,
                vec2_scale(
                    [
                        c * ship.thrust[0] - s * ship.thrust[1],
                        s * ship.thrust[0] + c * ship.thrust[1],
                    ],
                    dt / blocky.mass,
                ),
            );

            // Spawn Exhaust particles
            if role.graphical() {
                let mut spawn_thrust_exhaust = |idx, thrust| {
                    let &(rel, ref block): &(
                        [f64; 2],
                        Block,
                    ) = &blocky.blocks[idx];
                    let angle = match block.inner {
                        BlockInner::Thruster { angle } => angle,
                        _ => return,
                    };
                    let rate = 1.0 / (thrust * 40.0);
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
                };
                compute_thrust(
                    blocky.blocks.iter().enumerate(),
                    spawn_thrust_exhaust,
                    ship.want_thrust,
                    ship.want_thrust_rot,
                );
            }

            // Apply friction
            vel.vel = vec2_add(
                vel.vel,
                vec2_scale(vel.vel, -0.04 * dt * vec2_len(vel.vel)),
            );
            vel.rot -= vel.rot * vel.rot.abs() * 2.0 * dt;

            // Fire
            if role.authoritative() {
                let mut fired = false;
                let mass = blocky.mass;
                for &mut (rel, ref mut block) in &mut blocky.blocks {
                    let (angle, cooldown) = match block.inner {
                        BlockInner::PlasmaGun {
                            angle,
                            ref mut cooldown,
                        } => (angle, cooldown),
                        BlockInner::RailGun {
                            angle,
                            ref mut cooldown,
                        } => (angle, cooldown),
                        _ => continue,
                    };
                    if ship.want_fire && *cooldown <= 0.0 {
                        let fire_dir = {
                            let (fs, fc) = (pos.rot + angle).sin_cos();
                            [fc, fs]
                        };
                        let fire_pos = vec2_add(
                            pos.pos,
                            [rel[0] * c - rel[1] * s, rel[0] * s + rel[1] * c],
                        );
                        match block.inner {
                            BlockInner::PlasmaGun { .. } => {
                                let fire_dir_loc = {
                                    let (ps, pc) = angle.sin_cos();
                                    [pc, ps]
                                };
                                let proj_loc = vec2_add(
                                    rel,
                                    vec2_scale(fire_dir_loc, 1.6),
                                );
                                if find_collision_tree_ray(
                                    proj_loc,
                                    fire_dir_loc,
                                    &blocky.tree,
                                ).is_some()
                                {
                                    continue;
                                }
                                Projectile::create(
                                    &entities,
                                    &lazy,
                                    vec2_add(
                                        fire_pos,
                                        vec2_scale(fire_dir, 1.6),
                                    ),
                                    pos.rot + angle,
                                    ProjectileType::Plasma,
                                    ent,
                                );
                                *cooldown = rng.gen_range(0.3, 0.4);
                            }
                            BlockInner::RailGun { .. } => {
                                Projectile::create(
                                    &entities,
                                    &lazy,
                                    vec2_add(
                                        fire_pos,
                                        vec2_scale(fire_dir, 1.6),
                                    ),
                                    pos.rot + angle,
                                    ProjectileType::Rail,
                                    ent,
                                );
                                *cooldown = rng.gen_range(1.4, 1.6);
                            }
                            _ => {}
                        }
                        // Recoil
                        vel.vel = vec2_add(
                            vel.vel,
                            vec2_scale(fire_dir, -10.0 / mass),
                        );
                        fired = true;
                    } else if *cooldown > 0.0 {
                        *cooldown -= dt;
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

pub enum ProjectileType {
    Plasma,
    Rail,
}

impl ProjectileType {
    pub fn speed(&self) -> f64 {
        match *self {
            ProjectileType::Plasma => 60.0,
            ProjectileType::Rail => 35.0,
        }
    }

    pub fn mass(&self) -> Option<f64> {
        match *self {
            ProjectileType::Plasma => None,
            ProjectileType::Rail => Some(5.0),
        }
    }

    pub fn bounds(&self) -> AABox {
        match *self {
            ProjectileType::Plasma => AABox {
                xmin: -0.8,
                xmax: 0.8,
                ymin: -0.1,
                ymax: 0.1,
            },
            ProjectileType::Rail => AABox {
                xmin: -0.8,
                xmax: 0.8,
                ymin: -0.6,
                ymax: 0.6,
            },
        }
    }
}

/// A projectile.
///
/// This is a simple segment that goes in a straight line, and gets removed
/// when it hits something or exits the screen.
pub struct Projectile {
    pub kind: ProjectileType,
    pub shooter: Entity,
}

impl Projectile {
    pub fn create(
        entities: &Entities,
        lazy: &Fetch<LazyUpdate>,
        pos: [f64; 2],
        rot: f64,
        kind: ProjectileType,
        shooter: Entity,
    ) -> Entity {
        let entity = entities.create();
        let (s, c) = rot.sin_cos();
        lazy.insert(entity, Position { pos: pos, rot: rot });
        lazy.insert(
            entity,
            Velocity {
                vel: [kind.speed() * c, kind.speed() * s],
                rot: 0.0,
            },
        );
        let bounding_box = kind.bounds();
        let radius = bounding_box.compute_sq_radius().sqrt();
        lazy.insert(
            entity,
            DetectCollision {
                bounding_box,
                radius,
                mass: kind.mass(),
                ignore: None,
            },
        );
        lazy.insert(entity, Projectile { kind, shooter });
        #[cfg(feature = "network")]
        {
            lazy.insert(entity, net::Replicated::new());
            lazy.insert(entity, net::Dirty);
        }
        entity
    }
}

impl Component for Projectile {
    type Storage = VecStorage<Self>;
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

        for (entity, pos, proj) in (&*entities, &position, &projectile).join()
        {
            // Remove projectiles gone from the screen
            if pos.pos[0] < -150.0 || pos.pos[0] > 150.0 || pos.pos[1] < -150.0
                || pos.pos[1] > 150.0
            {
                delete_entity(*role, &entities, &lazy, entity);
            }

            // Hit projectiles go off and affect an area
            let (mut delete, mut hit_loc) = (false, None);
            match hits.get(entity) {
                Some(v) => for h in &**v {
                    match h.effect {
                        HitEffect::Collision(_, e) => {
                            delete = true;
                            if e != proj.shooter {
                                let (s, c) = pos.rot.sin_cos();
                                hit_loc = Some(vec2_add(
                                    pos.pos,
                                    [
                                        c * h.rel_location[0]
                                            - s * h.rel_location[1],
                                        s * h.rel_location[0]
                                            + s * h.rel_location[1],
                                    ],
                                ));
                                break;
                            }
                        }
                        _ => {}
                    }
                },
                None => {}
            };
            if delete {
                delete_entity(*role, &entities, &lazy, entity);
            }
            let hit_loc = match hit_loc {
                None => continue,
                Some(l) => l,
            };

            match proj.kind {
                ProjectileType::Plasma => {
                    // Affect entities in range with an Explosion
                    affect_area(
                        &entities,
                        &position,
                        &blocky,
                        &mut hits,
                        hit_loc,
                        3.0,
                        HitEffect::Explosion(3.0),
                    );

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
                            effect: EffectInner::LaserHit,
                            lifetime: -1.0,
                        },
                    );
                    #[cfg(feature = "network")]
                    lazy.insert(new_effect, net::Dirty);
                }
                ProjectileType::Rail => {
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
                            effect: EffectInner::MetalHit,
                            lifetime: -1.0,
                        },
                    );
                    #[cfg(feature = "network")]
                    lazy.insert(new_effect, net::Dirty);
                }
            }
            continue;
        }
    }
}

/// Computes the thrust generated by thrusters.
///
/// Goes over the iterator of blocks, computing the maximu thrust that can be
/// generated in a specific direction. The callback function gets called with
/// thrust generated by each individual thruster.
fn compute_thrust<'a, T, B, F>(
    blocks: B,
    mut cb: F,
    dir: [f64; 2],
    rot: f64,
) -> ([f64; 2], f64)
where
    T: Clone,
    B: Iterator<Item = (T, &'a ([f64; 2], Block))>,
    F: FnMut(T, f64),
{
    let dir = {
        let len = vec2_len(dir);
        if len > 0.1 {
            vec2_scale(dir, 1.0 / len)
        } else {
            [0.0, 0.0]
        }
    };

    let mut thrust = [0.0, 0.0];
    let mut thrust_rot = 0.0;

    for (ref udata, &(loc, ref block)) in blocks {
        match block.inner {
            BlockInner::Thruster { angle } => {
                let (s, c) = angle.sin_cos();
                let torque = loc[0] * s - loc[1] * c;
                // If this takes us forward, or rotating the right way
                if vec2_dot([c, s], dir) >= 0.5 || (torque > 1.0 && rot > 0.1)
                    || (torque < -1.0 && rot < -0.1)
                {
                    // Fire thruster
                    thrust = vec2_add(thrust, vec2_scale([c, s], 60.0));
                    thrust_rot += torque * 60.0;
                    cb(udata.clone(), 1.0);
                }
            }
            _ => {}
        }
    }
    (thrust, thrust_rot)
}
