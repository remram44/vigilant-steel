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

use gfx_core::Device;
use graphics::Transformed;
use piston::window::WindowSettings;
use piston_window::{OpenGL, PistonWindow};
use piston::input::*;
use sdl2_window::Sdl2Window;
use specs::{Component, DispatcherBuilder, System, World,
            ReadStorage, WriteStorage, Join,
            Fetch, VecStorage};
use vecmath::*;

type Window = PistonWindow<Sdl2Window>;

// Position component, for entities that are in the world
#[derive(Debug)]
struct Position([f64; 2]);

impl Component for Position {
    type Storage = VecStorage<Self>;
}

// Velocity component, for entities that move
#[derive(Debug)]
struct Velocity([f64; 2]);

impl Component for Velocity {
    type Storage = VecStorage<Self>;
}

// Delta resource, stores the simulation step
struct DeltaTime(f64);

// Input resource, stores the keyboard state
struct Input {
    ship: [f64; 2],
}

impl Input {
    fn new() -> Input {
        Input { ship: [0.0, 0.0] }
    }
}

// Input system, sets velocities from keyboard state
struct SysInput;

impl<'a> System<'a> for SysInput {
    type SystemData = (Fetch<'a, Input>,
                       WriteStorage<'a, Velocity>);

    fn run(&mut self, (input, mut vel): Self::SystemData) {
        for vel in (&mut vel).join() {
            vel.0 = input.ship;
        }
    }
}

// Simulation system, updates positions from velocities
struct SysSimu;

impl<'a> System<'a> for SysSimu {
    type SystemData = (Fetch<'a, DeltaTime>,
                       WriteStorage<'a, Position>,
                       ReadStorage<'a, Velocity>);

    fn run(&mut self, (dt, mut pos, vel): Self::SystemData) {
        let dt = dt.0;
        for (pos, vel) in (&mut pos, &vel).join() {
            pos.0 = vec2_add(pos.0, vec2_scale(vel.0, 200.0 * dt));
        }
    }
}

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

    world.create_entity()
        .with(Position([0.0, 0.0]))
        .with(Velocity([0.0, 0.0]))
        .build();

    world.add_resource(DeltaTime(0.0));
    world.add_resource(Input::new());

    let mut dispatcher = DispatcherBuilder::new()
        .add(SysInput, "input", &[])
        .add(SysSimu, "simu", &[])
        .build();

    while let Some(event) = window.next() {
        // Keyboard input
        if let Some(Button::Keyboard(key)) = event.press_args() {
            let mut input = world.write_resource::<Input>();
            match key {
                Key::Escape => break,
                Key::A => input.ship[0] = -1.0,
                Key::D => input.ship[0] =  1.0,
                Key::S => input.ship[1] = -1.0,
                Key::W => input.ship[1] =  1.0,
                _ => {}
            }
        } else if let Some(Button::Keyboard(key)) = event.release_args() {
            let mut input = world.write_resource::<Input>();
            match key {
                Key::A | Key::D => input.ship[0] = 0.0,
                Key::S | Key::W => input.ship[1] = 0.0,
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
        }

        // Draw
        if event.render_args().is_some() {
            let pos = world.read::<Position>();
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

                for pos in pos.join() {
                    let pos = pos.0;
                    graphics::rectangle(
                        [1.0, 0.0, 0.0, 1.0],
                        graphics::rectangle::centered([0.0, 0.0, 10.0, 10.0]),
                        tr.trans(pos[0], pos[1]),
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
