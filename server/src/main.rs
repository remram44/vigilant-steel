//! Entrypoint and eventloop for server.

use game::Game;
#[cfg(feature = "udp")]
use game::net::udp::UdpServer;
#[cfg(feature = "websocket")]
use game::net::websocket::WebsocketServer;
use log::{info, warn};
use std::thread::sleep;
use std::time::{Duration, SystemTime};

const TIME_STEP: f32 = 0.050; // 20 ticks per second
const MAX_SKIPPED_STEPS: u32 = 5;

fn to_secs(dt: Duration) -> f32 {
    dt.as_secs() as f32 + dt.subsec_nanos() as f32 * 0.000_000_001
}

/// Entrypoint for server.
fn main() {
    color_logger::init(log::Level::Info).unwrap();
    info!("Starting up");

    #[cfg(all(feature = "udp", feature = "websocket"))]
    compile_error!("Multiple transports enabled");
    #[cfg(feature = "udp")]
    let mut game = Game::new_server(UdpServer::new(34244));
    #[cfg(feature = "websocket")]
    let mut game = Game::new_server(WebsocketServer::new(8080));
    #[cfg(not(any(feature = "udp", feature = "websocket")))]
    compile_error!("No transport enabled");

    let mut previous = SystemTime::now();
    let mut timer = 0.0;

    loop {
        let now = SystemTime::now();

        match now.duration_since(previous) {
            Ok(dt) => {
                let dt = to_secs(dt);
                if dt > MAX_SKIPPED_STEPS as f32 * TIME_STEP {
                    warn!("Clock jumped forward by {} seconds!", dt);
                    timer = MAX_SKIPPED_STEPS as f32 * TIME_STEP;
                } else {
                    timer += dt;
                }
                while timer >= TIME_STEP {
                    game.update(TIME_STEP);
                    timer -= TIME_STEP;
                }

                if TIME_STEP - timer > 0.001 {
                    sleep(Duration::new(
                        0,
                        ((TIME_STEP - timer) * 1_000_000_000.0) as u32,
                    ));
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
