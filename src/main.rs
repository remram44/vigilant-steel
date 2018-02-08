extern crate env_logger;
extern crate graphics;
#[macro_use] extern crate log;
extern crate opengl_graphics;
extern crate piston;
extern crate rand;
extern crate sdl2_window;
extern crate specs;
extern crate vecmath;

mod asteroid;
mod input;
mod physics;
mod ship;
mod utils;

use graphics::Transformed;
use graphics::math::Matrix2d;
use opengl_graphics::{GlGraphics, OpenGL};
use piston::window::WindowSettings;
use piston::input::*;
use sdl2_window::Sdl2Window;
use specs::{Dispatcher, DispatcherBuilder, World, Join};
use vecmath::*;

use asteroid::{Asteroid, SysAsteroid};
use input::{Input, Press};
use physics::{DeltaTime, Position, Velocity, Collision, Collided,
              LocalControl,
              SysCollision, SysSimu};
use ship::{Ship, SysShip, Projectile, SysProjectile};

pub struct Health(i32);

struct App {
    gl: GlGraphics,
    world: World,
    dispatcher: Dispatcher<'static, 'static>,
}

#[cfg(not(target_os = "emscripten"))]
const OPENGL: OpenGL = OpenGL::V3_2;
#[cfg(target_os = "emscripten")]
const OPENGL: OpenGL = OpenGL::V2_1;

fn main() {
    env_logger::init().unwrap();
    info!("Starting up");

    let width = 800;
    let height = 600;

    // Create an SDL2 window.
    let window: Sdl2Window = WindowSettings::new(
            "vigilant-engine",
            [width, height],
        )
        .opengl(OPENGL)
        .srgb(false)
        .build()
        .expect("Couldn't create an OpenGL window");
    info!("Window created");

    let gl = GlGraphics::new(OPENGL);
    info!("OpenGL initialized");

    let mut world = World::new();
    world.register::<Position>();
    world.register::<Velocity>();
    world.register::<Collision>();
    world.register::<Collided>();
    world.register::<LocalControl>();
    world.register::<Ship>();
    world.register::<Projectile>();
    world.register::<Asteroid>();

    let ship = Ship::create_in_world(&mut world);
    world.write::<LocalControl>().insert(ship, LocalControl);

    world.add_resource(DeltaTime(0.0));
    world.add_resource(Input::new());
    world.add_resource(Health(8));

    let dispatcher = DispatcherBuilder::new()
        .add(SysSimu, "simu", &[])
        .add(SysCollision, "collision", &[])
        .add(SysShip, "ship", &[])
        .add(SysProjectile, "projectile", &[])
        .add(SysAsteroid::new(), "asteroid", &[])
        .build();

    let app = App {
        gl: gl,
        world: world,
        dispatcher: dispatcher,
    };

    event_loop::run(window, handle_event, app);
}

fn draw_line_loop<G>(color: [f32; 4], radius: graphics::types::Radius,
                     points: &[Vector2<f64>], tr: Matrix2d, g: &mut G)
    where G: graphics::Graphics
{
    let mut points = points.iter();
    let first = match points.next() {
        Some(p) => p,
        None => return,
    };
    let mut previous = first;
    for point in points {
        graphics::line(color, radius,
                       [previous[0], previous[1], point[0], point[1]],
                       tr, g);
        previous = point;
    }
    graphics::line(color, radius,
                   [previous[0], previous[1], first[0], first[1]],
                   tr, g);
}

fn handle_event(_window: &mut Sdl2Window, event: Event, app: &mut App) -> bool {
    // Keyboard input
    if let Some(button) = event.button_args() {
        let mut input = app.world.write_resource::<Input>();
        if let Some(scancode) = button.scancode {
            if button.state == ButtonState::Press {
                match scancode {
                    41 => return false,
                    4 => input.movement[0] = -1.0,
                    7 => input.movement[0] =  1.0,
                    22 => input.movement[1] = -1.0,
                    26 => input.movement[1] =  1.0,
                    44 => input.fire = Press::PRESSED,
                    _ => {}
                }
            } else {
                match scancode {
                    4 | 7 => input.movement[0] = 0.0,
                    22 | 26 => input.movement[1] = 0.0,
                    44 => input.fire = Press::UP,
                    _ => {}
                }
            }
        }
    }

    // Update
    if let Some(u) = event.update_args() {
        {
            let mut dt = app.world.write_resource::<DeltaTime>();
            *dt = DeltaTime(u.dt);
        }
        app.dispatcher.dispatch(&mut app.world.res);
        app.world.maintain();

        if app.world.read_resource::<Health>().0 <= 0 {
            return false;
        }

        let mut input = app.world.write_resource::<Input>();
        if input.fire == Press::PRESSED {
            input.fire = Press::KEPT;
        }
    }

    // Draw
    if let Some(r) = event.render_args() {
        let world = &mut app.world;
        let pos = world.read::<Position>();
        let ship = world.read::<Ship>();
        let projectile = world.read::<Projectile>();
        let asteroid = world.read::<Asteroid>();
        app.gl.draw(r.viewport(), |c, g| {
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
                let ship_tr = tr
                    .trans(pos.pos[0], pos.pos[1])
                    .rot_rad(pos.rot);
                let mut color = [0.0, 0.0, 0.0, 1.0];
                color[0..3].copy_from_slice(&ship.color);
                draw_line_loop(
                    color, 1.0,
                    &[
                        [-10.0, -8.0],
                        [10.0, 0.0],
                        [-10.0, 8.0],
                    ],
                    ship_tr, g);
            }

            for (pos, _) in (&pos, &asteroid).join() {
                let asteroid_tr = tr
                    .trans(pos.pos[0], pos.pos[1])
                    .rot_rad(pos.rot);
                draw_line_loop(
                    [1.0, 1.0, 1.0, 1.0], 1.0,
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
                    asteroid_tr, g);
            }

            for (pos, _) in (&pos, &projectile).join() {
                let projectile_tr = tr
                    .trans(pos.pos[0], pos.pos[1])
                    .rot_rad(pos.rot);
                graphics::line(
                    [0.0, 1.0, 0.0, 1.0], 2.0,
                    [-8.0, 0.0, 8.0, 0.0],
                    projectile_tr, g);
            }

            let health = world.read_resource::<Health>().0;
            let poly = &[
                [0.0, 0.0], [0.0, 1.0],
                [0.707, 0.707], [1.0, 0.0], [0.707, -0.707], [0.0, -1.0],
                [-0.707, -0.707], [-1.0, 0.0], [-0.707, 0.707], [0.0, 1.0],
            ];
            graphics::polygon(
                [0.0, 0.0, 1.0, 1.0],
                &poly[0..(2 + health.max(0) as usize)],
                tr.trans(-350.0, 250.0).scale(50.0, 50.0), g);
        });
    }
    true
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

#[cfg(not(target_os = "emscripten"))]
mod event_loop {
    use piston::event_loop::{EventSettings, Events};
    use piston::input::Event;
    use sdl2_window::Sdl2Window;

    pub fn run<T>(
        mut window: Sdl2Window,
        handler: fn(&mut Sdl2Window, Event, &mut T) -> bool,
        mut arg: T,
    ) {
        let mut events = Events::new(EventSettings::new());
        while let Some(e) = events.next(&mut window) {
            if !handler(&mut window, e, &mut arg) {
                break;
            }
        }
    }
}

#[cfg(target_os = "emscripten")]
mod event_loop {
    extern crate emscripten_sys;

    use piston::input::{Event, Loop, AfterRenderArgs, RenderArgs, UpdateArgs};
    use piston::window::Window;
    use sdl2_window::Sdl2Window;
    use std::mem;
    use std::os::raw::c_void;

    struct EventLoop<T> {
        last_updated: f64,
        window: Sdl2Window,
        handler: fn(&mut Sdl2Window, Event, &mut T) -> bool,
        arg: T,
    }

    pub fn run<T>(window: Sdl2Window,
                  handler: fn(&mut Sdl2Window, Event, &mut T) -> bool,
                  arg: T) {
        unsafe {
            let mut events = Box::new(EventLoop {
                last_updated: emscripten_sys::emscripten_get_now() as f64,
                window: window,
                handler: handler,
                arg: arg,
            });
            let events_ptr = &mut *events as *mut EventLoop<_> as *mut c_void;
            emscripten_sys::emscripten_set_main_loop_arg(Some(main_loop_c::<T>), events_ptr, 0, 1);
            mem::forget(events);
        }
    }

    extern "C" fn main_loop_c<T>(arg: *mut c_void) {
        unsafe {
            let events: &mut EventLoop<T> = mem::transmute(arg);
            let window = &mut events.window;
            let handler = events.handler;
            let arg = &mut events.arg;
            window.swap_buffers();

            let e = Event::Loop(Loop::AfterRender(AfterRenderArgs));
            handler(window, e, arg);

            while let Some(e) = window.poll_event() {
                handler(window, Event::Input(e), arg);
            }

            if window.should_close() {
                emscripten_sys::emscripten_cancel_main_loop();
                return;
            }

            let now = emscripten_sys::emscripten_get_now() as f64;
            let dt = (now - events.last_updated) / 1000.0;
            events.last_updated = now;

            let e = Event::Loop(Loop::Update(UpdateArgs { dt: dt }));
            handler(window, e, arg);

            let size = window.size();
            let draw_size = window.draw_size();
            let e = Event::Loop(Loop::Render(RenderArgs {
                ext_dt: dt,
                width: size.width,
                height: size.height,
                draw_width: draw_size.width,
                draw_height: draw_size.height,
            }));
            handler(window, e, arg);
        }
    }
}
