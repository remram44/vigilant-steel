//! Entrypoint and eventloop for server.

extern crate env_logger;
extern crate game;
#[macro_use]
extern crate log;

use game::Game;
use std::time::{Duration, SystemTime};

fn to_secs(dt: Duration) -> f64 {
    dt.as_secs() as f64 + dt.subsec_nanos() as f64 * 0.000_000_001
}

/// Entrypoint for server.
fn main() {
    env_logger::init().unwrap();
    info!("Starting up");

    let mut game = Game::new();

    let mut previous = SystemTime::now();

    loop {
        let now = SystemTime::now();
        let dt = now.duration_since(previous);
        previous = now;

        match dt {
            Ok(dt) => {
                let dt = to_secs(dt);
                if dt < 0.5 {
                    game.update(dt)
                } else {
                    warn!("Clock jumped forward by {} seconds!", dt);
                }
            }
            Err(e) => warn!(
                "Clock jumped backward by {} seconds!",
                to_secs(e.duration())
            ),
        }
    }
}