//! Tipos de niveles. Este fichero es el contrato NORMATIVO con los workers
//! de niveles (LEVEL_SCHEMA.md): los nombres de tipos, campos y variantes son
//! obligatorios y no deben renombrarse.
//!
//! `serde` es opcional (feature `serde`, activada por defecto): el formato en
//! disco (`assets/levels/*.ron`) lo usa la capa de persistencia, nunca el core.

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LevelSpec {
    pub id: u16,                  // 1-based, único dentro del modo
    pub name: String,             // corto, mostrable en HUD
    pub world: u8,                // 0 = Original; 1..=6 = mundos Enhanced
    pub kind: LevelKind,
    pub arena: ArenaSpec,
    pub balls: Vec<BallSpawn>,
    pub target_ratio: f32,        // 0.0..1.0, típico 0.75
    pub lives: u8,
    pub wall_speed: f32,          // celdas/seg por frente, típico 22.0
    pub time_limit: Option<f32>,  // segundos; None = sin reloj
    pub powerups_enabled: bool,
    pub objectives: Vec<Objective>, // máx 3; vacío en Original
    pub purist: bool,             // true => sin combos, sin power-ups, sin objetivos
    pub seed: u64,                // semilla del PRNG determinista
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
    pub w: u16,                   // celdas, 32..=96
    pub h: u16,                   // celdas, 20..=60
    pub shape: ArenaShape,
    pub obstacles: Vec<Obstacle>, // celdas Solid pre-colocadas
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ArenaShape {
    Rect,                         // rectángulo estándar
    Wide,                         // ratio ancho
    Tall,                         // ratio alto
    Irregular { notch: u16 },     // esquinas recortadas de `notch` celdas
    Circle,                       // elipse inscrita; fuera = Solid
    Maze { density: u8 },         // 0..=100, corredores generados por `seed`
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Obstacle {
    Block { x: u16, y: u16, w: u16, h: u16 },
    // se mueve en línea recta rebotando; destruye muros en construcción
    Mover { x: f32, y: f32, w: u16, h: u16, vx: f32, vy: f32 },
    // zona que NUNCA puede rellenarse ni cerrarse (cuenta como no-fillable)
    NoSplit { x: u16, y: u16, w: u16, h: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BallSpawn {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub kind: BallKind,
    pub radius_mul: f32,          // 1.0 = normal
}

/// Ver ARCHITECTURE.md §8 para la semántica de cada tipo.
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

/// Tipo de bola (LEVEL_SCHEMA.md, semántica en ARCHITECTURE.md §8).
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
    /// Radio base (en celdas) de cada tipo de bola (ARCHITECTURE.md §8).
    pub fn base_radius(self) -> f32 {
        match self {
            BallKind::Normal | BallKind::Fast | BallKind::Erratic | BallKind::Splitter => 0.45,
            BallKind::Heavy => 1.2,
        }
    }

    /// Multiplicador de velocidad respecto a la del spawn (ARCHITECTURE.md §8).
    pub fn speed_mult(self) -> f32 {
        match self {
            BallKind::Normal | BallKind::Erratic | BallKind::Splitter => 1.0,
            BallKind::Fast => 1.8,
            BallKind::Heavy => 0.6,
        }
    }

    /// La bola `Heavy` ignora el escudo: destruye SIEMPRE los muros
    /// en construcción (ARCHITECTURE.md §8).
    pub fn ignores_shield(self) -> bool {
        self == BallKind::Heavy
    }
}

/// Modo de juego (ARCHITECTURE.md §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Original,
    Enhanced,
}

/// Mundo de niveles (ARCHITECTURE.md §7).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct World {
    pub index: u8,
    pub name: String,
    pub levels: Vec<LevelSpec>,
}

/// Una estrella = un objetivo cumplido (máx 3 por nivel).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Star {
    pub index: u8,
    pub objective: Objective,
    pub achieved: bool,
}