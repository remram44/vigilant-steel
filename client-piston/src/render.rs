//! Rendering code, using Piston.

use game::blocks::{Block, BlockInner, Blocky};
use game::particles::{Particle, ParticleType};
use game::physics::Position;
use game::ship::{Projectile, ProjectileType};
use graphics::{self, Context, Graphics, Transformed};
use graphics::character::CharacterCache;
use graphics::math::Matrix2d;
use specs::{Join, World};
use std::fmt::Debug;

const MAX_RATIO: f64 = 1.6;
const VIEWPORT_SIZE: f64 = 80.0;

pub struct Viewport {
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

impl Viewport {
    pub fn new(size: [u32; 2]) -> Viewport {
        let (width, height) = (size[0] as f64, size[1] as f64);
        let maxsize = if width >= height {
            width.max(height * MAX_RATIO)
        } else {
            height.max(width * MAX_RATIO)
        };
        warn!(
            "Window is {}x{}, computed scale = {}",
            size[0],
            size[1],
            VIEWPORT_SIZE / maxsize
        );
        Viewport {
            width: size[0],
            height: size[1],
            scale: maxsize / VIEWPORT_SIZE,
        }
    }
}

/// Draws a line connecting points in sequence, then last to first.
///
/// This is similar to `graphics::polygon()` but only draws the outline.
fn draw_line_loop<G>(
    color: [f32; 4],
    radius: graphics::types::Radius,
    points: &[[f64; 2]],
    tr: Matrix2d,
    g: &mut G,
) where
    G: graphics::Graphics,
{
    let mut points = points.iter();
    let first = match points.next() {
        Some(p) => p,
        None => return,
    };
    let mut previous = first;
    for point in points {
        graphics::line(
            color,
            radius,
            [previous[0], previous[1], point[0], point[1]],
            tr,
            g,
        );
        previous = point;
    }
    graphics::line(
        color,
        radius,
        [previous[0], previous[1], first[0], first[1]],
        tr,
        g,
    );
}

fn draw_block<G: graphics::Graphics>(block: &Block, tr: Matrix2d, gl: &mut G) {
    match block.inner {
        BlockInner::Cockpit => {
            draw_line_loop(
                [1.0, 0.0, 0.0, 1.0],
                0.05,
                &[[-0.45, -0.45], [0.45, -0.45], [0.45, 0.45], [-0.45, 0.45]],
                tr,
                gl,
            );
            draw_line_loop(
                [1.0, 0.0, 0.0, 1.0],
                0.02,
                &[[-0.2, -0.3], [0.2, 0.0], [-0.2, 0.3]],
                tr,
                gl,
            );
        }
        BlockInner::Thruster { angle } => for i in &[-0.4, 0.0] {
            graphics::polygon(
                [0.4, 0.4, 0.4, 1.0],
                &[[0.45, 0.25], [0.05, 0.45], [0.05, -0.45], [0.45, -0.25]],
                tr.rot_rad(angle).trans(*i, 0.0),
                gl,
            );
        },
        BlockInner::PlasmaGun { angle, .. } => {
            draw_line_loop(
                [0.7, 0.7, 1.0, 1.0],
                0.05,
                &[
                    [-0.35, -0.35],
                    [0.0, -0.45],
                    [0.35, -0.35],
                    [0.45, 0.0],
                    [0.35, 0.35],
                    [0.0, 0.45],
                    [-0.35, 0.35],
                    [-0.45, 0.0],
                ],
                tr,
                gl,
            );
            graphics::polygon(
                [0.7, 0.7, 1.0, 1.0],
                &[[-0.0, -0.15], [0.6, -0.15], [0.6, 0.15], [-0.0, 0.15]],
                tr.rot_rad(angle),
                gl,
            );
        }
        BlockInner::RailGun { angle, .. } => {
            draw_line_loop(
                [0.7, 0.7, 1.0, 1.0],
                0.05,
                &[
                    [-0.35, -0.35],
                    [0.0, -0.45],
                    [0.35, -0.35],
                    [0.45, 0.0],
                    [0.35, 0.35],
                    [0.0, 0.45],
                    [-0.35, 0.35],
                    [-0.45, 0.0],
                ],
                tr,
                gl,
            );
            graphics::polygon(
                [0.7, 0.7, 1.0, 1.0],
                &[[-0.25, -0.25], [0.6, -0.25], [0.6, 0.25], [-0.25, 0.25]],
                tr.rot_rad(angle),
                gl,
            );
        }
        BlockInner::Armor => {
            draw_line_loop(
                [0.7, 0.7, 0.7, 1.0],
                0.05,
                &[[-0.45, -0.45], [0.45, -0.45], [0.45, 0.45], [-0.45, 0.45]],
                tr,
                gl,
            );
        }
        BlockInner::Rock => {
            draw_line_loop(
                [0.45, 0.45, 0.45, 1.0],
                0.05,
                &[[-0.45, -0.45], [0.45, -0.45], [0.45, 0.45], [-0.45, 0.45]],
                tr,
                gl,
            );
        }
    }
}

pub fn render<G, C, E>(
    context: Context,
    gl: &mut G,
    _glyph_cache: &mut C,
    world: &mut World,
) where
    G: graphics::Graphics,
    E: Debug,
    C: CharacterCache<Texture = <G as Graphics>::Texture, Error = E> + Sized,
{
    let viewport = world.read_resource::<Viewport>();
    let pos = world.read::<Position>();
    let projectile = world.read::<Projectile>();
    let particles = world.read::<Particle>();
    let blocky = world.read::<Blocky>();

    graphics::clear([0.0, 0.0, 0.1, 1.0], gl);

    let tr = context
        .transform
        .trans(viewport.width as f64 / 2.0, viewport.height as f64 / 2.0)
        .scale(viewport.scale, -viewport.scale);

    // Draw blocks
    for (pos, blocky) in (&pos, &blocky).join() {
        let blocks_tr = tr.trans(pos.pos[0], pos.pos[1]).rot_rad(pos.rot);
        for &(rel, ref block) in &blocky.blocks {
            draw_block(&block, blocks_tr.trans(rel[0], rel[1]), gl);
        }
    }

    // Draw projectiles
    for (pos, proj) in (&pos, &projectile).join() {
        let projectile_tr = tr.trans(pos.pos[0], pos.pos[1]).rot_rad(pos.rot);
        match proj.0 {
            ProjectileType::Plasma => {
                graphics::line(
                    [0.0, 1.0, 0.0, 1.0],
                    0.1,
                    [-0.8, 0.0, 0.8, 0.0],
                    projectile_tr,
                    gl,
                );
            }
            ProjectileType::Rail => {
                graphics::line(
                    [1.0, 1.0, 1.0, 1.0],
                    0.6,
                    [-0.8, 0.0, 0.8, 0.0],
                    projectile_tr,
                    gl,
                );
            }
        }
    }

    for (pos, particle) in (&pos, &particles).join() {
        let part_tr = tr.trans(pos.pos[0], pos.pos[1]).rot_rad(pos.rot);
        match particle.which {
            ParticleType::Spark => {
                let alpha = (particle.lifetime as f32) / 0.2;
                graphics::rectangle(
                    [1.0, 1.0, 1.0, alpha],
                    graphics::rectangle::centered([0.0, 0.0, 0.05, 0.05]),
                    part_tr,
                    gl,
                );
            }
            ParticleType::Exhaust => graphics::rectangle(
                [1.0, 1.0, 1.0, (particle.lifetime as f32).min(0.5)],
                graphics::rectangle::centered([0.0, 0.0, 0.3, 0.3]),
                part_tr,
                gl,
            ),
            ParticleType::Explosion => {
                let alpha = (particle.lifetime as f32 * 1.6).min(0.8);
                graphics::rectangle(
                    [1.0, particle.lifetime as f32 / 0.6, 0.0, alpha],
                    graphics::rectangle::centered([0.0, 0.0, 1.2, 1.2]),
                    part_tr,
                    gl,
                );
            }
            ParticleType::LaserHit => {
                let alpha = (particle.lifetime as f32 * 4.0).min(0.6);
                let size = (0.2 - particle.lifetime) * 15.0;
                graphics::ellipse(
                    [0.0, 1.0, 0.0, alpha],
                    graphics::rectangle::centered([0.0, 0.0, size, size]),
                    part_tr,
                    gl,
                );
            }
        }
    }
}
