//! Entrypoint and eventloop for server.

extern crate env_logger;
extern crate game;
#[macro_use]
extern crate log;

use game::Game;
use std::time::{Duration, SystemTime};

const TIME_STEP: f64 = 0.080;

fn to_secs(dt: Duration) -> f64 {
    dt.as_secs() as f64 + dt.subsec_nanos() as f64 * 0.000_000_001
}

/// Entrypoint for server.
fn main() {
    env_logger::init().unwrap();
    info!("Starting up");

    let mut game = Game::new();

    let mut previous = SystemTime::now();
    let mut timer = 0.0;

    loop {
        let now = SystemTime::now();

        match now.duration_since(previous) {
            Ok(dt) => {
                let dt = to_secs(dt);
                if dt > 0.5 {
                    warn!("Clock jumped forward by {} seconds!", dt);
                    timer = 5.0 * TIME_STEP;
                } else {
                    timer += dt;
                }
                while timer > TIME_STEP {
                    game.update(TIME_STEP);
                    timer -= TIME_STEP;
                }
            }
            Err(e) => {
                warn!(
                    "Clock jumped backward by {} seconds!",
                    to_secs(e.duration())
                );
                timer = 0.0;
            }
        }

        previous = now;
    }
}
