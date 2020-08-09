//! Particles and particle effects.
//!
//! Particles can be created directly, from client code; for example, the
//! ship's exhaust spawns particles.
//! Particle effects can also be spawned, from server code: it will be
//! replicated so that the effect happens on clients as well. For example,
//! explosions are an `Effect`, that is turned into particles by `SysParticles`
//! once we got to replicate it to the clients.

use rand::{self, Rng};
use specs::{Component, Entities, Read, ReadExpect, Join, LazyUpdate,
            ReadStorage, System, VecStorage, WriteStorage};
use std::f32::consts::PI;

use crate::Role;
use crate::physics::{DeltaTime, Position, Velocity};

/// Types of particles, that determine lifetime and render model.
#[derive(Clone, Copy, Debug)]
pub enum ParticleType {
    /// Metallic sparks, from things crashing into each other.
    Spark,
    /// Smoke out of a thruster.
    Exhaust,
    /// Destroyed parts blow up.
    Explosion,
    /// Laser hits flash.
    LaserHit,
}

/// This entity is a particle.
///
/// Those are only created on graphical clients, don't get replicated, and
/// disappear after a moment.
pub struct Particle {
    pub lifetime: f32,
    pub which: ParticleType,
}

impl Component for Particle {
    type Storage = VecStorage<Self>;
}

/// Particle effect.
///
/// A particle effect emit particles, possibly over time. If the entity is also
/// tagged with `net::Dirty`, it will be replicated to clients.
/// Some systems spawn particles directly, such as thrusters, and no
/// replication of the effect is needed (the ship is replicated).
#[derive(Debug, Clone)]
pub enum EffectInner {
    Explosion(f32),
    MetalHit,
    LaserHit,
}

pub struct Effect {
    pub effect: EffectInner,
    pub lifetime: f32,
}

impl Component for Effect {
    type Storage = VecStorage<Self>;
}

/// System that spawns particles (from effects) and deletes old particles.
pub struct SysParticles;

impl<'a> System<'a> for SysParticles {
    type SystemData = (
        Read<'a, DeltaTime>,
        ReadExpect<'a, Role>,
        Read<'a, LazyUpdate>,
        Entities<'a>,
        ReadStorage<'a, Position>,
        WriteStorage<'a, Effect>,
        WriteStorage<'a, Particle>,
    );

    fn run(
        &mut self,
        (
            dt,
            role,
            lazy,
            entities,
            position,
            mut effects,
            mut particles,
        ): Self::SystemData,
){
        if !role.graphical() {
            // If not graphical, we only send the effects to the clients once
            effects.clear();
            return;
        }

        let dt = dt.0;

        // Spawn particles from effects
        let mut rng = rand::thread_rng();
        for (ent, effect, pos) in (&*entities, &mut effects, &position).join()
        {
            match effect.effect {
                EffectInner::Explosion(size) => {
                    let lifetime = 0.4 * size.sqrt();
                    for _ in 0..(8.0 * size) as usize {
                        let ent = entities.create();
                        lazy.insert(
                            ent,
                            Position {
                                pos: [
                                    pos.pos[0]
                                        + 0.6 * rng.gen_range(-size, size),
                                    pos.pos[1]
                                        + 0.6 * rng.gen_range(-size, size),
                                ],
                                rot: rng.gen_range(0.0, 2.0 * PI),
                            },
                        );
                        lazy.insert(
                            ent,
                            Velocity {
                                vel: [
                                    rng.gen_range(-size, size),
                                    rng.gen_range(-size, size),
                                ],
                                rot: rng.gen_range(-5.0, 5.0),
                            },
                        );
                        particles.insert(
                            ent,
                            Particle {
                                lifetime: lifetime * rng.gen_range(0.7, 1.4),
                                which: ParticleType::Explosion,
                            },
                        ).unwrap();
                    }
                }
                EffectInner::MetalHit => for _ in 0..8 as usize {
                    let ent = entities.create();
                    lazy.insert(
                        ent,
                        Position {
                            pos: [
                                pos.pos[0] + rng.gen_range(-0.5, 0.5),
                                pos.pos[1] + rng.gen_range(-0.5, 0.5),
                            ],
                            rot: 0.0,
                        },
                    );
                    lazy.insert(
                        ent,
                        Velocity {
                            vel: [
                                rng.gen_range(-10.0, 10.0),
                                rng.gen_range(-10.0, 10.0),
                            ],
                            rot: 0.0,
                        },
                    );
                    particles.insert(
                        ent,
                        Particle {
                            lifetime: rng.gen_range(0.4, 0.6),
                            which: ParticleType::Spark,
                        },
                    ).unwrap();
                },
                EffectInner::LaserHit => {
                    let ent = entities.create();
                    lazy.insert(ent, pos.clone());
                    lazy.insert(
                        ent,
                        Particle {
                            lifetime: 0.2,
                            which: ParticleType::LaserHit,
                        },
                    );
                }
            }

            effect.lifetime -= dt;
            if effect.lifetime <= 0.0 {
                entities.delete(ent).unwrap();
            }
        }

        // Update particles' lifetime and delete dead ones
        for (ent, mut particle) in (&*entities, &mut particles).join() {
            particle.lifetime -= dt;
            if particle.lifetime < 0.0 {
                entities.delete(ent).unwrap();
            }
        }
    }
}
