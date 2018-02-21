//! Rendering code, using Piston.

use game::asteroid::Asteroid;
use game::physics::{LocalControl, Position};
use game::ship::{Projectile, Ship};
use graphics::{self, Context, Graphics, Transformed};
use graphics::character::CharacterCache;
use graphics::math::Matrix2d;
use specs::{Join, World};
use std::fmt::Debug;

const MAX_RATIO: f64 = 1.6;
const VIEWPORT_SIZE: f64 = 800.0;

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

pub fn render<G, C, E>(
    context: Context,
    gl: &mut G,
    glyph_cache: &mut C,
    world: &mut World,
) where
    G: graphics::Graphics,
    E: Debug,
    C: CharacterCache<Texture = <G as Graphics>::Texture, Error = E> + Sized,
{
    let viewport = world.read_resource::<Viewport>();
    let pos = world.read::<Position>();
    let ship = world.read::<Ship>();
    let projectile = world.read::<Projectile>();
    let asteroid = world.read::<Asteroid>();

    graphics::clear([0.0, 0.0, 0.1, 1.0], gl);

    let tr = context
        .transform
        .trans(viewport.width as f64 / 2.0, viewport.height as f64 / 2.0)
        .scale(viewport.scale, -viewport.scale);

    for (pos, ship) in (&pos, &ship).join() {
        let ship_tr = tr.trans(pos.pos[0], pos.pos[1]).rot_rad(pos.rot);
        let mut color = [
            ship.color[0] as f32 / 256.0,
            ship.color[1] as f32 / 256.0,
            ship.color[2] as f32 / 256.0,
            1.0,
        ];
        draw_line_loop(
            color,
            1.0,
            &[[-10.0, -8.0], [10.0, 0.0], [-10.0, 8.0]],
            ship_tr,
            gl,
        );
    }

    for (pos, _) in (&pos, &asteroid).join() {
        let asteroid_tr = tr.trans(pos.pos[0], pos.pos[1]).rot_rad(pos.rot);
        draw_line_loop(
            [1.0, 1.0, 1.0, 1.0],
            1.0,
            &[
                [-38.0, -26.0],
                [0.0, -46.0],
                [38.0, -26.0],
                [38.0, 26.0],
                [0.0, 46.0],
                [-38.0, 26.0],
                [-38.0, -26.0],
                [38.0, -26.0],
                [-38.0, 26.0],
                [38.0, 26.0],
            ],
            asteroid_tr,
            gl,
        );
    }

    for (pos, _) in (&pos, &projectile).join() {
        let projectile_tr = tr.trans(pos.pos[0], pos.pos[1]).rot_rad(pos.rot);
        graphics::line(
            [0.0, 1.0, 0.0, 1.0],
            2.0,
            [-8.0, 0.0, 8.0, 0.0],
            projectile_tr,
            gl,
        );
    }

    let local = world.read::<LocalControl>();
    let ship = world.read::<Ship>();
    if let Some((_, ship)) = (&local, &ship).join().next() {
        let health = ship.health;
        let poly = &[
            [0.0, 0.0],
            [0.0, 1.0],
            [0.707, 0.707],
            [1.0, 0.0],
            [0.707, -0.707],
            [0.0, -1.0],
            [-0.707, -0.707],
            [-1.0, 0.0],
            [-0.707, 0.707],
            [0.0, 1.0],
        ];
        graphics::polygon(
            [0.0, 0.0, 1.0, 1.0],
            &poly[0..(2 + health.max(0) as usize)],
            context.transform.trans(50.0, 50.0).scale(50.0, 50.0),
            gl,
        );
        graphics::text(
            [1.0, 0.0, 0.0, 1.0],
            32,
            &format!("{}", health),
            glyph_cache,
            context.transform.trans(45.0, 55.0).scale(0.8, 0.8),
            gl,
        ).unwrap();
        graphics::text(
            [0.3, 0.3, 1.0, 1.0],
            20,
            "Health",
            glyph_cache,
            context.transform.trans(27.0, 115.0).scale(0.8, 0.8),
            gl,
        ).unwrap();
    }
}
