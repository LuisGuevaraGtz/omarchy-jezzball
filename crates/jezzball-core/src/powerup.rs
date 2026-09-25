//! Power-ups (ARCHITECTURE.md §9). They only exist in Enhanced (worlds >= 5)
//! and are completely disabled in Original mode (`purist: true`).

/// Power-up types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerUpKind {
    SlowMotion, // ball dt x 0.5 for 5 s
    Freeze,     // balls stand still for 3 s
    DoubleWall, // allows 2 simultaneous builders (1 use)
    Shield,     // the next wall absorbs 1 ball impact
    RemoveBall, // removes the slowest ball (1 use)
}

/// Collectible power-up waiting on the arena floor.
#[derive(Clone, Debug, PartialEq)]
pub struct PowerUp {
    pub kind: PowerUpKind,
    pub cell: (u16, u16),
}

/// Temporary active power-up (SlowMotion/Freeze).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActivePowerUp {
    pub kind: PowerUpKind,
    pub remaining: f32,
}

/// Maximum number of collectible power-ups present in the arena at once.
pub const PICKUP_MAX: usize = 2;
/// Maximum size of the player's inventory.
pub const INVENTORY_MAX: usize = 4;

impl ActivePowerUp {
    /// Duration (seconds) of the temporary effect of each type.
    pub fn duration(kind: PowerUpKind) -> f32 {
        match kind {
            PowerUpKind::SlowMotion => 5.0,
            PowerUpKind::Freeze => 3.0,
            _ => 0.0,
        }
    }
}

/// List of every type (used to pick the kind of a spawn).
pub fn all_kinds() -> [PowerUpKind; 5] {
    [
        PowerUpKind::SlowMotion,
        PowerUpKind::Freeze,
        PowerUpKind::DoubleWall,
        PowerUpKind::Shield,
        PowerUpKind::RemoveBall,
    ]
}
