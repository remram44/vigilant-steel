use physics::Position;
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

/// Checks if two rectangles collide when projected on a specific axis.
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

pub struct Collision {
    pub direction: [f64; 2],
    pub depth: f64,
    pub location: [f64; 2],
}

/// Checks if two rectangles collide when projected on a specific axis.
///
/// Uses SAT to check if two rectangles collide.
/// If a collision is detected, returns the penetration axis (as a unit vector)
/// and depth.
pub fn find(
    pos1: &Position,
    size1: &[f64; 2],
    pos2: &Position,
    size2: &[f64; 2],
) -> Option<Collision> {
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
    Some(Collision {
        direction: dir,
        depth: depth,
        location: loc,
    })
}
