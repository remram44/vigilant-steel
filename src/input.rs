//! Keyboard input structure.
//!
//! This is a simple structure used as a specs resource to store input from the
//! local player.

/// A key status.
///
/// This enum allow the game to distinguish between keys that were pressed this
/// frame, and keys that are still down as part of a earlier press.
///
/// This is useful as some actions must be triggered only on press, and others
/// can be repeated as long as the key is down.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Key is up (not pressed).
    UP,
    /// Key was pressed this frame. Perform actions triggered on press.
    PRESSED,
    /// Key was kept down. Repeating actions can happen.
    KEPT,
}

impl Press {
    fn update(&mut self) {
        if let Press::PRESSED = *self {
            *self = Press::KEPT;
        }
    }
}

/// Input resource, stores the local user's controls.
pub struct Input {
    pub movement: [f64; 2],
    pub rotation: f64,
    pub fire: Press,
    pub tractor_beam: Press,
    pub mouse: [f64; 2],
}

impl Input {
    pub fn new() -> Input {
        Input {
            movement: [0.0, 0.0],
            rotation: 0.0,
            fire: Press::UP,
            tractor_beam: Press::UP,
            mouse: [0.0; 2],
        }
    }

    /// Update status of keys, called once per frame.
    pub fn update(&mut self) {
        self.fire.update();
        self.tractor_beam.update();
    }
}
