use specs::{Component, HashMapStorage, NullStorage, VecStorage};

/// Replicated entities have an id to match them on multiple machines.
pub struct Replicated {
    pub id: u64,
    pub last_update: u32,
}

impl Replicated {
    pub fn new() -> Replicated {
        Replicated {
            id: 0,
            last_update: 0,
        }
    }
}

impl Component for Replicated {
    type Storage = VecStorage<Self>;
}

/// Mark an entity to be deleted everywhere.
#[derive(Default)]
pub struct Delete;

impl Component for Delete {
    type Storage = NullStorage<Self>;
}

/// Flag that marks an entity as dirty, eg needs to be sent to clients.
#[derive(Default)]
pub struct Dirty;

impl Component for Dirty {
    type Storage = NullStorage<Self>;
}

/// Server component attached to entities controlled by clients.
///
/// Multiple entities can be controlled by the same client, and that's fine.
pub struct ClientControlled {
    pub client_id: u64,
}

impl Component for ClientControlled {
    type Storage = HashMapStorage<Self>;
}
