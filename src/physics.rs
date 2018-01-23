use std::f64::consts::PI;

use specs::{Entity, Component, System,
            Entities, ReadStorage, WriteStorage, Join,
            Fetch, HashMapStorage, NullStorage, VecStorage};
use vecmath::*;

use utils::IteratorExt;

// Position component, for entities that are in the world
#[derive(Debug)]
pub struct Position {
    pub pos: [f64; 2],
    pub rot: f64,
}

impl Component for Position {
    type Storage = VecStorage<Self>;
}

// Velocity component, for entities that move
#[derive(Debug)]
pub struct Velocity {
    pub vel: [f64; 2],
    pub rot: f64,
}

impl Component for Velocity {
    type Storage = VecStorage<Self>;
}

// Collision shapes; currently only axes-oriented rectangle
pub struct Collision {
    pub bounding_box: [f64; 2],
}

impl Component for Collision {
    type Storage = VecStorage<Self>;
}

// Collision information: this flags an entity has having collided
pub struct Collided {
    pub entity: Entity,
}

impl Component for Collided {
    type Storage = HashMapStorage<Self>;
}

// Marks that this entity is controlled by the local player
#[derive(Default)]
pub struct LocalControl;

impl Component for LocalControl {
    type Storage = NullStorage<Self>;
}

// Delta resource, stores the simulation step
pub struct DeltaTime(pub f64);

// Simulation system, updates positions from velocities
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

// Collision response
pub struct SysCollision;

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
                        collided.insert(s_e, Collided { entity: o_e });
                    }
                }
            }
        }
    }
}
