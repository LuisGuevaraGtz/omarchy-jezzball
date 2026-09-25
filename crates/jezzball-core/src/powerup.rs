//! Power-ups (ARCHITECTURE.md §9). Solo existen en Enhanced (mundos ≥ 5) y
//! quedan completamente desactivados en el modo Original (`purist: true`).

/// Tipos de power-up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerUpKind {
    SlowMotion, // dt de bolas × 0.5 durante 5 s
    Freeze,     // bolas quietas durante 3 s
    DoubleWall, // permite 2 builders simultáneos (1 uso)
    Shield,     // el próximo muro absorbe 1 impacto de bola
    RemoveBall, // elimina la bola más lenta (1 uso)
}

/// Power-up recogible esperando en el suelo de la arena.
#[derive(Clone, Debug, PartialEq)]
pub struct PowerUp {
    pub kind: PowerUpKind,
    pub cell: (u16, u16),
}

/// Power-up temporal active (SlowMotion/Freeze).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActivePowerUp {
    pub kind: PowerUpKind,
    pub remaining: f32,
}

/// Máximo de power-ups simultáneos recogibles en la arena.
pub const PICKUP_MAX: usize = 2;
/// Tamaño máximo del inventario del jugador.
pub const INVENTORY_MAX: usize = 4;

impl ActivePowerUp {
    /// Duración (segundos) del efecto temporal de cada tipo.
    pub fn duration(kind: PowerUpKind) -> f32 {
        match kind {
            PowerUpKind::SlowMotion => 5.0,
            PowerUpKind::Freeze => 3.0,
            _ => 0.0,
        }
    }
}

/// Lista de todos los tipos (para elegir el kind de un spawn).
pub fn all_kinds() -> [PowerUpKind; 5] {
    [
        PowerUpKind::SlowMotion,
        PowerUpKind::Freeze,
        PowerUpKind::DoubleWall,
        PowerUpKind::Shield,
        PowerUpKind::RemoveBall,
    ]
}
