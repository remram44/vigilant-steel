use game::blocks::{BlockInner, Blocky};
use game::guns::{Projectile, ProjectileType};
use game::particles::{Particle, ParticleType};
use game::physics::{LocalControl, Position};
use specs::{Entity, Join};
use specs::world::WorldExt;
use std::collections::hash_map::{Entry, HashMap};
use std::collections::HashSet;
use std::f32::consts::PI;
use std::num::Wrapping;
use vecmath::*;
use wasm_bindgen::prelude::*;

use crate::App;
use primitives::{BufType, VertexArrays, VertexVecs};

#[wasm_bindgen]
extern "C" {
    fn set_camera(pos_x: f32, pos_y: f32, scale_x: f32, scale_y: f32);
    fn del_buffer(id: f64);
    fn draw(
        position_x: f32, position_y: f32,
        angle: f32, scale: f32,
        color: &[f32],
        buffer_id: f64,
    );
}

const MAX_RATIO: f32 = 1.6;
const VIEWPORT_SIZE: f32 = 80.0;

// IDs of common buffers created in init()
const EXTRA_BUFS_BASE: f64 = (1u64 << 40) as f64;

const BUF_BOUNDS: f64 = EXTRA_BUFS_BASE + 0.0;
const BUF_PLASMA: f64 = EXTRA_BUFS_BASE + 1.0;
const BUF_RAIL: f64 = EXTRA_BUFS_BASE + 2.0;

const BUF_SPARK: f64 = EXTRA_BUFS_BASE + 20.0;
const BUF_EXHAUST: f64 = EXTRA_BUFS_BASE + 21.0;
const BUF_EXPLOSION: f64 = EXTRA_BUFS_BASE + 22.0;
const BUF_LASER_HIT: f64 = EXTRA_BUFS_BASE + 23.0;

// IDs for entities' buffers
const BUFFERS_PER_ENTITY:u32 = 2;
fn entity_buffer(entity_id: u32, buffer_id: u32) -> f64 {
    (entity_id * BUFFERS_PER_ENTITY + buffer_id) as f64
}

// "Default color", white (no modulation)
const DEF_COLOR: &[f32] = &[1.0, 1.0, 1.0, 1.0];

/// Global information kept by the render module
#[derive(Default)]
pub struct RenderApp {
    viewport: [u32; 2],
    scale: [f32; 2],
    camera: [f32; 2],
    blocky_buffers: HashMap<u32, (Entity, Wrapping<u32>)>,
}

impl RenderApp {
    /// Update the scale for a new viewport size
    fn set_viewport(&mut self, viewport: [u32; 2]) -> bool {
        if self.viewport == viewport {
            return false;
        }
        self.viewport = viewport;
        let width = viewport[0] as f32;
        let height = viewport[1] as f32;
        let (size_x, size_y) = if width > MAX_RATIO * height {
            (VIEWPORT_SIZE * MAX_RATIO, VIEWPORT_SIZE * MAX_RATIO * height / width)
        } else if width > height {
            (VIEWPORT_SIZE * width / height, VIEWPORT_SIZE)
        } else if height > MAX_RATIO * width {
            (VIEWPORT_SIZE * MAX_RATIO * width / height, VIEWPORT_SIZE * MAX_RATIO)
        } else /* if height > width */ {
            (VIEWPORT_SIZE, VIEWPORT_SIZE * height / width)
        };
        self.scale = [2.0 / size_x, 2.0 / size_y];
        info!(
            "Viewport {}x{}, scale x={} y={}",
            viewport[0], viewport[1],
            self.scale[0], self.scale[1],
        );
        true
    }

    /// Project back the cursor position from screen coordinates to game
    pub fn project_cursor(&self, screen_coords: [f32; 2]) -> [f32; 2] {
        [
            (screen_coords[0] * 2.0 / self.viewport[0] as f32 - 1.0) / self.scale[0],
            (1.0 - screen_coords[1] * 2.0 / self.viewport[1] as f32) / self.scale[1],
        ]
    }
}

/// Initialize the rendering module (create the common buffers)
pub fn init() {
    let mut bounds = VertexVecs::default();
    bounds.hollow_rect(
        [-5.0, -5.0],
        [155.0, 105.0],
        10.0,
        [0.8, 0.8, 0.8, 1.0],
    );
    bounds.store(BUF_BOUNDS, BufType::STATIC);

    let mut plasma = VertexVecs::default();
    plasma.line(
        [-0.8, 0.0], [0.8, 0.0],
        0.1,
        [0.0, 1.0, 0.0, 1.0],
    );
    plasma.store(BUF_PLASMA, BufType::STATIC);

    let mut rail = VertexVecs::default();
    rail.line(
        [-0.8, 0.0], [0.8, 0.0],
        0.6,
        [1.0, 1.0, 1.0, 1.0],
    );
    rail.store(BUF_RAIL, BufType::STATIC);
    let mut spark = VertexVecs::default();
    spark.filled_rect(
        [-0.05, -0.05], [0.05, 0.05],
        [1.0, 1.0, 1.0, 1.0],
    );
    spark.store(BUF_SPARK, BufType::STATIC);
    let mut exhaust = VertexVecs::default();
    exhaust.filled_rect(
        [-0.3, -0.3], [0.3, 0.3],
        [1.0, 1.0, 1.0, 1.0],
    );
    exhaust.store(BUF_EXHAUST, BufType::STATIC);
    let mut explosion = VertexVecs::default();
    explosion.filled_rect(
        [-1.2, -1.2], [1.2, 1.2],
        [1.0, 0.0, 0.0, 1.0],
    );
    explosion.store(BUF_EXPLOSION, BufType::STATIC);
    let mut laser_hit = VertexVecs::default();
    let mut points = Vec::new();
    for i in 0..32 {
        let (s, c) = (i as f32 * 2.0 * PI / 32.0).sin_cos();
        points.push([3.0 * c, 3.0 * s]);
    }
    laser_hit.filled_convex_polygon(
        &points,
        [0.0, 1.0, 0.0, 1.0],
    );
    laser_hit.store(BUF_LASER_HIT, BufType::STATIC);
}

/// Render everything
pub fn render(app: &mut App, viewport: [u32; 2]) {
    let world = &app.game.world;
    let entities = world.entities();
    let pos = world.read_component::<Position>();
    let local = world.read_component::<LocalControl>();
    let blocky = world.read_component::<Blocky>();
    let projectile = world.read_component::<Projectile>();
    let particle = world.read_component::<Particle>();

    // Update camera location
    app.render_app.set_viewport(viewport);
    for (pos, _) in (&pos, &local).join() {
        app.render_app.camera = pos.pos;
    }
    set_camera(
        app.render_app.camera[0], app.render_app.camera[1],
        app.render_app.scale[0], app.render_app.scale[1],
    );
    let sq_radius = vec2_square_len([
        1.0 / app.render_app.scale[0] + 30.0,
        1.0 / app.render_app.scale[1] + 30.0,
    ]);

    // TODO: Background

    // Bounds
    draw(0.0, 0.0, 0.0, 1.0, DEF_COLOR, BUF_BOUNDS);

    // Draw blocks
    let mut blocky_seen: HashSet<u32> = HashSet::new();
    for (ent, pos, blocky) in (&*entities, &pos, &blocky).join() {
        // Check position is within visible area
        if vec2_square_len(vec2_sub(pos.pos, app.render_app.camera)) > sq_radius {
            continue;
        }
        blocky_seen.insert(ent.id());

        // Look into hashmap to decide whether entity has changed
        let entry = app.render_app.blocky_buffers.entry(ent.id());
        let changed;
        match entry {
            Entry::Occupied(entry) => {
                let (e, rev) = entry.into_mut();
                if e != &ent {
                    // Different entity
                    changed = true;
                    *e = ent;
                } else if rev != &blocky.revision {
                    // Revision changed
                    changed = true;
                    *rev = blocky.revision;
                } else {
                    changed = false;
                }
            }
            Entry::Vacant(entry) => {
                changed = true;
                entry.insert((ent, blocky.revision));
            }
        };

        // Generate buffers
        generate_blocky_buffers(ent.id(), blocky, changed);

        // Draw
        draw(
            pos.pos[0], pos.pos[1],
            pos.rot, 1.0,
            DEF_COLOR,
            entity_buffer(ent.id(), 0),
        );
        draw(
            pos.pos[0], pos.pos[1],
            pos.rot, 1.0,
            DEF_COLOR,
            entity_buffer(ent.id(), 1),
        );
    }

    // Remove objects we haven't drawed
    app.render_app.blocky_buffers.retain(|_, (ent, _rev)| {
        if blocky_seen.contains(&ent.id()) {
            return true;
        }
        for i in 0..BUFFERS_PER_ENTITY {
            del_buffer(entity_buffer(ent.id(), i));
        }
        false
    });

    // Draw projectiles
    for (pos, proj) in (&pos, &projectile).join() {
        // Check position is within visible area
        if vec2_square_len(vec2_sub(pos.pos, app.render_app.camera)) > sq_radius {
            continue;
        }

        match proj.kind {
            ProjectileType::Plasma => {
                draw(
                    pos.pos[0], pos.pos[1],
                    pos.rot, 1.0,
                    DEF_COLOR,
                    BUF_PLASMA,
                );
            }
            ProjectileType::Rail => {
                draw(
                    pos.pos[0], pos.pos[1],
                    pos.rot, 1.0,
                    DEF_COLOR,
                    BUF_RAIL,
                );
            }
        }
    }

    // Draw particles
    for (pos, particle) in (&pos, &particle).join() {
        // Check position is within visible area
        if vec2_square_len(vec2_sub(pos.pos, app.render_app.camera)) > sq_radius {
            continue;
        }

        // TODO: Use different shader with alpha?
        match particle.which {
            ParticleType::Spark => {
                let alpha = (particle.lifetime as f32) / 0.2;
                draw(
                    pos.pos[0], pos.pos[1],
                    pos.rot, 1.0,
                    &[1.0, 1.0, 1.0, alpha],
                    BUF_SPARK,
                );
            }
            ParticleType::Exhaust => {
                let alpha = (particle.lifetime as f32).min(0.5);
                draw(
                    pos.pos[0], pos.pos[1],
                    pos.rot, 1.0,
                    &[1.0, 1.0, 1.0, alpha],
                    BUF_EXHAUST,
                );
            }
            ParticleType::Explosion => {
                let alpha = (particle.lifetime as f32 * 1.6).min(0.8);
                draw(
                    pos.pos[0], pos.pos[1],
                    pos.rot, 1.0,
                    &[1.0, 1.0, 1.0, alpha],
                    BUF_EXPLOSION,
                );
            }
            ParticleType::LaserHit => {
                let alpha = (particle.lifetime as f32 * 4.0).min(0.6);
                let size = 1.0 - particle.lifetime * 5.0;
                draw(
                    pos.pos[0], pos.pos[1],
                    pos.rot, size,
                    &[1.0, 1.0, 1.0, alpha],
                    BUF_LASER_HIT,
                );
            }
        }
    }
}

/// Generate vertex buffers for a Blocky object
fn generate_blocky_buffers(ent_id: u32, blocky: &Blocky, base_changed: bool) {
    // Base layer, doesn't change unless blocks are added/removed
    if base_changed {
        let mut buf_base = VertexVecs::default();
        for (pos, block) in &blocky.blocks {
            let mut buf_base = buf_base.translate(pos[0], pos[1]);
            match block.inner {
                BlockInner::Cockpit => {
                    buf_base.hollow_rect(
                        [-0.45, -0.45],
                        [0.45, 0.45],
                        0.05,
                        [1.0, 0.0, 0.0, 1.0],
                    );
                    buf_base.line(
                        [-0.2, -0.3],
                        [0.2, 0.0],
                        0.05,
                        [1.0, 0.0, 0.0, 1.0],
                    );
                    buf_base.line(
                        [0.2, 0.0],
                        [-0.2, 0.3],
                        0.05,
                        [1.0, 0.0, 0.0, 1.0],
                    );
                    buf_base.line(
                        [-0.2, 0.3],
                        [-0.2, -0.3],
                        0.05,
                        [1.0, 0.0, 0.0, 1.0],
                    );
                }
                BlockInner::Thruster { angle } => {
                    let mut buf_base = buf_base.rotate(angle);
                    for i in &[-0.4, 0.0] {
                        buf_base.filled_convex_polygon(
                            &[
                                [0.45 + i, 0.25],
                                [0.05 + i, 0.45],
                                [0.05 + i, -0.45],
                                [0.45 + i, -0.25],
                            ],
                            [0.5, 0.5, 0.5, 1.0],
                        );
                    }
                }
                BlockInner::PlasmaGun { .. } => {
                    buf_base.polygon(
                        &[
                            [-0.35, -0.35],
                            [0.0, -0.45],
                            [0.35, -0.35],
                            [0.45, 0.0],
                            [0.35, 0.35],
                            [0.0, 0.45],
                            [-0.35, 0.35],
                            [-0.45, 0.0],
                        ],
                        0.05,
                        [0.8, 0.8, 1.0, 1.0],
                    );
                }
                BlockInner::RailGun { .. } => {
                    buf_base.polygon(
                        &[
                            [-0.35, -0.35],
                            [0.0, -0.45],
                            [0.35, -0.35],
                            [0.45, 0.0],
                            [0.35, 0.35],
                            [0.0, 0.45],
                            [-0.35, 0.35],
                            [-0.45, 0.0],
                        ],
                        0.05,
                        [0.8, 0.8, 1.0, 1.0],
                    );
                }
                BlockInner::Armor => {
                    buf_base.hollow_rect(
                        [-0.4, -0.4],
                        [0.4, 0.4],
                        0.1,
                        [0.8, 0.8, 0.8, 1.0],
                    );
                }
                BlockInner::Rock => {
                    buf_base.filled_rect(
                        [-0.45, -0.45],
                        [0.45, 0.45],
                        [0.7, 0.5, 0.4, 1.0],
                    );
                    buf_base.hollow_rect(
                        [-0.46, -0.46],
                        [0.46, 0.46],
                        0.1,
                        [0.7, 0.7, 0.7, 1.0],
                    );
                }
            }
        }
        buf_base.store(entity_buffer(ent_id, 0), BufType::DYNAMIC);
    }

    // Dynamic layer, streamed each frame
    {
        let mut buf_dyn = VertexVecs::default();
        for (pos, block) in &blocky.blocks {
            let mut buf_dyn = buf_dyn.translate(pos[0], pos[1]);
            match block.inner {
                BlockInner::PlasmaGun { angle, .. } => {
                    buf_dyn.rotate(angle).filled_rect(
                        [0.0, -0.15], [0.6, 0.15],
                        [0.8, 0.8, 1.0, 1.0],
                    );
                }
                BlockInner::RailGun { angle, .. } => {
                    buf_dyn.rotate(angle).filled_rect(
                        [-0.25, -0.25], [0.65, 0.25],
                        [0.8, 0.8, 1.0, 1.0],
                    );
                }
                _ => {}
            }
        }
        if !buf_dyn.is_empty() || base_changed {
            buf_dyn.store(entity_buffer(ent_id, 1), BufType::STREAM);
        }
    }
}
