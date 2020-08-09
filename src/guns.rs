//! Guns and projectiles.

use specs::{Component, Entities, Entity, Read, ReadExpect, Join, LazyUpdate,
            ReadStorage, System, VecStorage, WriteStorage};
use vecmath::*;

use crate::Role;
use crate::blocks::Blocky;
#[cfg(feature = "network")]
use crate::net;
use crate::particles::{Effect, EffectInner};
use crate::physics::{affect_area, delete_entity, AABox, DetectCollision,
                     HitEffect, Hits, Position, Velocity};

pub enum ProjectileType {
    Plasma,
    Rail,
}

impl ProjectileType {
    pub fn speed(&self) -> f32 {
        match *self {
            ProjectileType::Plasma => 60.0,
            ProjectileType::Rail => 35.0,
        }
    }

    pub fn mass(&self) -> Option<f32> {
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
        lazy: &Read<LazyUpdate>,
        pos: [f32; 2],
        rot: f32,
        kind: ProjectileType,
        shooter: Entity,
    ) -> Entity {
        let entity = entities.create();
        let (s, c) = rot.sin_cos();
        lazy.insert(
            entity,
            Position {
                pos: pos,
                rot: rot,
            },
        );
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
        ReadExpect<'a, Role>,
        Read<'a, LazyUpdate>,
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
