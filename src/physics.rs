use specs::{Component, System,
            ReadStorage, WriteStorage, Join,
            Fetch, NullStorage, VecStorage};
use vecmath::*;

use input::Input;

// Position component, for entities that are in the world
#[derive(Debug)]
pub struct Position(pub [f64; 2]);

impl Component for Position {
    type Storage = VecStorage<Self>;
}

// Velocity component, for entities that move
#[derive(Debug)]
pub struct Velocity(pub [f64; 2]);

impl Component for Velocity {
    type Storage = VecStorage<Self>;
}

// This object is controlled by the local player
#[derive(Default)]
pub struct LocalControl;

impl Component for LocalControl {
    type Storage = NullStorage<Self>;
}

// A ship
pub struct Ship {
    thrust: [f64; 2],
    fire: bool,
    pub orientation: f64,
    pub color: [f32; 3],
}

impl Ship {
    pub fn new(color: [f32; 3]) -> Ship {
        Ship {
            thrust: [0.0, 0.0],
            fire: false,
            orientation: 0.0,
            color: color,
        }
    }
}

impl Component for Ship {
    type Storage = VecStorage<Self>;
}

// Delta resource, stores the simulation step
pub struct DeltaTime(pub f64);

// Input system, control ship from keyboard state
pub struct SysShipInput;

impl<'a> System<'a> for SysShipInput {
    type SystemData = (Fetch<'a, DeltaTime>,
                       Fetch<'a, Input>,
                       WriteStorage<'a, Ship>,
                       WriteStorage<'a, Velocity>,
                       ReadStorage<'a, LocalControl>);

    fn run(&mut self, (dt, input, mut ship, mut vel, local): Self::SystemData) {
        let dt = dt.0;
        for (mut ship, mut vel, _) in (&mut ship, &mut vel, &local).join() {
            // Set ship status
            ship.thrust[0] = -input.movement[0];
            if input.movement[1] >= 0.0 {
                ship.thrust[1] = input.movement[1];
            }
            ship.fire = input.fire;

            // Update orientation
            ship.orientation += ship.thrust[0] * 5.0 * dt;
            // Update velocity
            let thrust = [ship.orientation.cos(), ship.orientation.sin()];
            vel.0 = vec2_add(vel.0,
                             vec2_scale(thrust, ship.thrust[1] * 0.5 * dt));
        }
    }
}

// Simulation system, updates positions from velocities
pub struct SysSimu;

impl<'a> System<'a> for SysSimu {
    type SystemData = (Fetch<'a, DeltaTime>,
                       WriteStorage<'a, Position>,
                       ReadStorage<'a, Velocity>);

    fn run(&mut self, (dt, mut pos, vel): Self::SystemData) {
        let dt = dt.0;
        for (pos, vel) in (&mut pos, &vel).join() {
            pos.0 = vec2_add(pos.0, vec2_scale(vel.0, 200.0 * dt));
        }
    }
}
