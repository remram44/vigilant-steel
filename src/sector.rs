//! Map sector.
//!
//! The universe is divided into sectors, which are "paged in" when needed.

use physics::Position;
use specs::{Component, Entities, Entity, Join, Write, System, VecStorage, WriteStorage};
use std::collections::HashMap;
use std::fmt;

pub const SECTOR_SIZE: f32 = 50.0;

/// Identifies a sector
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SectorId(u32);

impl fmt::Debug for SectorId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Sec#{}", self.0)
    }
}

impl Component for SectorId {
    type Storage = VecStorage<Self>;
}

/// A sector of the map
pub struct Sector {
    // East, South, West, North
    pub neighbors: [Option<SectorId>; 4],
    // TODO: Use a quadtree for entities
    pub overlapping_entities: Vec<Entity>,
}

/// World resource containing all known sectors
#[derive(Default)]
pub struct SectorManager {
    pub sectors: HashMap<SectorId, Sector>,
}

impl SectorManager {
    pub fn new() -> SectorManager {
        // 1 2 3
        // 4 5 6
        // loops horizontally
        let mut sectors = HashMap::new();
        sectors.insert(
            SectorId(1),
            Sector {
                neighbors: [
                    Some(SectorId(2)),
                    Some(SectorId(4)),
                    Some(SectorId(3)),
                    None,
                ],
                overlapping_entities: vec![],
            },
        );
        sectors.insert(
            SectorId(2),
            Sector {
                neighbors: [
                    Some(SectorId(3)),
                    Some(SectorId(5)),
                    Some(SectorId(1)),
                    None,
                ],
                overlapping_entities: vec![],
            },
        );
        sectors.insert(
            SectorId(3),
            Sector {
                neighbors: [
                    Some(SectorId(1)),
                    Some(SectorId(6)),
                    Some(SectorId(2)),
                    None,
                ],
                overlapping_entities: vec![],
            },
        );
        sectors.insert(
            SectorId(4),
            Sector {
                neighbors: [
                    Some(SectorId(5)),
                    None,
                    Some(SectorId(6)),
                    Some(SectorId(1)),
                ],
                overlapping_entities: vec![],
            },
        );
        sectors.insert(
            SectorId(5),
            Sector {
                neighbors: [
                    Some(SectorId(6)),
                    None,
                    Some(SectorId(4)),
                    Some(SectorId(2)),
                ],
                overlapping_entities: vec![],
            },
        );
        sectors.insert(
            SectorId(6),
            Sector {
                neighbors: [
                    Some(SectorId(4)),
                    None,
                    Some(SectorId(5)),
                    Some(SectorId(3)),
                ],
                overlapping_entities: vec![],
            },
        );
        SectorManager { sectors }
    }

    pub fn get(&mut self, id: SectorId) -> Option<&mut Sector> {
        self.sectors.get_mut(&id)
    }
}

pub struct SysSector;

impl<'a> System<'a> for SysSector {
    type SystemData = (
        Entities<'a>,
        Write<'a, SectorManager>,
        WriteStorage<'a, SectorId>,
        WriteStorage<'a, Position>,
    );

    fn run(
        &mut self,
        (
            entities,
            mut sector_manager,
            mut sector_ids,
            mut pos,
        ): Self::SystemData,
    ) {
        // Assign entities to their sectors
        for (ent, sector_id, pos) in (&entities, &mut sector_ids, &mut pos).join() {
            // Get the sector the entity is in
            let sector = if let Some(s) = sector_manager.get(*sector_id) {
                s
            } else {
                error!("Entity {:?} is in unknown sector {:?}", ent, *sector_id);
                continue;
            };

            // Update the current sector if out of bounds
            let mut new_sector = None;
            let mut new_pos = pos.pos;
            if pos.pos[0] > SECTOR_SIZE {
                new_sector = sector.neighbors[0];
                new_pos[0] -= SECTOR_SIZE;
            } else if pos.pos[0] < 0.0 {
                new_sector = sector.neighbors[2];
                new_pos[0] += SECTOR_SIZE;
            } else if pos.pos[1] > SECTOR_SIZE {
                new_sector = sector.neighbors[3];
                new_pos[1] -= SECTOR_SIZE;
            } else if pos.pos[1] < 0.0 {
                new_sector = sector.neighbors[1];
                new_pos[1] += SECTOR_SIZE;
            }
            if let Some(id) = new_sector {
                *sector_id = id;
                pos.pos = new_pos;
            }

            // TODO: Set sectors we overlap with
        }

        // TODO: Load/unload sectors
    }
}
