//! Engine defence against inconsistent level specs: ball spawns that do not
//! land on an `Open` cell are relocated (or discarded if the arena is
//! degenerate). See `state::new_or_relocated_ball`.

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

/// A spawn on top of a `Solid` block is relocated to the nearest `Open` cell
/// (first one by BFS) that is not `NoSplit`, preserving velocity, `kind` and
/// radius.
///
/// Layout: solid block on cells (4..=6, 2..=3); `NoSplit` at (5, 4); spawn at
/// (5, 3), which is `Solid`. The BFS starts from (5, 3):
///   - (6, 3) `Solid`, (5, 4) `Open` but `NoSplit` (skipped),
///   - (4, 3) and (5, 2) `Solid`,
///   - (7, 3) `Open` and not `NoSplit` -> destination.
#[test]
fn spawn_on_solid_relocates_to_the_nearest_open_cell() {
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

    // The original spawn cell (5, 3) is `Solid` (the bug being guarded against).
    assert_eq!(s.arena.grid.get(5, 3), Cell::Solid);
    // The immediate candidate (5, 4) is `Open` but `NoSplit`: the BFS skips it.
    assert!(s.arena.grid.is_no_split(5, 4));

    // The second ball spawns on an `Open` cell: it is left alone.
    let a = &s.balls[0];
    let b = &s.balls[1];
    assert_eq!(
        a.pos,
        Vec2::new(7.5, 3.5),
        "relocated to the centre of (7, 3)"
    );
    assert_eq!(a.kind, BallKind::Fast);
    assert_eq!(a.vel, Vec2::new(4.0, -3.0) * BallKind::Fast.speed_mult());
    assert_eq!(a.radius, 0.45);
    assert_eq!(b.pos, Vec2::new(3.0, 6.0), "valid spawn left unchanged");
}

/// Degenerate arena: if no `Open` cell exists at all, the ball is discarded
/// instead of causing an invalid state.
#[test]
fn arena_with_no_open_cells_discards_the_ball() {
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

    assert_eq!(s.balls.len(), 0, "ball discarded, the arena is degenerate");
    assert_eq!(s.arena.grid.open_count(), 0);
}
