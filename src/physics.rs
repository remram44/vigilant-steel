//! Common components and behaviors for entities.

use Role;
#[cfg(feature = "network")]
use net;
use specs::{Component, Entities, Entity, Fetch, HashMapStorage, Join,
            LazyUpdate, NullStorage, ReadStorage, System, VecStorage,
            WriteStorage};
use std::cmp::Ordering;
use std::f64::consts::PI;
use utils::IteratorExt;
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
            pos.pos = vec2_add(pos.pos, vec2_scale(vel.vel, 200.0 * dt));
            pos.rot += vel.rot * dt;
            pos.rot %= 2.0 * PI;
        }
    }
}

/// Collision detection and response.
pub struct SysCollision;

#[derive(Clone, PartialEq)]
struct Projection {
    proj: f64,
    orig: [f64; 2],
}

impl PartialOrd for Projection {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.proj.partial_cmp(&other.proj)
    }
}

/// Checks if two shapes collide when projected on a specific axis.
///
/// This is part of the SAT collision detection method.
fn check_sat_collision_dir(
    pos1: &Position,
    size1: &[f64; 2],
    pos2: &Position,
    size2: &[f64; 2],
    dir: [f64; 2],
) -> Option<(f64, [f64; 2])> {
    // This is called for each normal of each rectangle
    // It checks whether there is collision of the shape projected along it

    let corners = &[(-1.0, -1.0), (-1.0, 1.0), (1.0, 1.0), (1.0, -1.0)];
    // Project rectangle 1
    let (s, c) = pos1.rot.sin_cos();
    let proj1 = corners
        .iter()
        .map(|&(x, y)| {
            // Compute corner coordinates
            let corner = vec2_add(
                pos1.pos,
                [
                    size1[0] * x * c + size1[1] * y * (-s),
                    size1[0] * x * s + size1[1] * y * c,
                ],
            );
            // Dot product with dir vector gives the distance along that vector
            Projection {
                proj: vec2_dot(corner, dir) as f64,
                orig: corner,
            }
        })
        .minmax()
        .unwrap();
    // Project rectangle 2
    let (s, c) = pos2.rot.sin_cos();
    let proj2 = corners
        .iter()
        .map(|&(x, y)| {
            // Compute corner coordinates
            let corner = vec2_add(
                pos2.pos,
                [
                    size2[0] * x * c + size2[1] * y * (-s),
                    size2[0] * x * s + size2[1] * y * c,
                ],
            );
            // Dot product with dir vector gives the distance along that vector
            Projection {
                proj: vec2_dot(corner, dir) as f64,
                orig: corner,
            }
        })
        .minmax()
        .unwrap();

    if proj1.0.proj < proj2.1.proj && proj2.0.proj < proj1.1.proj {
        let dist1 = proj2.1.proj - proj1.0.proj;
        let dist2 = proj1.1.proj - proj2.0.proj;
        if dist1 < dist2 {
            Some((dist1, proj2.1.orig))
        } else {
            Some((dist2, proj2.0.orig))
        }
    } else {
        None
    }
}

/// Checks if two shapes collide when projected on a specific axis.
///
/// Uses SAT to check if two rectangles collide.
/// If a collision is detected, returns the penetration axis (as a unit vector)
/// and depth.
fn check_sat_collision(
    pos1: &Position,
    size1: &[f64; 2],
    pos2: &Position,
    size2: &[f64; 2],
) -> Option<([f64; 2], f64, [f64; 2])> {
    let (s, c) = pos1.rot.sin_cos();
    let mut dir = [c, s];
    let (mut depth, mut loc) =
        check_sat_collision_dir(pos1, size1, pos2, size2, dir)?;

    let d = [-s, c];
    let r = check_sat_collision_dir(pos1, size1, pos2, size2, d)?;
    if r.0 < depth {
        dir = d;
        depth = r.0;
        loc = r.1;
    }

    let (s, c) = pos2.rot.sin_cos();
    let d = [c, s];
    let r = check_sat_collision_dir(pos2, size2, pos1, size1, d)?;
    if r.0 < depth {
        dir = d;
        depth = r.0;
        loc = r.1;
    }
    let d = [-s, c];
    let r = check_sat_collision_dir(pos2, size2, pos1, size1, d)?;
    if r.0 < depth {
        dir = d;
        depth = r.0;
        loc = r.1;
    }
    Some((dir, depth, loc))
}

impl<'a> System<'a> for SysCollision {
    type SystemData = (
        Fetch<'a, Role>,
        Fetch<'a, LazyUpdate>,
        Entities<'a>,
        WriteStorage<'a, Position>,
        ReadStorage<'a, Collision>,
        WriteStorage<'a, Collided>,
    );

    fn run(
        &mut self,
        (role, lazy, entities, pos, collision, mut collided): Self::SystemData,
    ) {
        assert!(role.authoritative());

        collided.clear();
        for (s_e, s_pos, s_col) in (&*entities, &pos, &collision).join() {
            for (o_e, o_pos, o_col) in (&*entities, &pos, &collision).join() {
                if s_e == o_e {
                    continue;
                }
                // Detect collisions using SAT
                if let Some((_dir, _depth, loc)) = check_sat_collision(
                    &s_pos,
                    &s_col.bounding_box,
                    &o_pos,
                    &o_col.bounding_box,
                ) {
                    let (s, c) = s_pos.rot.sin_cos();
                    let x = loc[0] - s_pos.pos[0];
                    let y = loc[1] - s_pos.pos[1];
                    let rel_loc = [x * c + y * s, -x * s + y * c];
                    let insert = if let Some(col) = collided.get_mut(s_e) {
                        col.hits.push(Hit {
                            entity: o_e,
                            rel_location: rel_loc,
                        });
                        false
                    } else {
                        true
                    };
                    if insert {
                        collided.insert(
                            s_e,
                            Collided {
                                hits: vec![
                                    Hit {
                                        entity: o_e,
                                        rel_location: rel_loc,
                                    },
                                ],
                            },
                        );
                    }
                    #[cfg(feature = "debug_markers")]
                    {
                        let me = entities.create();
                        lazy.insert(me, Marker { loc: loc, frame: 0 });
                    }
                    #[cfg(feature = "network")]
                    lazy.insert(s_e, net::Dirty);
                }
            }
        }
    }
}
