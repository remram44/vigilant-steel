//! Entrypoint, eventloop, and rendering.

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
mod render;
mod ship;
mod utils;

use opengl_graphics::{GlGraphics, OpenGL};
use piston::window::WindowSettings;
use piston::input::*;
use sdl2_window::Sdl2Window;
use specs::{Dispatcher, DispatcherBuilder, World, LazyUpdate};

use asteroid::{Asteroid, SysAsteroid};
use input::{Input, Press};
use physics::{DeltaTime, Position, Velocity, Collision, Collided,
              LocalControl, Health,
              SysCollision, SysSimu};
use render::Viewport;
use ship::{Ship, SysShip, Projectile, SysProjectile};

/// The application context, passed through the `event_loop` module.
struct App {
    gl: GlGraphics,
    world: World,
    dispatcher: Dispatcher<'static, 'static>,
    /// Indicates that the game has been lost, input should no longer be
    /// accepted.
    game_over: bool,
}

#[cfg(not(target_os = "emscripten"))]
const OPENGL: OpenGL = OpenGL::V3_2;
#[cfg(target_os = "emscripten")]
const OPENGL: OpenGL = OpenGL::V2_1;

/// Entrypoint. Sets up SDL and the event loop.
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
        .exit_on_esc(true)
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

    let ship = Ship::create(&world.entities(),
                            &world.read_resource::<LazyUpdate>());
    world.write::<LocalControl>().insert(ship, LocalControl);

    world.add_resource(DeltaTime(0.0));
    world.add_resource(Input::new());
    world.add_resource(Health(8));
    world.add_resource(Viewport::new([width, height]));

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
        game_over: false,
    };

    // Use the event_loop module to handle SDL/Emscripten differences
    event_loop::run(window, handle_event, app);
}

/// Handles a Piston event fed from the `event_loop` module.
fn handle_event(_window: &mut Sdl2Window, event: Event, app: &mut App) -> bool {
    // Window resize
    if let Some(newsize) = event.resize_args() {
        let mut viewport = app.world.write_resource::<Viewport>();
        *viewport = Viewport::new(newsize);
    }

    // Keyboard input
    if !app.game_over {
        if let Some(button) = event.button_args() {
            let mut input = app.world.write_resource::<Input>();
            if let Some(scancode) = button.scancode {
                if button.state == ButtonState::Press {
                    match scancode {
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
            app.game_over = true;
            let mut input = app.world.write_resource::<Input>();
            *input = Input::new();
        }

        let mut input = app.world.write_resource::<Input>();
        if input.fire == Press::PRESSED {
            input.fire = Press::KEPT;
        }
    }

    // Draw
    if let Some(r) = event.render_args() {
        let game_over = app.game_over;
        let world = &mut app.world;
        app.gl.draw(r.viewport(), |c, g| {
            render::render(c, g, world, game_over);
        });
    }
    true
}

/// Event loop, factored out for SDL and Emscripten support.
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

/// Event loop, factored out for SDL and Emscripten support.
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
