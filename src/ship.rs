//! Ships and projectiles.

use input::{Input, Press};
use physics::{Collided, Collision, DeltaTime, LocalControl, Position,
              Velocity};
use specs::{Component, Entities, Entity, Fetch, Join, LazyUpdate,
            NullStorage, ReadStorage, System, VecStorage, WriteStorage};
use vecmath::*;

/// A ship.
///
/// A ship has thrusters allowing it to rotate and move forward, and can fire
/// projectiles.
pub struct Ship {
    thrust: [f64; 2],
    fire: bool,
    reload: f64,
    pub color: [f32; 3],
    pub health: i32,
}

impl Ship {
    pub fn new(color: [f32; 3]) -> Ship {
        Ship {
            thrust: [0.0, 0.0],
            fire: false,
            reload: 0.0,
            color: color,
            health: 8,
        }
    }

    pub fn create(entities: &Entities, lazy: &Fetch<LazyUpdate>) -> Entity {
        let entity = entities.create();
        lazy.insert(
            entity,
            Position {
                pos: [0.0, 0.0],
                rot: 0.0,
            },
        );
        lazy.insert(
            entity,
            Velocity {
                vel: [0.0, 0.0],
                rot: 0.0,
            },
        );
        lazy.insert(
            entity,
            Collision {
                bounding_box: [10.0, 8.0],
            },
        );
        lazy.insert(entity, Ship::new([1.0, 0.0, 0.0]));
        entity
    }
}

impl Component for Ship {
    type Storage = VecStorage<Self>;
}

/// Ship physics and keyboard control.
///
/// This computes the ship's state from the keyboard if `LocalControl`, updates
/// the ship's speed, and fires projectiles.
pub struct SysShip;

impl<'a> System<'a> for SysShip {
    type SystemData = (
        Fetch<'a, DeltaTime>,
        Fetch<'a, LazyUpdate>,
        Fetch<'a, Input>,
        Entities<'a>,
        ReadStorage<'a, Position>,
        WriteStorage<'a, Velocity>,
        ReadStorage<'a, Collided>,
        WriteStorage<'a, Ship>,
        ReadStorage<'a, LocalControl>,
    );

    fn run(
        &mut self,
        (
            dt,
            lazy,
            input,
            entities,
            pos,
            mut vel,
            collided,
            mut ship,
            local,
        ): Self::SystemData,
){
        let dt = dt.0;

        // Handle collisions
        for (col, mut ship, _) in (&collided, &mut ship, &local).join() {
            for _ in &col.entities {
                ship.health -= 1;
            }
        }

        // Prevent leaving the screen
        for (pos, vel, mut ship) in (&pos, &mut vel, &mut ship).join() {
            if pos.pos[0] < -400.0 || pos.pos[0] > 400.0 || pos.pos[1] < -300.0
                || pos.pos[1] > 300.0
            {
                ship.health -= 1;
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
            match input.fire {
                Press::UP => ship.fire = false,
                Press::PRESSED => ship.fire = true,
                _ => {}
            }
        }

        for (ent, pos, mut vel, mut ship) in
            (&*entities, &pos, &mut vel, &mut ship).join()
        {
            // Death
            if ship.health <= 0 {
                entities.delete(ent).unwrap();
                continue;
            }

            // Apply thrust
            // Update orientation
            vel.rot = ship.thrust[0] * 5.0;
            // Update velocity
            let (s, c) = pos.rot.sin_cos();
            let thrust = [c, s];
            vel.vel = vec2_add(
                vel.vel,
                vec2_scale(thrust, ship.thrust[1] * 0.5 * dt),
            );

            // Apply friction
            vel.vel = vec2_add(
                vel.vel,
                vec2_scale(vel.vel, -0.8 * dt * vec2_len(vel.vel)),
            );

            // Fire
            if ship.fire && ship.reload <= 0.0 {
                ship.reload = 0.7;

                Projectile::create(
                    &entities,
                    &lazy,
                    vec2_add(pos.pos, [17.0 * c, 17.0 * s]),
                    pos.rot,
                );
            } else if ship.reload > 0.0 {
                ship.reload -= dt;
            }
        }
    }
}

/// A projectile.
///
/// This is a simple segment that goes in a straight line, and gets removed
/// when it hits something or exits the screen.
#[derive(Default)]
pub struct Projectile;

impl Projectile {
    pub fn create(
        entities: &Entities,
        lazy: &Fetch<LazyUpdate>,
        pos: [f64; 2],
        rot: f64,
    ) -> Entity {
        let entity = entities.create();
        let (s, c) = rot.sin_cos();
        lazy.insert(entity, Position { pos: pos, rot: rot });
        lazy.insert(
            entity,
            Velocity {
                vel: [3.0 * c, 3.0 * s],
                rot: 0.0,
            },
        );
        lazy.insert(
            entity,
            Collision {
                bounding_box: [8.0, 1.0],
            },
        );
        lazy.insert(entity, Projectile);
        entity
    }
}

impl Component for Projectile {
    type Storage = NullStorage<Self>;
}

/// Deletes projectiles when they fall off.
pub struct SysProjectile;

impl<'a> System<'a> for SysProjectile {
    type SystemData = (
        Entities<'a>,
        ReadStorage<'a, Collided>,
        ReadStorage<'a, Position>,
        ReadStorage<'a, Projectile>,
    );

    fn run(
        &mut self,
        (entities, collided, pos, projectile): Self::SystemData,
    ) {
        // Remove projectiles gone from the screen or hit
        for (entity, pos, _) in (&*entities, &pos, &projectile).join() {
            if let Some(_) = collided.get(entity) {
                entities.delete(entity).unwrap();
                continue;
            }

            let pos = pos.pos;
            if pos[0] < -500.0 || pos[0] > 500.0 || pos[1] < -500.0
                || pos[1] > 500.0
            {
                entities.delete(entity).unwrap();
            }
        }
    }
}
