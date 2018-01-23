use specs::{Component, System,
            ReadStorage, WriteStorage, Join,
            Fetch, VecStorage};
use vecmath::*;

use input::Input;
use physics::{DeltaTime, Position, Velocity, LocalControl};

// A ship
pub struct Ship {
    thrust: [f64; 2],
    fire: bool,
    pub color: [f32; 3],
}

impl Ship {
    pub fn new(color: [f32; 3]) -> Ship {
        Ship {
            thrust: [0.0, 0.0],
            fire: false,
            color: color,
        }
    }
}

impl Component for Ship {
    type Storage = VecStorage<Self>;
}

// Ship physics and keyboard control
pub struct SysShip;

impl<'a> System<'a> for SysShip {
    type SystemData = (Fetch<'a, DeltaTime>,
                       Fetch<'a, Input>,
                       ReadStorage<'a, Position>,
                       WriteStorage<'a, Velocity>,
                       WriteStorage<'a, Ship>,
                       ReadStorage<'a, LocalControl>);

    fn run(
        &mut self, (dt, input, pos, mut vel, mut ship, local): Self::SystemData
    ) {
        let dt = dt.0;

        // Control ship thrusters from input
        for (mut ship, _) in (&mut ship, &local).join() {
            ship.thrust[0] = -input.movement[0];
            if input.movement[1] >= 0.0 {
                ship.thrust[1] = input.movement[1];
            }
            ship.fire = input.fire;
        }

        // Apply thrust
        for (pos, mut vel, mut ship) in (&pos, &mut vel, &mut ship).join() {
            // Update orientation
            vel.rot = ship.thrust[0] * 5.0;
            // Update velocity
            let (s, c) = pos.rot.sin_cos();
            let thrust = [c, s];
            vel.vel = vec2_add(vel.vel,
                               vec2_scale(thrust, ship.thrust[1] * 0.5 * dt));

            // Apply friction
            vel.vel = vec2_add(vel.vel,
                               vec2_scale(vel.vel,
                                          -0.8 * dt * vec2_len(vel.vel)));
        }
    }
}
