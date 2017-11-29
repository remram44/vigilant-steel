extern crate env_logger;
extern crate gfx_core;
extern crate graphics;
#[macro_use] extern crate log;
extern crate rand;
extern crate piston;
extern crate piston_window;
extern crate sdl2_window;

use gfx_core::Device;
use piston::window::WindowSettings;
use piston_window::{Context, G2d, OpenGL, PistonWindow};
use piston::input::*;
use sdl2_window::Sdl2Window;

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

    while let Some(event) = window.next() {
        // Key
        let key_event = if let Some(Button::Keyboard(key)) = event.press_args() {
            info!("Pressed key '{:?}'", key);
            Some((key, true))
        } else if let Some(Button::Keyboard(key)) = event.release_args() {
            info!("Released key '{:?}", key);
            Some((key, false))
        } else {
            None
        };
        if let Some((key, pressed)) = key_event {
            if key == Key::Escape {
                break;
            }
        }

        // Call update method
        if let Some(u) = event.update_args() {
        }

        // Call draw method
        if let Some(r) = event.render_args() {
            window.draw_2d(&event, |c, g| {
                let (width, height) = if let Some(v) = c.viewport {
                    (v.rect[2], v.rect[3])
                } else {
                    warn!("Got Context with no attached Viewport");
                    return;
                };

                graphics::clear([0.0, 0.0, 0.5, 1.0], g);

                graphics::rectangle([1.0, 1.0, 1.0, 1.0],
                                    graphics::rectangle::centered([100.0, 100.0, 50.0, 50.0]),
                                                                  c.transform, g);
            });
            window.device.cleanup();
        }
    }
}
