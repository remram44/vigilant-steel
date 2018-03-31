//! Rendering code, using Piston.

use game::blocks::{Block, BlockInner, Blocky};
use game::particles::{Particle, ParticleType};
use game::physics::{LocalControl, Position};
use game::ship::{Projectile, ProjectileType};
use graphics::character::CharacterCache;
use graphics::math::Matrix2d;
use graphics::{self, Context, Graphics, Transformed};
use rand::{Rng, SeedableRng, XorShiftRng};
use specs::{Join, World};
use std::fmt::Debug;
use vecmath::*;

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
            maxsize / VIEWPORT_SIZE
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

fn draw_background<G: graphics::Graphics>(
    viewport: &Viewport,
    pos: [f64; 2],
    tr: Matrix2d,
    g: &mut G,
) {
    // First layer: bright, almost move with world
    draw_background_layer(
        pos[0],
        pos[1],
        viewport,
        0.6,
        [1.0, 1.0, 1.0, 0.4],
        50,
        2,
        tr,
        g,
    );

    // Second layer: less bright, move much slower
    draw_background_layer(
        pos[0],
        pos[1],
        viewport,
        0.4,
        [1.0, 1.0, 1.0, 0.05],
        30,
        3,
        tr,
        g,
    );
}

fn draw_background_layer<G: graphics::Graphics>(
    xpos: f64,
    ypos: f64,
    viewport: &Viewport,
    speed: f64,
    color: [f32; 4],
    nb: usize,
    seed: i32,
    tr: Matrix2d,
    g: &mut G,
) {
    let xpos = xpos * speed;
    let ypos = ypos * speed;

    let width = viewport.width as f64 * 0.5 / viewport.scale;
    let height = viewport.height as f64 * 0.5 / viewport.scale;

    let xmin = ((xpos - width) / 50.0).floor() as i32;
    let xmax = ((xpos + width) / 50.0).ceil() as i32;
    let ymin = ((ypos - height) / 50.0).floor() as i32;
    let ymax = ((ypos + height) / 50.0).ceil() as i32;

    for x in xmin..xmax {
        for y in ymin..ymax {
            let seed = (seed * (1 + 2 * x + 1024 * y)) as u32;
            let mut rng = XorShiftRng::from_seed([
                seed,
                seed >> 8,
                seed >> 16,
                seed >> 24,
            ]);
            let tr = tr.trans(
                x as f64 * 50.0 - xpos + xpos / speed,
                y as f64 * 50.0 - ypos + ypos / speed,
            );
            for _ in 0..nb {
                let point = [
                    rng.gen_range(0.0, 50.0),
                    rng.gen_range(0.0, 50.0),
                ];
                graphics::rectangle(
                    color,
                    graphics::rectangle::centered([
                        point[0], point[1], 0.05, 0.05
                    ]),
                    tr,
                    g,
                );
            }
        }
    }
}

fn draw_block<G: graphics::Graphics>(block: &Block, tr: Matrix2d, g: &mut G) {
    match block.inner {
        BlockInner::Cockpit => {
            draw_line_loop(
                [1.0, 0.0, 0.0, 1.0],
                0.05,
                &[
                    [-0.45, -0.45],
                    [0.45, -0.45],
                    [0.45, 0.45],
                    [-0.45, 0.45],
                ],
                tr,
                g,
            );
            draw_line_loop(
                [1.0, 0.0, 0.0, 1.0],
                0.02,
                &[[-0.2, -0.3], [0.2, 0.0], [-0.2, 0.3]],
                tr,
                g,
            );
        }
        BlockInner::Thruster { angle } => for i in &[-0.4, 0.0] {
            graphics::polygon(
                [0.4, 0.4, 0.4, 1.0],
                &[
                    [0.45, 0.25],
                    [0.05, 0.45],
                    [0.05, -0.45],
                    [0.45, -0.25],
                ],
                tr.rot_rad(angle).trans(*i, 0.0),
                g,
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
                g,
            );
            graphics::polygon(
                [0.7, 0.7, 1.0, 1.0],
                &[
                    [-0.0, -0.15],
                    [0.6, -0.15],
                    [0.6, 0.15],
                    [-0.0, 0.15],
                ],
                tr.rot_rad(angle),
                g,
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
                g,
            );
            graphics::polygon(
                [0.7, 0.7, 1.0, 1.0],
                &[
                    [-0.25, -0.25],
                    [0.6, -0.25],
                    [0.6, 0.25],
                    [-0.25, 0.25],
                ],
                tr.rot_rad(angle),
                g,
            );
        }
        BlockInner::Armor => {
            draw_line_loop(
                [0.7, 0.7, 0.7, 1.0],
                0.05,
                &[
                    [-0.45, -0.45],
                    [0.45, -0.45],
                    [0.45, 0.45],
                    [-0.45, 0.45],
                ],
                tr,
                g,
            );
        }
        BlockInner::Rock => {
            draw_line_loop(
                [0.45, 0.45, 0.45, 1.0],
                0.05,
                &[
                    [-0.45, -0.45],
                    [0.45, -0.45],
                    [0.45, 0.45],
                    [-0.45, 0.45],
                ],
                tr,
                g,
            );
        }
    }
}

pub fn render<G, C, E>(
    context: Context,
    g: &mut G,
    _glyph_cache: &mut C,
    world: &mut World,
    camera: &mut [f64; 2],
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
    let local = world.read::<LocalControl>();

    graphics::clear([0.0, 0.0, 0.1, 1.0], g);

    let tr = context
        .transform
        .trans(
            viewport.width as f64 / 2.0,
            viewport.height as f64 / 2.0,
        )
        .scale(viewport.scale, -viewport.scale);

    // Update camera location
    for (pos, _) in (&pos, &local).join() {
        *camera = pos.pos;
    }
    let tr = tr.trans(-camera[0], -camera[1]);
    let sq_radius = {
        let w = viewport.width as f64;
        let h = viewport.height as f64;
        (w * w + h * h) * 0.25
    };

    // Starry background
    draw_background(&*viewport, *camera, tr, g);

    // Bounds
    draw_line_loop(
        [0.8, 0.8, 0.8, 1.0],
        5.0,
        &[
            [-105.0, -105.0],
            [105.0, -105.0],
            [105.0, 105.0],
            [-105.0, 105.0],
        ],
        tr,
        g,
    );

    // Draw blocks
    for (pos, blocky) in (&pos, &blocky).join() {
        if vec2_square_len(vec2_sub(*camera, pos.pos)) > sq_radius {
            continue;
        }
        let blocks_tr = tr.trans(pos.pos[0], pos.pos[1])
            .rot_rad(pos.rot);
        for &(rel, ref block) in &blocky.blocks {
            draw_block(&block, blocks_tr.trans(rel[0], rel[1]), g);
        }
    }

    // Draw projectiles
    for (pos, proj) in (&pos, &projectile).join() {
        if vec2_square_len(vec2_sub(*camera, pos.pos)) > sq_radius {
            continue;
        }
        let projectile_tr = tr.trans(pos.pos[0], pos.pos[1])
            .rot_rad(pos.rot);
        match proj.kind {
            ProjectileType::Plasma => {
                graphics::line(
                    [0.0, 1.0, 0.0, 1.0],
                    0.1,
                    [-0.8, 0.0, 0.8, 0.0],
                    projectile_tr,
                    g,
                );
            }
            ProjectileType::Rail => {
                graphics::line(
                    [1.0, 1.0, 1.0, 1.0],
                    0.6,
                    [-0.8, 0.0, 0.8, 0.0],
                    projectile_tr,
                    g,
                );
            }
        }
    }

    for (pos, particle) in (&pos, &particles).join() {
        if vec2_square_len(vec2_sub(*camera, pos.pos)) > sq_radius {
            continue;
        }
        let part_tr = tr.trans(pos.pos[0], pos.pos[1])
            .rot_rad(pos.rot);
        match particle.which {
            ParticleType::Spark => {
                let alpha = (particle.lifetime as f32) / 0.2;
                graphics::rectangle(
                    [1.0, 1.0, 1.0, alpha],
                    graphics::rectangle::centered([0.0, 0.0, 0.05, 0.05]),
                    part_tr,
                    g,
                );
            }
            ParticleType::Exhaust => graphics::rectangle(
                [
                    1.0,
                    1.0,
                    1.0,
                    (particle.lifetime as f32).min(0.5),
                ],
                graphics::rectangle::centered([0.0, 0.0, 0.3, 0.3]),
                part_tr,
                g,
            ),
            ParticleType::Explosion => {
                let alpha = (particle.lifetime as f32 * 1.6).min(0.8);
                graphics::rectangle(
                    [1.0, particle.lifetime as f32 / 0.6, 0.0, alpha],
                    graphics::rectangle::centered([0.0, 0.0, 1.2, 1.2]),
                    part_tr,
                    g,
                );
            }
            ParticleType::LaserHit => {
                let alpha = (particle.lifetime as f32 * 4.0).min(0.6);
                let size = (0.2 - particle.lifetime) * 15.0;
                graphics::ellipse(
                    [0.0, 1.0, 0.0, alpha],
                    graphics::rectangle::centered([0.0, 0.0, size, size]),
                    part_tr,
                    g,
                );
            }
        }
    }
}
