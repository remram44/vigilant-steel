//! Common components and behaviors for entities.

use Role;
#[cfg(feature = "network")]
use net;
use sat;
use specs::{Component, Entities, Entity, Fetch, HashMapStorage, Join,
            LazyUpdate, NullStorage, ReadStorage, System, VecStorage,
            WriteStorage};
use std::f64::consts::PI;
use vecmath::*;

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

/// Collision shapes; currently only axes-oriented rectangle.
///
/// Entities with Collision components will be checked for collisions, and a
/// Collided component will be added to them when it happens.
pub struct Collision {
    pub bounding_box: [f64; 2],
    pub mass: f64,
    pub inertia: f64,
}

impl Component for Collision {
    type Storage = VecStorage<Self>;
}

/// A single collision, stored in the Collided component.
pub struct Hit {
    /// Entity we collided with.
    pub entity: Entity,
    /// Location of the hit, in this entity's coordinate system.
    pub rel_location: [f64; 2],
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
        ReadStorage<'a, Collision>,
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
            collision,
            mut collided,
        ): Self::SystemData,
){
        assert!(role.authoritative());

        collided.clear();
        let mut hits = Vec::new();
        for (e1, pos1, col1) in (&*entities, &pos, &collision).join() {
            for (e2, pos2, col2) in (&*entities, &pos, &collision).join() {
                if e1 >= e2 {
                    continue;
                }
                // Detect collisions using SAT
                if let Some(hit) = sat::find(
                    &pos1,
                    &col1.bounding_box,
                    &pos2,
                    &col2.bounding_box,
                ) {
                    mark_collision(e1, pos1, e2, &hit, &mut collided);
                    mark_collision(e2, pos2, e1, &hit, &mut collided);

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
    }
}

fn mark_collision<'a>(
    ent: Entity,
    pos: &Position,
    o_ent: Entity,
    hit: &sat::Collision,
    collided: &mut WriteStorage<'a, Collided>,
) {
    // Compute location in object space
    let (s, c) = pos.rot.sin_cos();
    let x = hit.location[0] - pos.pos[0];
    let y = hit.location[1] - pos.pos[1];
    let rel_loc = [x * c + y * s, -x * s + y * c];

    // Add hit in a Collided component
    let insert = if let Some(col) = collided.get_mut(ent) {
        col.hits.push(Hit {
            entity: o_ent,
            rel_location: rel_loc,
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
    pos: &Position,
    vel: &Velocity,
    col: &Collision,
    o_ent: Entity,
    o_pos: &Position,
    o_vel: &Velocity,
    o_col: &Collision,
    hit: &sat::Collision,
    lazy: &Fetch<'a, LazyUpdate>,
    entities: &Entities<'a>,
) {
    // Compute impulse
    let rap = vec2_sub(hit.location, pos.pos);
    let rbp = vec2_sub(hit.location, o_pos.pos);
    let vab1 = vec2_sub(
        vec2_add(vel.vel, cross(rap, -vel.rot)),
        vec2_add(o_vel.vel, cross(rbp, -o_vel.rot)),
    );
    let n = hit.direction;
    let ma = col.mass;
    let mb = o_col.mass;
    let ia = col.inertia;
    let ib = o_col.inertia;

    let impulse = (-(1.0 + ELASTICITY) * vec2_dot(vab1, n))
        / (1.0 / ma + 1.0 / mb + cross_dot2(rap, n) / ia
            + cross_dot2(rbp, n) / ib);

    #[cfg(feature = "network")]
    lazy.insert(ent, net::Dirty);
}
