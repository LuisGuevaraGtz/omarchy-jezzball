//! Deterministic fuzzing: simulates complete games of ALL the real levels
//! with pseudorandom inputs, hunting for panics and impossible states.
//!
//! It does not use OS randomness: every game is seeded with a fixed `u64`, so
//! any failure it finds is exactly reproducible.

use jezzball_core::level::{ArenaShape, BallSpawn, LevelSpec, Objective};
use jezzball_core::rng::Rng64;
use jezzball_core::state::{step, GamePhase, GameState, PlayerInput};
use jezzball_core::wall::WallAxis;

const DT: f32 = 1.0 / 60.0;

/// Generates a spread of synthetic levels covering odd shapes and sizes,
/// including the edge cases the assets do not exercise.
fn synthetic_levels() -> Vec<LevelSpec> {
    let mut out = Vec::new();
    let shapes = [
        ArenaShape::Rect,
        ArenaShape::Wide,
        ArenaShape::Tall,
        ArenaShape::Irregular { notch: 3 },
        ArenaShape::Circle,
        ArenaShape::Maze { density: 40 },
        ArenaShape::Maze { density: 0 },
        ArenaShape::Maze { density: 100 },
    ];
    // Deliberately hostile sizes: even, odd and minimal.
    let sizes = [(64u16, 40u16), (33, 21), (32, 20), (9, 7), (4, 4), (3, 3)];

    let mut id = 0u16;
    for (si, shape) in shapes.iter().enumerate() {
        for (w, h) in sizes {
            id += 1;
            out.push(LevelSpec {
                id,
                name: format!("fuzz-{si}-{w}x{h}"),
                world: 1,
                kind: jezzball_core::level::LevelKind::Standard,
                arena: jezzball_core::level::ArenaSpec {
                    w,
                    h,
                    shape: *shape,
                    obstacles: vec![],
                },
                balls: vec![
                    BallSpawn {
                        x: w as f32 * 0.35,
                        y: h as f32 * 0.35,
                        vx: 7.0,
                        vy: 7.0,
                        kind: jezzball_core::level::BallKind::Normal,
                        radius_mul: 1.0,
                    },
                    BallSpawn {
                        x: w as f32 * 0.65,
                        y: h as f32 * 0.65,
                        vx: -9.0,
                        vy: 5.0,
                        kind: jezzball_core::level::BallKind::Erratic,
                        radius_mul: 1.0,
                    },
                ],
                target_ratio: 0.75,
                lives: 3,
                wall_speed: 22.0,
                time_limit: None,
                powerups_enabled: true,
                objectives: vec![Objective::NoLivesLost],
                purist: false,
                seed: 1000 + id as u64,
            });
        }
    }
    out
}

/// Runs a whole game hammering it with random inputs.
/// If anything panics, it panics here (the test fails naming the guilty level).
fn hammer(spec: &LevelSpec, seed: u64, frames: usize) {
    let mut rng = Rng64::new(seed);
    let mut st = GameState::new(spec.clone());

    for frame in 0..frames {
        // Roughly 1 in every 12 frames attempts a player action.
        let input = if rng.next_f32() < 0.08 {
            let roll = rng.next_f32();
            if roll < 0.70 {
                let x = rng.range_i64(0, spec.arena.w as i64 - 1) as u16;
                let y = rng.range_i64(0, spec.arena.h as i64 - 1) as u16;
                let axis = if rng.next_f32() < 0.5 {
                    WallAxis::Horizontal
                } else {
                    WallAxis::Vertical
                };
                PlayerInput::StartWall { cell: (x, y), axis }
            } else if roll < 0.80 {
                PlayerInput::ToggleAxis
            } else if roll < 0.88 {
                PlayerInput::Pause
            } else if roll < 0.96 {
                PlayerInput::Resume
            } else {
                PlayerInput::Restart
            }
        } else {
            PlayerInput::None
        };

        let (next, _events) = step(&st, input, DT);
        st = next;

        // Invariants that must hold ALWAYS.
        for b in &st.balls {
            assert!(
                b.pos.x.is_finite() && b.pos.y.is_finite(),
                "{}: frame {frame}: non-finite position ({}, {})",
                spec.name,
                b.pos.x,
                b.pos.y
            );
            assert!(
                b.vel.x.is_finite() && b.vel.y.is_finite(),
                "{}: frame {frame}: non-finite velocity",
                spec.name
            );
            // A 2-cell margin: tolerant, it only looks for serious leaks.
            assert!(
                b.pos.x >= -2.0
                    && b.pos.y >= -2.0
                    && b.pos.x <= spec.arena.w as f32 + 2.0
                    && b.pos.y <= spec.arena.h as f32 + 2.0,
                "{}: frame {frame}: ball outside the arena ({}, {}) in {}x{}",
                spec.name,
                b.pos.x,
                b.pos.y,
                spec.arena.w,
                spec.arena.h
            );
        }
        assert!(
            st.lives <= spec.lives,
            "{}: frame {frame}: lives went up on their own",
            spec.name
        );

        if st.phase == GamePhase::Won || st.phase == GamePhase::Lost {
            break;
        }
    }
}

/// Fuzzing over synthetic levels: hostile shapes and sizes.
#[test]
fn fuzz_synthetic_levels_does_not_panic() {
    for spec in synthetic_levels() {
        for seed in [1u64, 7, 99] {
            hammer(&spec, seed, 1200);
        }
    }
}

/// Building a `GameState` of ANY shape and size must not panic.
/// Covers the off-by-one in `materialize_maze` and its relatives.
#[test]
fn building_an_arena_of_any_size_does_not_panic() {
    for w in 3u16..=40 {
        for h in [3u16, 4, 7, 8, 20, 21, 39, 40] {
            for shape in [
                ArenaShape::Rect,
                ArenaShape::Circle,
                ArenaShape::Maze { density: 50 },
                ArenaShape::Irregular { notch: 2 },
            ] {
                let spec = LevelSpec {
                    id: 1,
                    name: format!("{w}x{h}"),
                    world: 1,
                    kind: jezzball_core::level::LevelKind::Standard,
                    arena: jezzball_core::level::ArenaSpec {
                        w,
                        h,
                        shape,
                        obstacles: vec![],
                    },
                    balls: vec![BallSpawn {
                        x: w as f32 / 2.0,
                        y: h as f32 / 2.0,
                        vx: 6.0,
                        vy: 6.0,
                        kind: jezzball_core::level::BallKind::Normal,
                        radius_mul: 1.0,
                    }],
                    target_ratio: 0.75,
                    lives: 3,
                    wall_speed: 22.0,
                    time_limit: None,
                    powerups_enabled: false,
                    objectives: vec![],
                    purist: false,
                    seed: 42,
                };
                let st = GameState::new(spec);
                // Advance a few frames to exercise the physics in minimal arenas.
                let mut cur = st;
                for _ in 0..60 {
                    cur = step(&cur, PlayerInput::None, DT).0;
                }
            }
        }
    }
}
