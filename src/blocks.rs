use specs::{Component, Entities, Fetch, LazyUpdate, VecStorage};
use tree::Tree;
use vecmath::*;

/// Active component of the block.
#[derive(Debug)]
pub enum BlockInner {
    /// This is what allows a ship to be controlled. Ships can't be operated
    /// without this.
    Cockpit,
    /// Allows a ship to move. A ship needs multiple of this to be able to
    /// move and rotate.
    Thruster { angle: f64 },
    /// This shoots projectiles.
    Gun { angle: f64, cooldown: f64 },
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
            BlockInner::Gun {
                ref mut cooldown, ..
            } => {
                if *cooldown > 0.0 {
                    *cooldown -= dt;
                }
            }
            _ => {}
        }
    }

    pub fn mass(&self) -> f64 {
        match *self {
            BlockInner::Cockpit => 1.0,
            BlockInner::Thruster { .. } => 0.8,
            BlockInner::Gun { .. } => 0.2,
            BlockInner::Armor => 0.6,
            BlockInner::Rock => 0.6,
        }
    }

    pub fn max_health(&self) -> f64 {
        match *self {
            BlockInner::Cockpit => 1.0,
            BlockInner::Thruster { .. } => 0.6,
            BlockInner::Gun { .. } => 0.4,
            BlockInner::Armor => 0.4,
            BlockInner::Rock => 0.3,
        }
    }
}

#[derive(Debug)]
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

// Entity is made of blocks
pub struct Blocky {
    pub blocks: Vec<([f64; 2], Block)>,
    pub tree: Tree,
    pub mass: f64,
    pub inertia: f64,
}

impl Blocky {
    pub fn new(mut blocks: Vec<([f64; 2], Block)>) -> (Blocky, [f64; 2]) {
        let (mass, inertia, center, tree) = Self::compute_stats(&mut blocks);

        let blocky = Blocky {
            blocks: blocks,
            tree: tree,
            mass: mass,
            inertia: inertia,
        };
        (blocky, center)
    }

    fn compute_stats(
        blocks: &mut Vec<([f64; 2], Block)>,
    ) -> (f64, f64, [f64; 2], Tree) {
        let mut mass = 0.0;
        let mut center = [0.0, 0.0];
        for &(ref loc, ref block) in &*blocks {
            center = vec2_scale(
                vec2_add(
                    vec2_scale(center, mass),
                    vec2_scale(*loc, block.inner.mass()),
                ),
                1.0 / (mass + block.inner.mass()),
            );
            mass += block.inner.mass();
        }
        let mut inertia = 0.0;
        for &mut (ref mut loc, ref block) in blocks.iter_mut() {
            *loc = vec2_sub(*loc, center);
            inertia += vec2_square_len(*loc) * block.inner.mass();
        }

        let tree = Tree::new_(blocks);

        (mass, inertia, center, tree)
    }

    pub fn pop_dead_blocks(&mut self) -> Vec<([f64; 2], Block)> {
        // Drop blocks with no health
        let mut i = 0;
        let mut dead_blocks = Vec::new();
        while i != self.blocks.len() {
            if self.blocks[i].1.health < 0.0 {
                dead_blocks.push(self.blocks.remove(i));
            } else {
                i += 1;
            }
        }

        // Update tree
        self.tree = Tree::new_(&self.blocks);

        dead_blocks
    }
}

impl Component for Blocky {
    type Storage = VecStorage<Self>;
}
