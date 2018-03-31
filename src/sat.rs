//! Collision detection code using Separating Axis Theorem.
//!
//! This contains the low-level SAT code used by `physics.rs`. It detects
//! collisions and returns location, direction, and depth, but `SysCollision`
//! actually handles it.

use physics::{AABox, Position};
use std::cmp::Ordering;
use utils::IteratorExt;
use vecmath::*;

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

/// Little structure returned by `find()`.
pub struct Collision {
    pub direction: [f64; 2],
    pub depth: f64,
    pub location: [f64; 2],
}

/// Checks if two rectangles collide when projected on a specific axis.
///
/// This is part of the SAT collision detection method.
fn check_sat_collision_dir(
    pos1: &Position,
    size1: &AABox,
    pos2: &Position,
    size2: &AABox,
    dir: [f64; 2],
) -> Option<Collision> {
    // This is called for each normal of each rectangle
    // It checks whether there is collision of the shape projected along it

    // Project rectangle 1
    let (s, c) = pos1.rot.sin_cos();
    let proj1 = size1
        .corners()
        .iter()
        .map(|&corner| {
            // Compute corner coordinates
            let corner = vec2_add(
                pos1.pos,
                [
                    corner[0] * c - corner[1] * s,
                    corner[0] * s + corner[1] * c,
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
    let proj2 = size2
        .corners()
        .iter()
        .map(|&corner| {
            // Compute corner coordinates
            let corner = vec2_add(
                pos2.pos,
                [
                    corner[0] * c - corner[1] * s,
                    corner[0] * s + corner[1] * c,
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
            Some(Collision {
                direction: dir,
                depth: dist1,
                location: proj2.1.orig,
            })
        } else {
            Some(Collision {
                direction: [-dir[0], -dir[1]],
                depth: dist2,
                location: proj2.0.orig,
            })
        }
    } else {
        None
    }
}

/// Checks if two rectangles collide when projected on a specific axis.
///
/// Uses SAT to check if two rectangles collide.
/// If a collision is detected, returns the penetration axis (as a unit vector)
/// and depth.
pub fn find(
    pos1: &Position,
    size1: &AABox,
    pos2: &Position,
    size2: &AABox,
) -> Option<Collision> {
    let (s, c) = pos1.rot.sin_cos();
    let mut res = check_sat_collision_dir(pos1, size1, pos2, size2, [c, s])?;

    let r = check_sat_collision_dir(pos1, size1, pos2, size2, [-s, c])?;
    if r.depth < res.depth {
        res = r;
    }

    let (s, c) = pos2.rot.sin_cos();
    let r = check_sat_collision_dir(pos2, size2, pos1, size1, [c, s])?;
    if r.depth < res.depth {
        res = r;
        res.direction = [-res.direction[0], -res.direction[1]];
    }
    let r = check_sat_collision_dir(pos2, size2, pos1, size1, [-s, c])?;
    if r.depth < res.depth {
        res = r;
        res.direction = [-res.direction[0], -res.direction[1]];
    }
    Some(res)
}
