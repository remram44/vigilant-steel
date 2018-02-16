//! Common components and behaviors for entities.

use std::f64::consts::PI;

use specs::{Entity, Component, System,
            Entities, ReadStorage, WriteStorage, Join,
            Fetch, HashMapStorage, NullStorage, VecStorage};
use vecmath::*;

use utils::IteratorExt;

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

/// Collision information: this flags an entity has having collided.
pub struct Collided {
    pub entities: Vec<Entity>,
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

/// Simulation system, updates positions from velocities.
pub struct SysSimu;

impl<'a> System<'a> for SysSimu {
    type SystemData = (Fetch<'a, DeltaTime>,
                       WriteStorage<'a, Position>,
                       ReadStorage<'a, Velocity>);

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

/// Checks for collisions between non-axis-oriented rectangles.
///
/// Uses SAT to check if two rectangles collide.
// TODO: replace with better method
fn check_sat_collision(
    s_pos: &Position, s_col: &Collision,
    o_pos: &Position, o_col: &Collision,
    dir: [f64; 2],
) -> bool {
    // This is called for each normal of each rectangle
    // It checks whether there is collision of the shape projected along it
    let sides = &[(-1.0, -1.0), (-1.0, 1.0),
                  (1.0, 1.0), (1.0, -1.0)];
    // Project S rectangle
    let (s, c) = s_pos.rot.sin_cos();
    let s_proj = sides.iter().map(|&(xs, ys)| {
        // Compute corner coordinates
        let corner = vec2_add(s_pos.pos, [s_col.bounding_box[0] * xs * c +
                                          s_col.bounding_box[1] * ys * (-s),
                                          s_col.bounding_box[0] * xs * s +
                                          s_col.bounding_box[1] * ys * c]);
        // Dot product with dir vector gives the distance along that vector
        vec2_dot(corner, dir) as f64
    }).minmax().unwrap();
    // Project O rectangle
    let (s, c) = o_pos.rot.sin_cos();
    let o_proj = sides.iter().map(|&(xs, ys)| {
        // Compute corner coordinates
        let corner = vec2_add(o_pos.pos, [o_col.bounding_box[0] * xs * c +
                                          o_col.bounding_box[1] * ys * (-s),
                                          o_col.bounding_box[0] * xs * s +
                                          o_col.bounding_box[1] * ys * c]);
        // Dot product with dir vector gives the distance along that vector
        vec2_dot(corner, dir) as f64
    }).minmax().unwrap();

    s_proj.0 < o_proj.1 && o_proj.0 < s_proj.1
}

/// Sliding square/fixed point collision
///
/// Finds the time of collision between a moving square and a fixed point.
/// The square is assumed to be aligned, centered on (0, 0) and of size 1.
fn square_point_collision(mut square_move: Vector2<f64>, mut target: Vector2<f64>)
    -> Option<f64>
{
    // Rotate so direction is positive
    if square_move[0] < 0.0 {
        if square_move[1] < 0.0 {
            square_move = [-square_move[0], -square_move[1]];
            target = [-target[0], -target[1]];
        } else {
            square_move = [square_move[1], -square_move[0]];
            target = [target[1], -target[0]];
        }
    } else if square_move[1] < 0.0 {
        square_move = [-square_move[1], square_move[0]];
        target = [-target[1], target[0]];
    }

    // Find collision with top
    let top = segment_point_collision([-0.5, 0.5], [0.5, 0.5],
                                      square_move, target);
    // Find collision with right
    let right = segment_point_collision([0.5, 0.5], [0.5, -0.5],
                                        square_move, target);
    match (top, right) {
        (Some(t), Some(r)) => Some(t.min(r)),
        (None, r) => r,
        (t, None) => t,
    }
}

/// Sliding line segment/fixed point collision
///
/// Finds the time of collision between a moving line segment and a fixed point.
/// Assumes that the segment has length 1.
fn segment_point_collision(seg_a: Vector2<f64>, seg_b: Vector2<f64>,
                           seg_move: Vector2<f64>, target: Vector2<f64>)
    -> Option<f64>
{
    let segdir = vec2_sub(seg_b, seg_a);
    let perdir = [segdir[1], -segdir[0]];

    // Distance to collision
    let dist = vec2_dot(perdir, vec2_sub(target, seg_a));
    // Speed of travel along perpendicular to segment/
    let proj = vec2_dot(perdir, seg_move);
    // Time of collision with line
    let t = dist / proj;
    if t < 0.0 {
        return None;
    }

    // We know when we hit the line, now find out if we hit the segment
    let line_pos = vec2_dot(segdir, vec2_sub(
        target,
        vec2_add(seg_a, vec2_scale(seg_move, t))));
    if 0.0 <= line_pos && line_pos <= 1.0 { // 1.0 == vec2_square_len(segdir)
        Some(t)
    } else {
        None
    }
}

impl<'a> System<'a> for SysCollision {
    type SystemData = (Entities<'a>,
                       WriteStorage<'a, Position>,
                       ReadStorage<'a, Collision>,
                       WriteStorage<'a, Collided>);

    fn run(
        &mut self,
        (entities, pos, collision, mut collided): Self::SystemData)
    {
        collided.clear();
        for (s_e, s_pos, s_col) in (&*entities, &pos, &collision).join() {
            for (o_e, o_pos, o_col) in (&*entities, &pos, &collision).join() {
                if s_e == o_e { continue; }
                // Detect collisions using SAT
                let (s_s, s_c) = s_pos.rot.sin_cos();
                if check_sat_collision(s_pos, s_col, o_pos, o_col,
                                       [s_c, s_s]) &&
                    check_sat_collision(s_pos, s_col, o_pos, o_col,
                                        [-s_s, s_c])
                {
                    let (o_s, o_c) = o_pos.rot.sin_cos();
                    if check_sat_collision(s_pos, s_col, o_pos, o_col,
                                           [o_c, o_s]) &&
                        check_sat_collision(s_pos, s_col, o_pos, o_col,
                                            [-o_s, o_c])
                    {
                        // Collision!
                        if let Some(col) = collided.get_mut(s_e) {
                            col.entities.push(o_e);
                            continue;
                        }
                        collided.insert(s_e, Collided { entities: vec![o_e] });
                    }
                }
            }
        }
    }
}
