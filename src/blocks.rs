//! The block definitions, as well as the `Blocky` component.
//!
//! `Blocky` is an important component. A lot of entities are made of blocks,
//! like ships, remnants of ships, drifting modules, and asteroids.
//!
//! A ship is a `Blocky` entity with a cockpit.
//!
//! This module contains the code to update `Blocky` objects, computing mass,
//! center, inertia, removing blocks, and splitting the entity in multiple new
//! entities. However their is currently no blocky system, as that
//! functionality is factored in `SysShip` right now.
// TODO: Refactor some blocky behavior out of SysShip, into a blocky system?

use specs::{Component, Entities, Fetch, LazyUpdate, VecStorage};
use tree::Tree;
use vecmath::*;

/// Active component of the block.
#[derive(Debug, Clone)]
pub enum BlockInner {
    /// This is what allows a ship to be controlled. Ships can't be operated
    /// without this.
    Cockpit,
    /// Allows a ship to move. A ship needs multiple of this to be able to
    /// move and rotate.
    Thruster { angle: f64 },
    /// This shoots explosive energy projectiles.
    PlasmaGun { angle: f64, cooldown: f64 },
    /// This shoots heavy projectiles.
    RailGun { angle: f64, cooldown: f64 },
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
            BlockInner::PlasmaGun {
                ref mut cooldown, ..
            } => {
                if *cooldown > 0.0 {
                    *cooldown -= dt;
                }
            }
            _ => {}
        }
    }

    /// The mass of this block. Must be constant, queried on structure
    /// changes.
    pub fn mass(&self) -> f64 {
        match *self {
            BlockInner::Cockpit => 1.0,
            BlockInner::Thruster { .. } => 0.8,
            BlockInner::PlasmaGun { .. } => 0.2,
            BlockInner::RailGun { .. } => 0.8,
            BlockInner::Armor => 0.6,
            BlockInner::Rock => 0.6,
        }
    }

    /// The starting health of this block.
    pub fn max_health(&self) -> f64 {
        match *self {
            BlockInner::Cockpit => 1.0,
            BlockInner::Thruster { .. } => 0.6,
            BlockInner::PlasmaGun { .. } => 0.4,
            BlockInner::RailGun { .. } => 0.4,
            BlockInner::Armor => 0.4,
            BlockInner::Rock => 0.3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Block {
    /// Health of this blocks, starting at `inner.max_health()`.
    pub health: f64,
    /// The state and behavior of this block, depending on its concrete
    /// type.
    pub inner: BlockInner,
}

impl Block {
    /// Creates a block of a given type with correct starting health.
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
    pub radius: f64,
    pub mass: f64,
    pub inertia: f64,
}

impl Blocky {
    pub fn new(blocks: Vec<([f64; 2], Block)>) -> (Blocky, [f64; 2]) {
        let mut blocky = Blocky {
            blocks: blocks,
            tree: Tree(vec![]),
            radius: 0.0,
            mass: 0.0,
            inertia: 0.0,
        };
        let center = blocky.compute_stats();
        (blocky, center)
    }

    fn compute_stats(&mut self) -> [f64; 2] {
        let mut center = [0.0, 0.0];
        self.mass = 0.0;
        for &(ref loc, ref block) in &self.blocks {
            center = vec2_scale(
                vec2_add(
                    vec2_scale(center, self.mass),
                    vec2_scale(*loc, block.inner.mass()),
                ),
                1.0 / (self.mass + block.inner.mass()),
            );
            self.mass += block.inner.mass();
        }
        self.inertia = 0.0;
        for &mut (ref mut loc, ref block) in self.blocks.iter_mut() {
            *loc = vec2_sub(*loc, center);
            self.inertia += (0.5 + vec2_square_len(*loc)) * block.inner.mass();
        }

        self.tree = Tree::new_(&self.blocks);
        self.radius = 0.0;
        if !self.blocks.is_empty() {
            self.radius = self.tree.0[0]
                .bounds
                .corners()
                .iter()
                .map(|&c| vec2_square_len(c))
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap()
                .sqrt();
        }

        center
    }

    /// Called when some blocks are added or reach 0 health.
    ///
    /// Removes dead blocks, split the entity in multiple `Blocky` objects if
    /// disjoint, recompute mass/center/inertia.
    ///
    /// Returns a triple of dead blocks, new center of mass, and broken off
    /// pieces.
    pub fn maintain(
        &mut self,
    ) -> (Vec<([f64; 2], Block)>, [f64; 2], Vec<(Blocky, [f64; 2])>) {
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

        if self.blocks.is_empty() {
            return (dead_blocks, [0.0, 0.0], Vec::new());
        }

        // Compute adjacency of blocks
        let mut blocks =
            (0..self.blocks.len()).into_iter().collect::<Vec<usize>>();
        for (mut i, &(loc, _)) in self.blocks.iter().enumerate() {
            for v in &[[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]] {
                let pos = vec2_add(loc, *v);
                if let Some(j) = self.tree.find(pos) {
                    let a = blocks[i];
                    let b = blocks[j];
                    let (min, max) = (a.min(b), a.max(b));
                    for e in &mut blocks {
                        if *e == max {
                            *e = min;
                        }
                    }
                    i = min;
                }
            }
        }

        // Find broken off blocks
        let mut pieces: Vec<Vec<([f64; 2], Block)>> =
            Vec::with_capacity(self.blocks.len() - 1);
        for _ in 0..self.blocks.len() - 1 {
            pieces.push(Vec::new());
        }
        let mut removed = 0;
        for (block, &group) in blocks.iter().enumerate() {
            if group != 0 {
                let group = group - 1;
                let b = self.blocks.remove(block - removed);
                pieces[group].push(b);
                removed += 1;
            }
        }

        // Recompute mass, center, inertia
        let center = self.compute_stats();

        // Make Blocky components for the broken off pieces
        let pieces = pieces
            .into_iter()
            .filter(|v| !Vec::is_empty(v))
            .map(Blocky::new)
            .collect::<Vec<_>>();

        (dead_blocks, center, pieces)
    }
}

impl Component for Blocky {
    type Storage = VecStorage<Self>;
}
