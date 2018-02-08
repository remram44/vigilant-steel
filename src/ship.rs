use specs::{Entity, Component, System, World,
            Entities, ReadStorage, WriteStorage, Join,
            Fetch, FetchMut, VecStorage};
use vecmath::*;

use input::Input;
use super::Health;
use physics::{DeltaTime, Position, Velocity, Collision, Collided,
              LocalControl};

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

    pub fn create(
        entities: Entities,
        mut pos: WriteStorage<Position>, mut vel: WriteStorage<Velocity>,
        mut collision: WriteStorage<Collision>, mut ship: WriteStorage<Ship>,
    ) -> Entity
    {
        let entity = entities.create();
        pos.insert(entity, Position { pos: [0.0, 0.0], rot: 0.0 });
        vel.insert(entity, Velocity { vel: [0.0, 0.0], rot: 0.0 });
        collision.insert(entity, Collision { bounding_box: [10.0, 8.0] });
        ship.insert(entity, Ship::new([1.0, 0.0, 0.0]));
        entity
    }

    pub fn create_in_world(world: &mut World) -> Entity {
        Ship::create(
            world.entities(),
            world.write::<Position>(),
            world.write::<Velocity>(),
            world.write::<Collision>(),
            world.write::<Ship>())
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
                       FetchMut<'a, Health>,
                       ReadStorage<'a, Position>,
                       WriteStorage<'a, Velocity>,
                       ReadStorage<'a, Collided>,
                       WriteStorage<'a, Ship>,
                       ReadStorage<'a, LocalControl>);

    fn run(
        &mut self,
        (dt, input, mut health,
         pos, mut vel, collided, mut ship, local): Self::SystemData
    ) {
        let dt = dt.0;

        // Handle collisions
        for _ in (&collided, &ship, &local).join() {
            health.0 -= 1;
        }

        // Prevent leaving the screen
        for (pos, vel, _) in (&pos, &mut vel, &ship).join() {
            if pos.pos[0] < -400.0 || pos.pos[0] > 400.0 ||
                pos.pos[1] < -300.0 || pos.pos[1] > 300.0
            {
                health.0 -= 1;
                vel.vel = vec2_sub([0.0, 0.0], pos.pos);
                vel.vel = vec2_scale(vel.vel, 3.0 * vec2_inv_len(vel.vel));
            }
        }

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
