//! Helpers comunes de los tests de integración del core (ARCHITECTURE.md §15).
//!
//! Todos los tests son deterministas: usan `dt = 0.04` y el seed fijo que
//! `spec` inyecta en el `LevelSpec`. No dependen de cotas temporales exactas,
//! solo de los OUTCOMES (eventos, vidas, muros consolidados/destruidos).

use jezzball_core::level::{
    ArenaShape, ArenaSpec, BallKind, BallSpawn, LevelKind, LevelSpec, Objective, Obstacle,
};
use jezzball_core::state::{step, GameEvent, GamePhase, GameState, PlayerInput};

pub const DT: f32 = 0.04;

/// Bola de velocidad pura (los `kind` la escalan internamente).
pub fn ball(x: f32, y: f32, vx: f32, vy: f32, kind: BallKind) -> BallSpawn {
    BallSpawn {
        x,
        y,
        vx,
        vy,
        kind,
        radius_mul: 1.0,
    }
}

/// `LevelSpec` recto y sin amañar para tests.
#[allow(clippy::too_many_arguments)]
pub fn spec(
    w: u16,
    h: u16,
    obstacles: Vec<Obstacle>,
    balls: Vec<BallSpawn>,
    wall_speed: f32,
    target_ratio: f32,
    lives: u8,
    powerups_enabled: bool,
    objectives: Vec<Objective>,
    purist: bool,
) -> LevelSpec {
    LevelSpec {
        id: 1,
        name: "test".into(),
        world: 5,
        kind: LevelKind::Standard,
        arena: ArenaSpec {
            w,
            h,
            shape: ArenaShape::Rect,
            obstacles,
        },
        balls,
        target_ratio,
        lives,
        wall_speed,
        time_limit: None,
        powerups_enabled,
        objectives,
        purist,
        seed: 1234,
    }
}

/// Avanza `n` frames a `dt` fijo con `PlayerInput::None`, acumulando los
/// eventos. Se detiene antes si la partida termina (Won/Lost).
pub fn frames(state: &GameState, n: u32) -> (GameState, Vec<GameEvent>) {
    let mut s = state.clone();
    let mut evs = Vec::new();
    for _ in 0..n {
        if s.phase != GamePhase::Running {
            break;
        }
        let (nxt, e) = step(&s, PlayerInput::None, DT);
        s = nxt;
        evs.extend(e);
    }
    (s, evs)
}
