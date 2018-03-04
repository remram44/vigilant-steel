//! Common components and behaviors for entities.

use Role;
use blocks::Blocky;
#[cfg(feature = "network")]
use net;
use sat;
use specs::{Component, Entities, Entity, Fetch, HashMapStorage, Join,
            LazyUpdate, NullStorage, ReadStorage, System, VecStorage,
            WriteStorage};
use std::f64::consts::PI;
use vecmath::*;

/// Bounding-box.
#[derive(Debug)]
pub struct AABox {
    pub xmin: f64,
    pub xmax: f64,
    pub ymin: f64,
    pub ymax: f64,
}

impl AABox {
    pub fn empty() -> AABox {
        AABox {
            xmin: ::std::f64::INFINITY,
            xmax: -::std::f64::INFINITY,
            ymin: ::std::f64::INFINITY,
            ymax: -::std::f64::INFINITY,
        }
    }

    pub fn corners(&self) -> [[f64; 2]; 4] {
        [
            [self.xmin, self.ymin],
            [self.xmax, self.ymin],
            [self.xmax, self.ymax],
            [self.xmin, self.ymax],
        ]
    }
}

/// Wrapper for entity deletion that triggers network update.
pub fn delete_entity(
    role: Role,
    entities: &Entities,
    lazy: &Fetch<LazyUpdate>,
    entity: Entity,
) {
    #[cfg(feature = "network")]
    {
        assert!(role.authoritative());
        if role.networked() {
            lazy.insert(entity, net::Delete);
        } else {
            entities.delete(entity).unwrap();
        }
    }

    #[cfg(not(feature = "network"))]
    {
        entities.delete(entity).unwrap();
    }
}

/// Position component, for entities that are somewhere in the world.
#[derive(Debug)]
pub struct Position {
    pub pos: [f64; 2],
    pub rot: f64,
}

impl Component for Position {
    type Storage = VecStorage<Self>;
}

/// Velocity component, for entities that move.
#[derive(Debug)]
pub struct Velocity {
    pub vel: [f64; 2],
    pub rot: f64,
}

impl Component for Velocity {
    type Storage = VecStorage<Self>;
}

/// Special collision.
///
/// No built-in collision response, just detect collision and mark that object.
/// Don't even mark the other object.
pub struct DetectCollision {
    pub bounding_box: AABox,
}

impl Component for DetectCollision {
    type Storage = VecStorage<Self>;
}

/// A single collision, stored in the Collided component.
pub struct Hit {
    /// Entity we collided with.
    pub entity: Entity,
    /// Location of the hit, in this entity's coordinate system.
    pub rel_location: [f64; 2],
    /// Impulse differential.
    pub impulse: f64,
}

/// Collision information: this flags an entity as having collided.
pub struct Collided {
    pub hits: Vec<Hit>,
}

impl Component for Collided {
    type Storage = HashMapStorage<Self>;
}

/// Marks that this entity is controlled by the local player.
#[derive(Default)]
pub struct LocalControl;

impl Component for LocalControl {
    type Storage = NullStorage<Self>;
}

/// Delta resource, stores the simulation step.
pub struct DeltaTime(pub f64);

#[cfg(feature = "debug_markers")]
pub struct Marker {
    pub loc: [f64; 2],
    pub frame: u32,
}

#[cfg(feature = "debug_markers")]
impl Component for Marker {
    type Storage = VecStorage<Self>;
}

#[cfg(feature = "debug_markers")]
pub struct Arrow {
    pub ends: [[f64; 2]; 2],
    pub frame: u32,
}

#[cfg(feature = "debug_markers")]
impl Component for Arrow {
    type Storage = VecStorage<Self>;
}

/// Simulation system, updates positions from velocities.
pub struct SysSimu;

impl<'a> System<'a> for SysSimu {
    type SystemData = (
        Fetch<'a, DeltaTime>,
        WriteStorage<'a, Position>,
        ReadStorage<'a, Velocity>,
    );

    fn run(&mut self, (dt, mut pos, vel): Self::SystemData) {
        let dt = dt.0;
        for (pos, vel) in (&mut pos, &vel).join() {
            pos.pos = vec2_add(pos.pos, vec2_scale(vel.vel, dt));
            pos.rot += vel.rot * dt;
            pos.rot %= 2.0 * PI;
        }
    }
}

/// Collision detection and response.
pub struct SysCollision;

impl<'a> System<'a> for SysCollision {
    type SystemData = (
        Fetch<'a, Role>,
        Fetch<'a, LazyUpdate>,
        Entities<'a>,
        WriteStorage<'a, Position>,
        WriteStorage<'a, Velocity>,
        ReadStorage<'a, Blocky>,
        ReadStorage<'a, DetectCollision>,
        WriteStorage<'a, Collided>,
    );

    fn run(
        &mut self,
        (
            role,
            lazy,
            entities,
            mut pos,
            mut vel,
            blocky,
            collision,
            mut collided,
        ): Self::SystemData,
    ) {
        assert!(role.authoritative());

        collided.clear();

        // Detect collisions between Blocky objects
        let mut hits = Vec::new();
        for (e1, pos1, blocky1) in (&*entities, &pos, &blocky).join() {
            for (e2, pos2, blocky2) in (&*entities, &pos, &blocky).join() {
                if e2 >= e1 {
                    break;
                }
                // Detect collisions using SAT
                if let Some(hit) = sat::find(
                    &pos1,
                    &blocky1.bounding_box,
                    &pos2,
                    &blocky2.bounding_box,
                ) {
                    #[cfg(feature = "debug_markers")]
                    {
                        let me = entities.create();
                        lazy.insert(
                            me,
                            Marker {
                                loc: hit.location,
                                frame: 0,
                            },
                        );
                    }

                    hits.push((e1, e2, hit));
                }
            }
        }

        // Detect collisions between Blocky and DetectCollision objects
        for (e1, pos1, col1) in (&*entities, &pos, &collision).join() {
            for (e2, pos2, blocky2) in (&*entities, &pos, &blocky).join() {
                // Detect collisions using SAT
                if let Some(hit) = sat::find(
                    &pos1,
                    &col1.bounding_box,
                    &pos2,
                    &blocky2.bounding_box,
                ) {
                    #[cfg(feature = "debug_markers")]
                    {
                        let me = entities.create();
                        lazy.insert(
                            me,
                            Marker {
                                loc: hit.location,
                                frame: 0,
                            },
                        );
                    }

                    store_collision(
                        pos1,
                        hit.location,
                        0.0,
                        e1,
                        e2,
                        &mut collided,
                    );
                    store_collision(
                        pos2,
                        hit.location,
                        0.0,
                        e2,
                        e1,
                        &mut collided,
                    );
                }
            }
        }

        for (e1, e2, hit) in hits {
            handle_collision(
                e1,
                e2,
                &mut pos,
                &mut vel,
                &blocky,
                &mut collided,
                &hit,
                &lazy,
                &entities,
            );
        }
    }
}

fn store_collision<'a>(
    pos: &Position,
    hit: [f64; 2],
    impulse: f64,
    ent: Entity,
    o_ent: Entity,
    collided: &mut WriteStorage<'a, Collided>,
) {
    let (s, c) = pos.rot.sin_cos();
    let x = hit[0] - pos.pos[0];
    let y = hit[1] - pos.pos[1];
    let rel_loc = [x * c + y * s, -x * s + y * c];

    // Add hit in a Collided component
    let insert = if let Some(col) = collided.get_mut(ent) {
        col.hits.push(Hit {
            entity: o_ent,
            rel_location: rel_loc,
            impulse: impulse,
        });
        false
    } else {
        true
    };
    if insert {
        collided.insert(
            ent,
            Collided {
                hits: vec![
                    Hit {
                        entity: o_ent,
                        rel_location: rel_loc,
                        impulse: impulse,
                    },
                ],
            },
        );
    }
}

const ELASTICITY: f64 = 0.6;

/// Cross-product of planar vector with orthogonal vector.
fn cross(a: [f64; 2], b: f64) -> [f64; 2] {
    [a[1] * b, -a[0] * b]
}

/// Compute cross product of planar vectors and take dot with itself.
fn cross_dot2(a: [f64; 2], b: [f64; 2]) -> f64 {
    let c = a[0] * b[1] - a[1] * b[0];
    c * c
}

fn handle_collision<'a>(
    ent: Entity,
    o_ent: Entity,
    position: &mut WriteStorage<'a, Position>,
    velocity: &mut WriteStorage<'a, Velocity>,
    blocky: &ReadStorage<'a, Blocky>,
    collided: &mut WriteStorage<'a, Collided>,
    hit: &sat::Collision,
    lazy: &Fetch<'a, LazyUpdate>,
    entities: &Entities<'a>,
) {
    let blk = blocky.get(ent).unwrap();
    let o_blk = blocky.get(o_ent).unwrap();
    let (impulse, rap, rbp) = {
        let pos = position.get(ent).unwrap();
        let o_pos = position.get(o_ent).unwrap();
        let vel = velocity.get(ent).unwrap();
        let o_vel = velocity.get(o_ent).unwrap();

        // Compute impulse
        let rap = vec2_sub(hit.location, pos.pos);
        let rbp = vec2_sub(hit.location, o_pos.pos);
        let vab1 = vec2_sub(
            vec2_add(vel.vel, cross(rap, -vel.rot)),
            vec2_add(o_vel.vel, cross(rbp, -o_vel.rot)),
        );
        let n = hit.direction;
        let ma = blk.mass;
        let mb = o_blk.mass;
        let ia = blk.inertia;
        let ib = o_blk.inertia;

        (
            (-(1.0 + ELASTICITY) * vec2_dot(vab1, n))
                / (1.0 / ma + 1.0 / mb + cross_dot2(rap, n) / ia
                    + cross_dot2(rbp, n) / ib),
            rap,
            rbp,
        )
    };

    {
        // Compute location in object space
        let pos = position.get_mut(ent).unwrap();
        store_collision(pos, hit.location, impulse, ent, o_ent, collided);

        // Move object out of collision
        pos.pos = vec2_add(
            pos.pos,
            vec2_scale(hit.direction, hit.depth * 0.5 + 0.05),
        );

        // Update velocity
        let vel = velocity.get_mut(ent).unwrap();
        vel.vel =
            vec2_add(vel.vel, vec2_scale(hit.direction, impulse / blk.mass));
        vel.rot += impulse
            * (rap[0] * hit.direction[1] - rap[1] * hit.direction[0])
            / blk.inertia;
    }
    {
        // Compute location in object space
        let pos = position.get_mut(o_ent).unwrap();
        store_collision(pos, hit.location, impulse, o_ent, ent, collided);

        // Move object out of collision
        pos.pos = vec2_add(
            pos.pos,
            vec2_scale(hit.direction, -(hit.depth * 0.5 + 0.05)),
        );

        // Update velocity
        let vel = velocity.get_mut(o_ent).unwrap();
        vel.vel = vec2_add(
            vel.vel,
            vec2_scale(hit.direction, -impulse / o_blk.mass),
        );
        vel.rot += -impulse
            * (rbp[0] * hit.direction[1] - rbp[1] * hit.direction[0])
            / o_blk.inertia;
    }

    #[cfg(feature = "debug_markers")]
    {
        let me = entities.create();
        lazy.insert(
            me,
            Arrow {
                ends: [
                    hit.location,
                    vec2_add(
                        hit.location,
                        vec2_scale(hit.direction, impulse * 10.0),
                    ),
                ],
                frame: 0,
            },
        );
    }

    #[cfg(feature = "network")]
    lazy.insert(ent, net::Dirty);
}
