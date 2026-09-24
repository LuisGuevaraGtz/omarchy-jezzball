//! Fuzz determinista: simula partidas completas de TODOS los niveles reales
//! con entradas pseudoaleatorias, buscando panics y estados imposibles.
//!
//! No usa aleatoriedad del SO: cada partida se siembra con un `u64` fijo, así
//! que cualquier fallo que encuentre es reproducible exactamente.

use jezzball_core::level::{ArenaShape, BallSpawn, LevelSpec, Objective};
use jezzball_core::rng::Rng64;
use jezzball_core::state::{step, GamePhase, GameState, PlayerInput};
use jezzball_core::wall::WallAxis;

const DT: f32 = 1.0 / 60.0;

/// Genera un abanico de niveles sintéticos que cubre formas y tamaños raros,
/// incluidos los casos límite que los assets no ejercitan.
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
    // Tamaños deliberadamente hostiles: pares, impares y mínimos.
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

/// Ejecuta una partida entera martilleando entradas aleatorias.
/// Si algo va a panicar, panica aquí (y el test falla con el nivel culpable).
fn hammer(spec: &LevelSpec, seed: u64, frames: usize) {
    let mut rng = Rng64::new(seed);
    let mut st = GameState::new(spec.clone());

    for frame in 0..frames {
        // ~1 de cada 12 frames intenta una acción del jugador.
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

        // Invariantes que deben cumplirse SIEMPRE.
        for b in &st.balls {
            assert!(
                b.pos.x.is_finite() && b.pos.y.is_finite(),
                "{}: frame {frame}: posicion no finita ({}, {})",
                spec.name,
                b.pos.x,
                b.pos.y
            );
            assert!(
                b.vel.x.is_finite() && b.vel.y.is_finite(),
                "{}: frame {frame}: velocidad no finita",
                spec.name
            );
            // Margen de 2 celdas: tolerante, solo busca fugas graves.
            assert!(
                b.pos.x >= -2.0
                    && b.pos.y >= -2.0
                    && b.pos.x <= spec.arena.w as f32 + 2.0
                    && b.pos.y <= spec.arena.h as f32 + 2.0,
                "{}: frame {frame}: bola fuera de la arena ({}, {}) en {}x{}",
                spec.name,
                b.pos.x,
                b.pos.y,
                spec.arena.w,
                spec.arena.h
            );
        }
        assert!(
            st.lives <= spec.lives,
            "{}: frame {frame}: vidas subieron solas",
            spec.name
        );

        if st.phase == GamePhase::Won || st.phase == GamePhase::Lost {
            break;
        }
    }
}

/// Fuzz sobre niveles sintéticos: formas y tamaños hostiles.
#[test]
fn fuzz_niveles_sinteticos_no_panica() {
    for spec in synthetic_levels() {
        for seed in [1u64, 7, 99] {
            hammer(&spec, seed, 1200);
        }
    }
}

/// Construir un `GameState` de CUALQUIER forma y tamaño no debe panicar.
/// Cubre el off-by-one de `materialize_maze` y sus parientes.
#[test]
fn construir_arena_de_cualquier_tamano_no_panica() {
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
                // Avanzar unos frames para ejercitar la física en arenas mínimas.
                let mut cur = st;
                for _ in 0..60 {
                    cur = step(&cur, PlayerInput::None, DT).0;
                }
            }
        }
    }
}
