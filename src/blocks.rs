use specs::{Entities, Fetch, LazyUpdate};

/// Active component of the block.
pub enum BlockInner {
    /// This is what allows a ship to be controlled. Ships can't be operated
    /// without this.
    Cockpit,
    /// Allows a ship to move. A ship needs multiple of this to be able to
    /// move and rotate.
    Thruster(f64),
    /// This shoots projectiles. Guns have a reload timer, which is the
    /// second attribute.
    Gun(f64, f64),
    /// An armor block does nothing, it is only there to take damage (and
    /// weigh you down).
    Armor,
    /// Rock is similar to armor, but weaker.
    Rock,
}

impl BlockInner {
    /// Updates this block each frame.
    pub fn update(
        &mut self,
        dt: f64,
        _entities: &Entities,
        _lazy: &Fetch<LazyUpdate>,
    ) {
        match *self {
            BlockInner::Gun(_, ref mut reload) => {
                if *reload > 0.0 {
                    *reload -= dt;
                }
            }
            _ => {}
        }
    }

    pub fn mass(&self) -> f64 {
        match *self {
            BlockInner::Cockpit => 1.0,
            BlockInner::Thruster(_) => 0.8,
            BlockInner::Gun(_, _) => 0.2,
            BlockInner::Armor => 0.6,
            BlockInner::Rock => 2.0,
        }
    }

    pub fn max_health(&self) -> f64 {
        match *self {
            BlockInner::Cockpit => 1.0,
            BlockInner::Thruster(_) => 0.6,
            BlockInner::Gun(_, _) => 0.4,
            BlockInner::Armor => 0.4,
            BlockInner::Rock => 0.3,
        }
    }
}

pub struct Block {
    pub health: f64,
    pub inner: BlockInner,
}

impl Block {
    pub fn new(inner: BlockInner) -> Block {
        Block {
            health: inner.max_health(),
            inner: inner,
        }
    }
}
