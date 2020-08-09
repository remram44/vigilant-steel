//! Test client

use log::{error, warn};
use game::Game;
use game::net::udp::UdpClient;
use std::thread::sleep;
use std::time::{Duration, SystemTime};

const MIN_STEP: f32 = 0.020; // 50 ticks per second

fn to_secs(dt: Duration) -> f32 {
    dt.as_secs() as f32 + dt.subsec_nanos() as f32 * 0.000_000_001
}

fn main() {
    color_logger::init(log::Level::Info).unwrap();
    let mut args = std::env::args();
    args.next().unwrap();
    let addr = match args.next() {
        Some(s) => match s.parse() {
            Ok(a) => a,
            Err(err) => {
                error!("Invalid address {:?}: {}", s, err);
                std::process::exit(1);
            }
        }
        None => {
            error!("No address specified");
            std::process::exit(1);
        }
    };
    if let Some(_) = args.next() {
        error!("Too many arguments");
        std::process::exit(1);
    }

    let client = UdpClient::new(addr);
    let mut game = Game::new_client(client);

    let mut previous = SystemTime::now();

    loop {
        let now = SystemTime::now();

        match now.duration_since(previous) {
            Ok(dt) => {
                let dt = to_secs(dt);
                game.update(dt);

                if let Ok(frame) = SystemTime::now().duration_since(now) {
                    let frame = to_secs(frame);
                    if frame < MIN_STEP {
                        sleep(Duration::new(
                            0,
                            ((MIN_STEP - frame) * 1_000_000_000.0) as u32,
                        ));
                    }
                }
            }
            Err(e) => {
                warn!(
                    "Clock jumped backward by {} seconds!",
                    to_secs(e.duration())
                );
            }
        }

        previous = now;
    }
}
