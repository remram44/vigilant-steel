use rand::{self, Rng};
use std::f64::consts::PI;

use specs::{Component, System,
            Entities, ReadStorage, WriteStorage, Join,
            Fetch, NullStorage, VecStorage};
use vecmath::*;

use input::Input;

// Position component, for entities that are in the world
#[derive(Debug)]
pub struct Position {
    pub pos: [f64; 2],
    pub rot: f64,
}

impl Component for Position {
    type Storage = VecStorage<Self>;
}

// Velocity component, for entities that move
#[derive(Debug)]
pub struct Velocity {
    pub vel: [f64; 2],
    pub rot: f64,
}

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

// An asteroid
#[derive(Default)]
pub struct Asteroid;

impl Component for Asteroid {
    type Storage = NullStorage<Self>;
}

// Delta resource, stores the simulation step
pub struct DeltaTime(pub f64);

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
            let thrust = [pos.rot.cos(), pos.rot.sin()];
            vel.vel = vec2_add(vel.vel,
                               vec2_scale(thrust, ship.thrust[1] * 0.5 * dt));

            // Apply friction
            vel.vel = vec2_add(vel.vel,
                               vec2_scale(vel.vel,
                                          -0.8 * dt * vec2_len(vel.vel)));
        }
    }
}

// Asteroid spawning and removing
pub struct SysAsteroid {
    spawn_delay: Option<f64>,
}

impl SysAsteroid {
    pub fn new() -> SysAsteroid {
        SysAsteroid { spawn_delay: None }
    }
}

impl<'a> System<'a> for SysAsteroid {
    type SystemData = (Fetch<'a, DeltaTime>,
                       Entities<'a>,
                       WriteStorage<'a, Position>,
                       WriteStorage<'a, Velocity>,
                       WriteStorage<'a, Asteroid>);

    fn run(
        &mut self,
        (dt, entities, mut pos, mut vel, mut asteroid): Self::SystemData
    ) {
        let dt = dt.0;

        // Update orientations
        let mut count = 0;
        for (entity, pos, _) in (&*entities, &pos, &asteroid).join() {
            let pos = pos.pos;
            if pos[0] < -550.0 || pos[0] > 550.0 ||
                pos[1] < -550.0 || pos[1] > 550.0
            {
                warn!("Deleting asteroid");
                entities.delete(entity).unwrap();
            }
            count += 1;
        }

        self.spawn_delay = if let Some(d) = self.spawn_delay.take() {
            if d <= 0.0 {
                warn!("Spawning asteroid now");
                let mut rng = rand::thread_rng();
                let &(xpos, ypos) = rng.choose(&[
                    (-1.0,  0.0), // left
                    ( 1.0,  0.0), // right
                    ( 0.0, -1.0), // bottom
                    ( 0.0,  1.0), // top
                ]).unwrap();
                let entity = entities.create();
                pos.insert(
                    entity,
                    Position {
                        pos: [
                            xpos * 500.0 + ypos * rng.gen_range(-500.0, 500.0),
                            ypos * 500.0 + xpos * rng.gen_range(-500.0, 500.0),
                        ],
                        rot: rng.gen_range(0.0, 2.0 * PI),
                    },
                );
                vel.insert(
                    entity,
                    Velocity {
                        vel: [
                            rng.gen_range(-0.3, 0.3) - xpos * 0.4,
                            rng.gen_range(-0.3, 0.3) - ypos * 0.4,
                        ],
                        rot: rng.gen_range(-2.0, 2.0),
                    },
                );
                asteroid.insert(entity, Asteroid);
                None
            } else {
                Some(d - dt)
            }
        } else if count < 10 {
            warn!("Currently {} asteroids", count);
            let delay = 3.0 - 0.5 * (10 - count) as f64;
            warn!("Spawning asteroid in {} seconds", delay);
            Some(delay)
        } else {
            None
        };
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
            pos.pos = vec2_add(pos.pos, vec2_scale(vel.vel, 200.0 * dt));
            pos.rot += vel.rot * dt;
            pos.rot %= 2.0 * PI;
        }
    }
}
