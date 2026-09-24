//! Defensa del motor contra specs de nivel inconsistentes: los spawns de
//! bola que no caen en una celda `Open` se reubican (o se descartan si la
//! arena está degenerada). Ver `state::new_or_relocated_ball`.

use jezzball_core::geom::Vec2;
use jezzball_core::grid::Cell;
use jezzball_core::level::{
    ArenaShape, ArenaSpec, BallKind, BallSpawn, LevelKind, LevelSpec, Objective, Obstacle,
};
use jezzball_core::state::GameState;

fn ball(x: f32, y: f32, vx: f32, vy: f32, kind: BallKind) -> BallSpawn {
    BallSpawn {
        x,
        y,
        vx,
        vy,
        kind,
        radius_mul: 1.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn spec(
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

/// Un spawn sobre un bloque `Solid` se reubica a la celda `Open` más cercana
/// (primera por BFS) que no sea `NoSplit`, conservando velocidad, `kind` y
/// radio.
///
/// Disposición: bloque sólido en celdas (4..=6, 2..=3); `NoSplit` en (5, 4);
/// spawn en (5, 3), que es `Solid`. El BFS parte de (5, 3):
///   - (6, 3) `Solid`, (5, 4) `Open` pero `NoSplit` (se salta),
///   - (4, 3) y (5, 2) `Solid`,
///   - (7, 3) `Open` y no `NoSplit` -> destino.
#[test]
fn spawn_sobre_solid_se_reubica_a_la_abierta_mas_cercana() {
    let s = GameState::new(spec(
        12,
        8,
        vec![
            Obstacle::Block {
                x: 4,
                y: 2,
                w: 3,
                h: 2,
            },
            Obstacle::NoSplit {
                x: 5,
                y: 4,
                w: 1,
                h: 1,
            },
        ],
        vec![
            ball(5.5, 3.5, 4.0, -3.0, BallKind::Fast),
            ball(3.0, 6.0, 2.0, 1.0, BallKind::Normal),
        ],
        22.0,
        0.9,
        3,
        false,
        vec![],
        false,
    ));

    // La celda del spawn original (5, 3) es `Solid` (el bug que se defiende).
    assert_eq!(s.arena.grid.get(5, 3), Cell::Solid);
    // La candidata inmediata (5, 4) es `Open` pero `NoSplit`: el BFS la salta.
    assert!(s.arena.grid.is_no_split(5, 4));

    // La segunda bola nace en celda `Open`: no se toca.
    let a = &s.balls[0];
    let b = &s.balls[1];
    assert_eq!(a.pos, Vec2::new(7.5, 3.5), "reubicada al centro de (7, 3)");
    assert_eq!(a.kind, BallKind::Fast);
    assert_eq!(a.vel, Vec2::new(4.0, -3.0) * BallKind::Fast.speed_mult());
    assert_eq!(a.radius, 0.45);
    assert_eq!(b.pos, Vec2::new(3.0, 6.0), "spawn válido sin cambios");
}

/// Arena degenerada: si no existe ninguna celda `Open`, la bola se descarta
/// en vez de provocar un estado inválido.
#[test]
fn arena_sin_celdas_abiertas_descarta_la_bola() {
    let s = GameState::new(spec(
        12,
        8,
        vec![Obstacle::Block {
            x: 0,
            y: 0,
            w: 12,
            h: 8,
        }],
        vec![ball(6.5, 4.5, 3.0, 2.0, BallKind::Normal)],
        22.0,
        0.9,
        3,
        false,
        vec![],
        false,
    ));

    assert_eq!(
        s.balls.len(),
        0,
        "bola descartada, la arena está degenerada"
    );
    assert_eq!(s.arena.grid.open_count(), 0);
}
