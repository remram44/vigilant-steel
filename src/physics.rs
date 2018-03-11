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
use std::ops::Deref;
use tree;
use vecmath::*;

/// Bounding-box.
#[derive(Debug, Clone)]
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

    pub fn sq_radius(&self) -> f64 {
        self.corners()
            .iter()
            .map(|&c| vec2_square_len(c))
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap()
    }

    /// Add a square of size 1 by the location of its center.
    pub fn add_square1(&mut self, point: [f64; 2]) {
        *self = AABox {
            xmin: self.xmin.min(point[0] - 0.5),
            xmax: self.xmax.max(point[0] + 0.5),
            ymin: self.ymin.min(point[1] - 0.5),
            ymax: self.ymax.max(point[1] + 0.5),
        };
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
#[derive(Debug, Clone)]
pub struct Position {
    pub pos: [f64; 2],
    pub rot: f64,
}

impl Component for Position {
    type Storage = VecStorage<Self>;
}

/// Velocity component, for entities that move.
#[derive(Debug, Clone)]
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
    pub mass: Option<f64>,
}

impl Component for DetectCollision {
    type Storage = VecStorage<Self>;
}

/// Attached to a Hit, indicates the effect on the receiving entity.
#[derive(Clone)]
pub enum HitEffect {
    /// Material collision, such as between block objects.
    Collision(f64),
    /// Caught in an explosion.
    Explosion(f64),
}

/// A single collision, stored in the Hits component.
pub struct Hit {
    /// Location of the hit, in this entity's coordinate system.
    pub rel_location: [f64; 2],
    pub effect: HitEffect,
}

/// Collision information: this flags an entity as having collided.
pub struct Hits {
    hits_vec: Vec<Hit>,
}

impl Hits {
    pub fn record<'a>(
        hits: &mut WriteStorage<'a, Hits>,
        ent: Entity,
        hit: Hit,
    ) {
        if let Some(hits) = hits.get_mut(ent) {
            hits.hits_vec.push(hit);
            return;
        }
        hits.insert(
            ent,
            Hits {
                hits_vec: vec![hit],
            },
        );
    }
}

impl Component for Hits {
    type Storage = HashMapStorage<Self>;
}

impl Deref for Hits {
    type Target = [Hit];

    fn deref(&self) -> &[Hit] {
        &self.hits_vec
    }
}

/// Marks that this entity is controlled by the local player.
#[derive(Default)]
pub struct LocalControl;

impl Component for LocalControl {
    type Storage = NullStorage<Self>;
}

/// Delta resource, stores the simulation step.
pub struct DeltaTime(pub f64);

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
        WriteStorage<'a, Hits>,
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
            mut hits,
        ): Self::SystemData,
){
        assert!(role.authoritative());

        hits.clear();

        // Detect collisions between Blocky objects
        let mut block_hits = Vec::new();
        for (e1, pos1, blocky1) in (&*entities, &pos, &blocky).join() {
            for (e2, pos2, blocky2) in (&*entities, &pos, &blocky).join() {
                if e2 >= e1 {
                    break;
                }
                if blocky1.blocks.is_empty() || blocky2.blocks.is_empty() {
                    continue;
                }
                // Detect collisions using tree
                if let Some(hit) = find_collision_tree(
                    pos1,
                    &blocky1.tree,
                    0,
                    pos2,
                    &blocky2.tree,
                    0,
                ) {
                    block_hits.push((e1, e2, hit));
                }
            }
        }

        for (e1, e2, hit) in block_hits {
            handle_collision(
                e1,
                e2,
                &mut pos,
                &mut vel,
                &blocky,
                &mut hits,
                &hit,
                &lazy,
            );
        }

        // Detect collisions between Blocky and DetectCollision objects
        for (e2, pos2, blocky2) in (&*entities, &pos, &blocky).join() {
            for (e1, pos1, col1) in (&*entities, &pos, &collision).join() {
                if blocky2.blocks.is_empty() {
                    continue;
                }
                // Detect collisions using tree
                if let Some(hit) = find_collision_tree_box(
                    pos1,
                    &col1.bounding_box,
                    pos2,
                    &blocky2.tree,
                    0,
                ) {
                    let vel1 = vel.get(e1).unwrap().vel;
                    let vel2 = vel.get(e2).unwrap().vel;
                    let momentum = vec2_sub(vel1, vel2);
                    let momentum = vec2_len(momentum) * blocky2.mass;
                    store_collision(
                        pos1,
                        hit.location,
                        HitEffect::Collision(momentum),
                        e1,
                        &mut hits,
                    );
                    if let Some(mass1) = col1.mass {
                        let impulse = vec2_scale(vel1, mass1);
                        let vel2 = vel.get_mut(e2).unwrap();
                        vel2.vel = vec2_add(
                            vel2.vel,
                            vec2_scale(impulse, 1.0 / blocky2.mass),
                        );
                        let rel = vec2_sub(hit.location, pos2.pos);
                        vel2.rot += (rel[0] * impulse[1] - rel[1] * impulse[0])
                            / blocky2.inertia;
                    }
                }
            }
        }
    }
}

fn find_collision_tree(
    pos1: &Position,
    tree1: &tree::Tree,
    idx1: usize,
    pos2: &Position,
    tree2: &tree::Tree,
    idx2: usize,
) -> Option<sat::Collision> {
    let n1 = &tree1.0[idx1];
    let n2 = &tree2.0[idx2];
    if let Some(hit) = sat::find(pos1, &n1.bounds, pos2, &n2.bounds) {
        if let tree::Content::Internal(left, right) = n1.content {
            match find_collision_tree(pos1, tree1, left, pos2, tree2, idx2) {
                None => {
                    find_collision_tree(pos1, tree1, right, pos2, tree2, idx2)
                }
                r => r,
            }
        } else if let tree::Content::Internal(left, right) = n2.content {
            match find_collision_tree(pos1, tree1, idx1, pos2, tree2, left) {
                None => {
                    find_collision_tree(pos1, tree1, idx1, pos2, tree2, right)
                }
                r => r,
            }
        } else {
            Some(hit)
        }
    } else {
        None
    }
}

fn find_collision_tree_box(
    pos1: &Position,
    box1: &AABox,
    pos2: &Position,
    tree2: &tree::Tree,
    idx2: usize,
) -> Option<sat::Collision> {
    let n2 = &tree2.0[idx2];
    if let Some(hit) = sat::find(pos1, box1, pos2, &n2.bounds) {
        if let tree::Content::Internal(left, right) = n2.content {
            match find_collision_tree_box(pos1, box1, pos2, tree2, left) {
                None => {
                    find_collision_tree_box(pos1, box1, pos2, tree2, right)
                }
                r => r,
            }
        } else {
            Some(hit)
        }
    } else {
        None
    }
}

fn store_collision<'a>(
    pos: &Position,
    hit: [f64; 2],
    effect: HitEffect,
    ent: Entity,
    hits: &mut WriteStorage<'a, Hits>,
) {
    let (s, c) = pos.rot.sin_cos();
    let x = hit[0] - pos.pos[0];
    let y = hit[1] - pos.pos[1];
    let rel_loc = [x * c + y * s, -x * s + y * c];

    Hits::record(
        hits,
        ent,
        Hit {
            rel_location: rel_loc,
            effect: effect,
        },
    );
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
    hits: &mut WriteStorage<'a, Hits>,
    hit: &sat::Collision,
    lazy: &Fetch<'a, LazyUpdate>,
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
        store_collision(
            pos,
            hit.location,
            HitEffect::Collision(impulse),
            ent,
            hits,
        );

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
        store_collision(
            pos,
            hit.location,
            HitEffect::Collision(impulse),
            o_ent,
            hits,
        );

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

    #[cfg(feature = "network")]
    lazy.insert(ent, net::Dirty);
}

pub fn affect_area<'a>(
    entities: &Entities<'a>,
    pos: &ReadStorage<'a, Position>,
    blocky: &ReadStorage<'a, Blocky>,
    hits: &mut WriteStorage<'a, Hits>,
    center: [f64; 2],
    radius: f64,
    effect: HitEffect,
) {
    let sq_radius = radius * radius;
    for (ent, pos, blk) in (&**entities, &*pos, &*blocky).join() {
        let entity_sq_radius = blk.tree.0[0].bounds.sq_radius();
        let dist = vec2_square_len(vec2_sub(pos.pos, center));
        if dist < sq_radius + entity_sq_radius {
            store_collision(pos, center, effect.clone(), ent, hits);
        }
    }
}
