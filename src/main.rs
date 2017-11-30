extern crate env_logger;
extern crate gfx_core;
extern crate graphics;
#[macro_use] extern crate log;
extern crate rand;
extern crate piston;
extern crate piston_window;
extern crate sdl2_window;
extern crate vecmath;

use gfx_core::Device;
use graphics::Transformed;
use piston::window::WindowSettings;
use piston_window::{OpenGL, PistonWindow};
use piston::input::*;
use sdl2_window::Sdl2Window;
use vecmath::*;

type Window = PistonWindow<Sdl2Window>;

fn main() {
    env_logger::init().unwrap();
    info!("Starting up");

    let width = 800;
    let height = 600;

    // Change this to OpenGL::V2_1 if not working.
    let opengl = OpenGL::V3_2;

    // Create an SDL2 window.
    let mut window: Window = WindowSettings::new(
            "vigilant-engine",
            [width, height],
        )
        .opengl(opengl)
        .build()
        .unwrap();
    info!("Window created");

    // Point to collide
    let mut target = [20.0, 20.0];

    // Colliding square
    let mut square = [200.0, 150.0];
    let mut square_o = 0.0;

    // Square movement vector
    let mut square_move = [0.0, 10.0];

    // Key status
    let mut key_target = [0.0, 0.0];
    let mut key_square = [0.0, 0.0];
    let mut key_square_o = 0.0;
    let mut key_square_move = [0.0, 0.0];
    let mut shift = false;

    // Collision indicator
    let mut col = None;

    while let Some(event) = window.next() {
        // Keyboard input
        if let Some(Button::Keyboard(key)) = event.press_args() {
            match key {
                Key::Escape => break,
                Key::LShift => shift = true,
                Key::A => key_target[0] = -1.0,
                Key::D => key_target[0] =  1.0,
                Key::S => key_target[1] = -1.0,
                Key::W => key_target[1] =  1.0,
                Key::J => if shift { key_square_move[0] = -1.0 }
                          else { key_square[0] = -1.0 },
                Key::L => if shift { key_square_move[0] =  1.0 }
                          else { key_square[0] =  1.0 },
                Key::K => if shift { key_square_move[1] = -1.0 }
                          else { key_square[1] = -1.0 },
                Key::I => if shift { key_square_move[1] =  1.0 }
                          else { key_square[1] =  1.0 },
                Key::O => key_square_o = -1.0,
                Key::U => key_square_o =  1.0,
                _ => {}
            }
        } else if let Some(Button::Keyboard(key)) = event.release_args() {
            match key {
                Key::LShift => shift = false,
                Key::A | Key::D => key_target[0] = 0.0,
                Key::W | Key::S => key_target[1] = 0.0,
                Key::J | Key::L => { key_square_move[0] = 0.0;
                                     key_square[0] = 0.0 },
                Key::I | Key::K => { key_square_move[1] = 0.0;
                                     key_square[1] = 0.0 },
                Key::O | Key::U => key_square_o = 0.0,
                _ => {}
            }
        }

        // Update
        if let Some(u) = event.update_args() {
            let dt = u.dt;
            target = vec2_add(target, vec2_scale(key_target, 200.0 * dt));
            square_move = vec2_add(square_move, vec2_scale(key_square_move, 200.0 * dt));
            square = vec2_add(square, vec2_scale(key_square, 200.0 * dt));
            square_o += key_square_o * dt;

            // Find a collision
            let tr = graphics::math::identity()
                .rot_rad(-square_o)
                .zoom(1./100.0)
                .trans(-square[0], -square[1]);
            let tr_target = row_mat2x3_transform_pos2(tr, target);
            let tr_move = row_mat2x3_transform_vec2(tr, square_move);
            let t = square_point_collision(tr_move, tr_target);
            col = t.map(|t| vec2_add(square, vec2_scale(square_move, t)));
        }

        // Draw
        if event.render_args().is_some() {
            window.draw_2d(&event, |c, g| {
                let (width, height) = if let Some(v) = c.viewport {
                    (v.rect[2], v.rect[3])
                } else {
                    warn!("Got Context with no attached Viewport");
                    return;
                };

                graphics::clear([0.0, 0.0, 0.1, 1.0], g);

                let tr = c.transform
                    .trans(width as f64 / 2.0, height as f64 / 2.0)
                    .scale(1.0, -1.0);

                graphics::rectangle(
                    [1.0, 0.0, 0.0, 1.0],
                    graphics::rectangle::centered([0.0, 0.0, 10.0, 10.0]),
                    tr.trans(target[0], target[1]),
                    g);

                graphics::rectangle(
                    [0.8, 0.8, 1.0, 1.0],
                    graphics::rectangle::centered([0.0, 0.0, 50.0, 50.0]),
                    tr.trans(square[0], square[1]).rot_rad(square_o),
                    g);

                graphics::Line::new(
                    [0.0, 0.0, 1.0, 1.0],
                    5.0
                ).draw_arrow(
                    [0.0, 0.0, square_move[0], square_move[1]],
                    20.0,
                    &Default::default(),
                    tr.trans(square[0], square[1]),
                    g);

                if let Some(pos) = col {
                    for l in &[
                        [-50.0f64, -50., -50., 50.], [-50., 50., 50., 50.],
                        [50., 50., 50., -50.], [50., -50., -50., -50.]]
                    {
                        graphics::line(
                            [0.0, 1.0, 0.0, 1.0],
                            1.0,
                            l.clone(),
                            tr.trans(pos[0], pos[1]).rot_rad(square_o),
                            g);
                    }
                }
            });
            window.device.cleanup();
        }
    }
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
fn segment_point_collision(seg_a: Vector2<f64>, seg_b: Vector2<f64>,
                           seg_move: Vector2<f64>, target: Vector2<f64>)
    -> Option<f64>
{
    let segdir = vec2_sub(seg_b, seg_a);
    let perdir = [segdir[1], -segdir[0]];
    // Assume segment has length 1, otherwise we'd normalize here

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
    if 0.0 <= line_pos && line_pos <= 1.0 {
        Some(t)
    } else {
        None
    }
}
