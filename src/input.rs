//! Keyboard input structure.

/// A key status.
///
/// This enum allow the game to distinguish between keys that were pressed this
/// frame, and keys that are still down as part of a earlier press.
///
/// This is useful as some actions must be triggered only on press, and others
/// can be repeated as long as the key is down.
#[derive(PartialEq, Eq)]
pub enum Press {
    /// Key is up (not pressed).
    UP,
    /// Key was pressed this frame. Perform actions triggered on press.
    PRESSED,
    /// Key was kept down. Repeating actions can happen.
    KEPT,
}

// Input resource, stores the keyboard state
pub struct Input {
    pub movement: [f64; 2],
    pub fire: Press,
}

impl Input {
    pub fn new() -> Input {
        Input {
            movement: [0.0, 0.0],
            fire: Press::UP,
        }
    }
}
