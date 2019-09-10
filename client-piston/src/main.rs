//! Entrypoint and eventloop for SDL client.

extern crate color_logger;
extern crate game;
extern crate graphics;
#[macro_use]
extern crate log;
extern crate opengl_graphics;
extern crate piston;
extern crate rand;
extern crate sdl2_window;
extern crate specs;
extern crate vecmath;

mod render;

use game::Game;
use game::input::{Input, Press};
use game::utils::FpsCounter;
use opengl_graphics::{GlGraphics, GlyphCache, OpenGL, TextureSettings};
use piston::input::*;
use piston::window::WindowSettings;
use render::Viewport;
use sdl2_window::Sdl2Window;
use specs::WorldExt;
use std::collections::HashMap;

const MAX_TIME_STEP: f64 = 0.040;

/// The application context, passed through the `event_loop` module.
struct App {
    gl: GlGraphics,
    glyph_cache: GlyphCache<'static>,
    fps_counter: FpsCounter,
    game: Game,
    camera: [f64; 2],
    touches: HashMap<i64, [f64; 2]>,
    touch_mode: bool,
}

#[cfg(not(target_os = "emscripten"))]
const OPENGL: OpenGL = OpenGL::V3_2;
#[cfg(target_os = "emscripten")]
const OPENGL: OpenGL = OpenGL::V2_1;

/// Entrypoint. Sets up SDL and the event loop.
fn main() {
    color_logger::init(log::Level::Info).unwrap();
    info!("Starting up");

    let width = 800;
    let height = 600;

    // Create an SDL2 window.
    let window: Sdl2Window =
        WindowSettings::new("vigilant-engine", [width, height])
            .opengl(OPENGL)
            .srgb(false)
            .resizable(true)
            .build()
            .expect("Couldn't create an OpenGL window");
    info!("Window created");

    let gl = GlGraphics::new(OPENGL);
    info!("OpenGL initialized");

    let glyph_cache = GlyphCache::new(
        "assets/FiraSans-Regular.ttf",
        (),
        TextureSettings::new(),
    ).unwrap();

    let game = {
        let mut args = std::env::args();
        args.next().unwrap();
        match args.next() {
            Some(a) => {
                if args.next().is_some() {
                    eprintln!("Too many arguments!");
                    std::process::exit(1);
                }
                #[cfg(not(feature = "network"))]
                panic!("Want to connect but networking is not compiled in");
                #[cfg(feature = "network")]
                {
                    let addr = match a.parse() {
                        Ok(a) => a,
                        Err(_) => {
                            eprintln!("Invalid address");
                            std::process::exit(1);
                        }
                    };
                    Game::new_client(addr)
                }
            }
            None => Game::new_standalone(),
        }
    };

    let mut app = App {
        gl: gl,
        glyph_cache: glyph_cache,
        fps_counter: FpsCounter::new(),
        game: game,
        camera: [0.0, 0.0],
        touches: HashMap::new(),
        touch_mode: false,
    };
    app.game
        .world
        .insert(Viewport::new([width, height]));

    // Use the event_loop module to handle SDL/Emscripten differences
    event_loop::run(window, handle_event, app);
}

/// Handles a Piston event fed from the `event_loop` module.
fn handle_event(
    _window: &mut Sdl2Window,
    event: Event,
    app: &mut App,
) -> bool {
    // Window resize
    if let Some(newsize) = event.resize_args() {
        let mut viewport = app.game.world.write_resource::<Viewport>();
        *viewport = Viewport::new(newsize);
    }

    // Keyboard input and buttons
    if let Some(button) = event.button_args() {
        if app.touches.is_empty() {
            app.touch_mode = false;

            let mut input = app.game.world.write_resource::<Input>();
            if let Button::Mouse(m) = button.button {
                let pressed = match button.state {
                    ButtonState::Press => Press::PRESSED,
                    ButtonState::Release => Press::UP,
                };
                match m {
                    MouseButton::Left => input.buttons[0] = pressed,
                    MouseButton::Right => input.buttons[1] = pressed,
                    MouseButton::Middle => input.buttons[2] = pressed,
                    _ => {}
                }
            } else if let Some(scancode) = button.scancode {
                if button.state == ButtonState::Press {
                    match scancode {
                        22 => input.movement[0] = -1.0, // S
                        26 => input.movement[0] = 1.0,  // W
                        20 => input.movement[1] = 1.0,  // Q
                        8 => input.movement[1] = -1.0,  // E
                        4 => input.rotation = 1.0,      // A
                        7 => input.rotation = -1.0,     // D
                        44 => input.fire = Press::PRESSED,
                        _ => {}
                    }
                } else {
                    match scancode {
                        22 | 26 => input.movement[0] = 0.0,
                        8 | 20 => input.movement[1] = 0.0,
                        4 | 7 => input.rotation = 0.0,
                        44 => input.fire = Press::UP,
                        _ => {}
                    }
                }
            }
        }
    }

    // Mouse
    if let Some(cursor) = event.mouse_cursor_args() {
        let mut input = app.game.world.write_resource::<Input>();
        let viewport = app.game.world.read_resource::<Viewport>();
        input.mouse = [
            (cursor[0] - 0.5 * viewport.width as f64) / viewport.scale,
            (0.5 * viewport.height as f64 - cursor[1]) / viewport.scale,
        ];
    }

    // Touch
    if let Some(touch) = event.touch_args() {
        let mut input = app.game.world.write_resource::<Input>();
        if !app.touch_mode {
            *input = Default::default();
            app.touch_mode = true;
        }
        match touch.touch {
            Touch::Start | Touch::Move => {
                app.touches.insert(touch.id, touch.position());
            }
            Touch::End | Touch::Cancel => {
                app.touches.remove(&touch.id);
            }
        }
    }

    // Update
    if let Some(u) = event.update_args() {
        let mut dt = u.dt;
        if dt > 0.5 {
            warn!("Clock jumped forward by {} seconds!", dt);
            dt = 5.0 * MAX_TIME_STEP;
        }

        if app.touch_mode {
            let mut input = app.game.world.write_resource::<Input>();
            input.movement = [0.0, 0.0];
            input.rotation = 0.0;
            let mut fire = false;
            for (_, touch) in &app.touches {
                if touch[1] < 0.3 {
                    input.movement[0] = 1.0;
                } else if touch[1] > 0.7 {
                    fire = true;
                } else if touch[0] < 0.3 {
                    input.rotation = 1.0;
                } else if touch[0] > 0.7 {
                    input.rotation = -1.0;
                }
            }
            if fire && input.fire == Press::UP {
                input.fire = Press::PRESSED;
            } else if !fire {
                input.fire = Press::UP;
            }
        }

        while dt > 0.0 {
            if dt > MAX_TIME_STEP {
                app.game.update(MAX_TIME_STEP);
                dt -= MAX_TIME_STEP;
            } else {
                app.game.update(dt);
                break;
            }
        }
    }

    // Draw
    if let Some(r) = event.render_args() {
        {
            let world = &mut app.game.world;
            let glyph_cache = &mut app.glyph_cache;
            let mut camera = &mut app.camera;
            app.gl.draw(r.viewport(), |c, g| {
                render::render(c, g, glyph_cache, world, camera);
            });
        }
        if app.fps_counter.rendered() {
            info!("fps = {}", app.fps_counter.value());
            app.game.profile();
        }
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

    use piston::input::{AfterRenderArgs, Event, Loop, RenderArgs, UpdateArgs};
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

    pub fn run<T>(
        window: Sdl2Window,
        handler: fn(&mut Sdl2Window, Event, &mut T) -> bool,
        arg: T,
    ) {
        unsafe {
            let mut events = Box::new(EventLoop {
                last_updated: emscripten_sys::emscripten_get_now() as f64,
                window: window,
                handler: handler,
                arg: arg,
            });
            let events_ptr = &mut *events as *mut EventLoop<_> as *mut c_void;
            emscripten_sys::emscripten_set_main_loop_arg(
                Some(main_loop_c::<T>),
                events_ptr,
                0,
                1,
            );
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
