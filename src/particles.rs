use Role;
use physics::{DeltaTime, Position, Velocity};
use rand::{self, Rng};
use specs::{Component, Entities, Fetch, FetchMut, Join, LazyUpdate, System,
            VecStorage, WriteStorage};
use std::cell::RefCell;
use std::f64::consts::PI;
use std::sync::{Arc, Mutex};

/// Types of particles, that determine lifetime and render model.
#[derive(Clone, Copy, Debug)]
pub enum ParticleType {
    /// Metallic sparks, from things crashing into each other.
    Spark,
    /// Smoke out of a thruster.
    Exhaust,
    /// Destroyed parts blow up.
    Explosion,
}

impl ParticleType {
    /// How long the particle lives for, in seconds.
    fn lifetime(&self) -> f64 {
        match *self {
            ParticleType::Spark => 0.2,
            ParticleType::Exhaust => 1.0,
            ParticleType::Explosion => 0.6,
        }
    }
}

/// This entity is a particle.
///
/// Those are only created on graphical clients, don't get replicated, and
/// disappear after a moment.
pub struct Particle {
    pub lifetime: f64,
    pub which: ParticleType,
}

impl Component for Particle {
    type Storage = VecStorage<Self>;
}

pub struct ParticleEffects {
    // FIXME: More efficient collection
    pending_effects: Arc<Mutex<RefCell<Vec<(ParticleType, [f64; 2])>>>>,
}

impl ParticleEffects {
    pub fn new() -> ParticleEffects {
        ParticleEffects {
            pending_effects: Arc::new(Mutex::new(RefCell::new(Vec::new()))),
        }
    }

    pub fn delay(&self, which: ParticleType, pos: [f64; 2]) {
        self.pending_effects
            .lock()
            .unwrap()
            .get_mut()
            .push((which, pos));
    }

    pub fn pending(&self) -> Vec<(ParticleType, [f64; 2])> {
        let guard = self.pending_effects.lock().unwrap();
        let v: &Vec<_> = &guard.borrow();
        v.clone()
    }
}

pub struct SysParticles;

impl<'a> System<'a> for SysParticles {
    type SystemData = (
        Fetch<'a, DeltaTime>,
        Fetch<'a, Role>,
        Fetch<'a, LazyUpdate>,
        FetchMut<'a, ParticleEffects>,
        Entities<'a>,
        WriteStorage<'a, Particle>,
    );

    fn run(
        &mut self,
        (dt, role, lazy, effects, entities, mut particles): Self::SystemData,
    ) {
        let mut guard = effects.pending_effects.lock().unwrap();
        let effects: &mut Vec<_> = guard.get_mut();

        if !role.graphical() {
            effects.clear();
            return;
        }

        let dt = dt.0;

        let mut rng = rand::thread_rng();
        let mut c_rng = rng.clone();
        let mut create = |which: ParticleType, pos, vel| {
            let ent = entities.create();
            lazy.insert(ent, pos);
            lazy.insert(ent, vel);
            lazy.insert(
                ent,
                Particle {
                    lifetime: which.lifetime() * c_rng.gen_range(0.8, 1.5),
                    which: which,
                },
            );
        };
        for (which, pos) in effects.drain(..) {
            match which {
                ParticleType::Spark => for _ in 0..2 {
                    create(
                        which,
                        Position {
                            pos: [
                                pos[0] + 0.6 * rng.gen_range(-0.2, 0.2),
                                pos[1] + 0.6 * rng.gen_range(-0.2, 0.2),
                            ],
                            rot: 0.0,
                        },
                        Velocity {
                            vel: [
                                rng.gen_range(-0.05, 0.05),
                                rng.gen_range(-0.05, 0.05),
                            ],
                            rot: 0.0,
                        },
                    );
                },
                ParticleType::Explosion => for _ in 0..10 {
                    create(
                        which,
                        Position {
                            pos: [
                                pos[0] + 0.6 * rng.gen_range(-4.0, 4.0),
                                pos[1] + 0.6 * rng.gen_range(-4.0, 4.0),
                            ],
                            rot: rng.gen_range(0.0, 2.0 * PI),
                        },
                        Velocity {
                            vel: [
                                rng.gen_range(-0.02, 0.02),
                                rng.gen_range(-0.02, 0.02),
                            ],
                            rot: rng.gen_range(-5.0, 5.0),
                        },
                    );
                },
                _ => warn!("Unexpected pending particle effect {:?}", which),
            }
        }

        for (ent, mut particle) in (&*entities, &mut particles).join() {
            particle.lifetime -= dt;
            if particle.lifetime < 0.0 {
                entities.delete(ent).unwrap();
            }
        }
    }
}
