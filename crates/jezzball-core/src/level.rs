//! Level types. This file is the NORMATIVE contract with the level workers
//! (LEVEL_SCHEMA.md): the names of types, fields and variants are mandatory
//! and must not be renamed.
//!
//! `serde` is optional (feature `serde`, enabled by default): the on-disk
//! format (`assets/levels/*.ron`) is used by the persistence layer, never by
//! the core.

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LevelSpec {
    pub id: u16,      // 1-based, unique within the mode
    pub name: String, // short, displayable in the HUD
    pub world: u8,    // 0 = Original; 1..=6 = Enhanced worlds
    pub kind: LevelKind,
    pub arena: ArenaSpec,
    pub balls: Vec<BallSpawn>,
    pub target_ratio: f32, // 0.0..1.0, typically 0.75
    pub lives: u8,
    pub wall_speed: f32,         // cells/sec per front, typically 22.0
    pub time_limit: Option<f32>, // seconds; None = no clock
    pub powerups_enabled: bool,
    pub objectives: Vec<Objective>, // max 3; empty in Original
    pub purist: bool,               // true => no combos, no power-ups, no objectives
    pub seed: u64,                  // seed of the deterministic PRNG
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LevelKind {
    Standard,
    Boss,
    SpeedRun,
    Chaos,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ArenaSpec {
    pub w: u16, // cells, 32..=96
    pub h: u16, // cells, 20..=60
    pub shape: ArenaShape,
    pub obstacles: Vec<Obstacle>, // pre-placed Solid cells
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ArenaShape {
    Rect,                     // standard rectangle
    Wide,                     // wide ratio
    Tall,                     // tall ratio
    Irregular { notch: u16 }, // corners cut back by `notch` cells
    Circle,                   // inscribed ellipse; outside = Solid
    Maze { density: u8 },     // 0..=100, corridors generated from `seed`
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Obstacle {
    Block {
        x: u16,
        y: u16,
        w: u16,
        h: u16,
    },
    // moves in a straight line, bouncing; destroys walls under construction
    Mover {
        x: f32,
        y: f32,
        w: u16,
        h: u16,
        vx: f32,
        vy: f32,
    },
    // zone that can NEVER be filled or closed (counts as non-fillable)
    NoSplit {
        x: u16,
        y: u16,
        w: u16,
        h: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BallSpawn {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub kind: BallKind,
    pub radius_mul: f32, // 1.0 = normal
}

/// See ARCHITECTURE.md §8 for the semantics of each type.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Objective {
    ClearRatio(f32),
    NoLivesLost,
    UnderTime(f32),
    MinScore(u32),
    KeepCombo(u8),
    NoPowerUps,
}

/// Ball kind (LEVEL_SCHEMA.md, semantics in ARCHITECTURE.md §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BallKind {
    Normal,
    Fast,
    Erratic,
    Splitter,
    Heavy,
}

impl BallKind {
    /// Base radius (in cells) of each ball kind (ARCHITECTURE.md §8).
    pub fn base_radius(self) -> f32 {
        match self {
            BallKind::Normal | BallKind::Fast | BallKind::Erratic | BallKind::Splitter => 0.45,
            BallKind::Heavy => 1.2,
        }
    }

    /// Speed multiplier relative to the spawn's speed (ARCHITECTURE.md §8).
    pub fn speed_mult(self) -> f32 {
        match self {
            BallKind::Normal | BallKind::Erratic | BallKind::Splitter => 1.0,
            BallKind::Fast => 1.8,
            BallKind::Heavy => 0.6,
        }
    }

    /// The `Heavy` ball ignores the shield: it ALWAYS destroys walls under
    /// construction (ARCHITECTURE.md §8).
    pub fn ignores_shield(self) -> bool {
        self == BallKind::Heavy
    }
}

/// Game mode (ARCHITECTURE.md §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Original,
    Enhanced,
}

/// World of levels (ARCHITECTURE.md §7).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct World {
    pub index: u8,
    pub name: String,
    pub levels: Vec<LevelSpec>,
}

/// One star = one objective achieved (max 3 per level).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Star {
    pub index: u8,
    pub objective: Objective,
    pub achieved: bool,
}
