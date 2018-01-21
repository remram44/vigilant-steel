extern crate env_logger;
extern crate gfx_core;
extern crate graphics;
#[macro_use] extern crate log;
extern crate rand;
extern crate piston;
extern crate piston_window;
extern crate sdl2_window;
extern crate specs;
extern crate vecmath;

mod input;
mod physics;

use gfx_core::Device;
use graphics::Transformed;
use piston::window::WindowSettings;
use piston_window::{OpenGL, PistonWindow};
use piston::input::*;
use sdl2_window::Sdl2Window;
use specs::{DispatcherBuilder, World, Join};
use vecmath::*;

use input::Input;
use physics::{DeltaTime, Position, Velocity, LocalControl, Ship,
              SysShipInput, SysSimu};

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

    let mut world = World::new();
    world.register::<Position>();
    world.register::<Velocity>();
    world.register::<LocalControl>();
    world.register::<Ship>();

    world.create_entity()
        .with(Position([0.0, 0.0]))
        .with(Velocity([0.0, 0.0]))
        .with(LocalControl)
        .with(Ship::new([1.0, 0.0, 0.0]))
        .build();

    world.create_entity()
        .with(Position([100.0, 50.0]))
        .with(Velocity([0.0, 0.0]))
        .with(Ship::new([0.0, 0.0, 1.0]))
        .build();

    world.add_resource(DeltaTime(0.0));
    world.add_resource(Input::new());

    let mut dispatcher = DispatcherBuilder::new()
        .add(SysShipInput, "input", &[])
        .add(SysSimu, "simu", &[])
        .build();

    while let Some(event) = window.next() {
        // Keyboard input
        if let Some(Button::Keyboard(key)) = event.press_args() {
            let mut input = world.write_resource::<Input>();
            match key {
                Key::Escape => break,
                Key::A => input.movement[0] = -1.0,
                Key::D => input.movement[0] =  1.0,
                Key::S => input.movement[1] = -1.0,
                Key::W => input.movement[1] =  1.0,
                Key::Space => input.fire = true,
                _ => {}
            }
        } else if let Some(Button::Keyboard(key)) = event.release_args() {
            let mut input = world.write_resource::<Input>();
            match key {
                Key::A | Key::D => input.movement[0] = 0.0,
                Key::S | Key::W => input.movement[1] = 0.0,
                _ => {}
            }
        }

        // Update
        if let Some(u) = event.update_args() {
            {
                let mut dt = world.write_resource::<DeltaTime>();
                *dt = DeltaTime(u.dt);
            }
            dispatcher.dispatch(&mut world.res);

            let mut input = world.write_resource::<Input>();
            input.fire = false;
        }

        // Draw
        if event.render_args().is_some() {
            let pos = world.read::<Position>();
            let ship = world.read::<Ship>();
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

                for (pos, ship) in (&pos, &ship).join() {
                    let pos = pos.0;
                    let ship_tr = tr
                        .trans(pos[0], pos[1])
                        .rot_rad(ship.orientation);
                    let mut color = [0.0, 0.0, 0.0, 1.0];
                    color[0..3].copy_from_slice(&ship.color);
                    graphics::line(
                        color,
                        1.0,
                        [-10.0, 8.0, -10.0, -8.0],
                        ship_tr,
                        g);
                    graphics::line(
                        color,
                        1.0,
                        [-10.0, 8.0, 10.0, 0.0],
                        ship_tr,
                        g);
                    graphics::line(
                        color,
                        1.0,
                        [-10.0, -8.0, 10.0, 0.0],
                        ship_tr,
                        g);
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
