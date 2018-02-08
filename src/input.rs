#[derive(PartialEq, Eq)]
pub enum Press {
    UP,
    PRESSED,
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
