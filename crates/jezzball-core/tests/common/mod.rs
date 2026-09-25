//! Common helpers for the core integration tests (ARCHITECTURE.md §15).
//!
//! All tests are deterministic: they use `dt = 0.04` and the fixed seed that
//! `spec` injects into the `LevelSpec`. They do not depend on exact timing
//! bounds, only on OUTCOMES (events, lives, consolidated/destroyed walls).

use jezzball_core::level::{
    ArenaShape, ArenaSpec, BallKind, BallSpawn, LevelKind, LevelSpec, Objective, Obstacle,
};
use jezzball_core::state::{step, GameEvent, GamePhase, GameState, PlayerInput};

pub const DT: f32 = 0.04;

/// Pure-velocity ball (the `kind` scales it internally).
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

/// A straightforward, unrigged `LevelSpec` for tests.
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

/// Advances `n` frames at a fixed `dt` with `PlayerInput::None`, accumulating
/// the events. Stops early if the game ends (Won/Lost).
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
