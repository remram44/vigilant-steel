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
    let mut target_x = 20.0;
    let mut target_y = 20.0;

    // Colliding square
    let mut square_x = 200.0;
    let mut square_y = 150.0;
    let mut square_o = 0.0;

    // Square movement vector
    let mut square_move_x = 0.0;
    let mut square_move_y = 10.0;

    // Key status
    let mut key_target_x = 0.0;
    let mut key_target_y = 0.0;
    let mut key_square_x = 0.0;
    let mut key_square_y = 0.0;
    let mut key_square_o = 0.0;
    let mut key_square_move_x = 0.0;
    let mut key_square_move_y = 0.0;
    let mut shift = false;

    // Collision indicator
    let mut col = None;

    while let Some(event) = window.next() {
        // Keyboard input
        if let Some(Button::Keyboard(key)) = event.press_args() {
            match key {
                Key::Escape => break,
                Key::LShift => shift = true,
                Key::A => key_target_x = -1.0,
                Key::D => key_target_x =  1.0,
                Key::S => key_target_y = -1.0,
                Key::W => key_target_y =  1.0,
                Key::J => if shift { key_square_move_x = -1.0 }
                          else { key_square_x = -1.0 },
                Key::L => if shift { key_square_move_x =  1.0 }
                          else { key_square_x =  1.0 },
                Key::K => if shift { key_square_move_y = -1.0 }
                          else { key_square_y = -1.0 },
                Key::I => if shift { key_square_move_y =  1.0 }
                          else { key_square_y =  1.0 },
                Key::O => key_square_o = -1.0,
                Key::U => key_square_o =  1.0,
                _ => {}
            }
        } else if let Some(Button::Keyboard(key)) = event.release_args() {
            match key {
                Key::LShift => shift = false,
                Key::A | Key::D => key_target_x = 0.0,
                Key::W | Key::S => key_target_y = 0.0,
                Key::J | Key::L => { key_square_move_x = 0.0;
                                     key_square_x = 0.0 },
                Key::I | Key::K => { key_square_move_y = 0.0;
                                     key_square_y = 0.0 },
                Key::O | Key::U => key_square_o = 0.0,
                _ => {}
            }
        }

        // Update
        if let Some(u) = event.update_args() {
            let dt = u.dt;
            target_x += key_target_x * 200.0 * dt;
            target_y += key_target_y * 200.0 * dt;
            square_move_x += key_square_move_x * 200.0 * dt;
            square_move_y += key_square_move_y * 200.0 * dt;
            square_x += key_square_x * 200.0 * dt;
            square_y += key_square_y * 200.0 * dt;
            square_o += key_square_o * dt;

            // Find a collision
            let tr = graphics::math::identity()
                .trans(square_x, square_y)
                .rot_rad(square_o)
                .zoom(1./50.0);;
            let tr_target = row_mat2x3_transform_pos2(tr, [target_x, target_y]);
            let tr_move = row_mat2x3_transform_vec2(tr, [square_move_x, square_move_y]);
            let t = square_point_collision(tr_target, tr_move);
            col = t.map(|t| {
                (square_x + square_move_x * t,
                 square_y + square_move_y * t)
            });
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
                    tr.trans(target_x, target_y),
                    g);

                graphics::rectangle(
                    [0.8, 0.8, 1.0, 1.0],
                    graphics::rectangle::centered([0.0, 0.0, 50.0, 50.0]),
                    tr.trans(square_x, square_y).rot_rad(square_o),
                    g);

                graphics::Line::new(
                    [0.0, 0.0, 1.0, 1.0],
                    5.0
                ).draw_arrow(
                    [0.0, 0.0, square_move_x, square_move_y],
                    20.0,
                    &Default::default(),
                    tr.trans(square_x, square_y),
                    g);

                if let Some((x, y)) = col {
                    for l in &[
                        [-25.0f64, -25., -25., 25.], [-25., 25., 25., 25.],
                        [25., 25., 25., -25.], [25., -25., -25., -25.]]
                    {
                        graphics::line(
                            [0.0, 1.0, 0.0, 1.0],
                            1.0,
                            l.clone(),
                            tr.trans(x, y),
                            g);
                    }
                }
            });
            window.device.cleanup();
        }
    }
}

fn square_point_collision<T>(target: Vector2<T>, square_move: Vector2<T>) -> Option<f64> {
    // TODO
    Some(1.0)
}
