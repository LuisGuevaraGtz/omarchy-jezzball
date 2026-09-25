//! Area partitioning and closing (ARCHITECTURE.md §4).
//!
//! This is the heart of JezzBall: when a wall consolidates, the regions with
//! no ball are closed. A failure here shows up immediately while playing (an
//! area that does not close, or that closes with a ball inside).

use jezzball_core::grid::Cell;
use jezzball_core::level::{
    ArenaShape, ArenaSpec, BallKind, BallSpawn, LevelKind, LevelSpec, Objective,
};
use jezzball_core::state::{step, GameEvent, GameState, PlayerInput};
use jezzball_core::wall::WallAxis;

const DT: f32 = 1.0 / 60.0;

fn level_spec(w: u16, h: u16, balls: Vec<BallSpawn>, purist: bool) -> LevelSpec {
    LevelSpec {
        id: 1,
        name: "t".into(),
        world: if purist { 0 } else { 1 },
        kind: LevelKind::Standard,
        arena: ArenaSpec {
            w,
            h,
            shape: ArenaShape::Rect,
            obstacles: vec![],
        },
        balls,
        target_ratio: 0.75,
        lives: 3,
        wall_speed: 22.0,
        time_limit: None,
        powerups_enabled: false,
        objectives: vec![Objective::NoLivesLost],
        purist,
        seed: 7,
    }
}

fn ball(x: f32, y: f32, vx: f32, vy: f32) -> BallSpawn {
    BallSpawn {
        x,
        y,
        vx,
        vy,
        kind: BallKind::Normal,
        radius_mul: 1.0,
    }
}

/// Advances until the wall consolidates, returning the state and whether there
/// was a `WallCompleted` event.
fn until_consolidated(mut st: GameState, max_frames: usize) -> (GameState, bool) {
    let mut consolidated = false;
    for _ in 0..max_frames {
        let (next, events) = step(&st, PlayerInput::None, DT);
        st = next;
        if events
            .iter()
            .any(|e| matches!(e, GameEvent::WallCompleted { .. }))
        {
            consolidated = true;
            break;
        }
    }
    (st, consolidated)
}

/// A region with no balls closes completely; the one holding a ball does not.
#[test]
fn closes_only_the_region_without_balls() {
    // 40x20 arena. Both balls live in the RIGHT HALF (x > 20).
    let spec = level_spec(
        40,
        20,
        vec![ball(30.0, 8.0, 6.0, 6.0), ball(34.0, 12.0, -6.0, 6.0)],
        false,
    );
    let st = GameState::new(spec);
    // Vertical wall on column 20: splits the arena into two halves.
    let (st, _) = step(
        &st,
        PlayerInput::StartWall {
            cell: (20, 10),
            axis: WallAxis::Vertical,
        },
        DT,
    );
    let (st, consolidated) = until_consolidated(st, 2000);
    assert!(consolidated, "the wall never consolidated");

    // The left half (no balls) must end up closed.
    let g = &st.arena.grid;
    let mut left_open = 0;
    for y in 0..20u16 {
        for x in 0..20u16 {
            if g.get(x, y) == Cell::Open {
                left_open += 1;
            }
        }
    }
    assert_eq!(
        left_open, 0,
        "the left half did not close: {left_open} cells are still open"
    );

    // The right half (with the balls) must stay mostly open.
    let mut right_open = 0;
    for y in 0..20u16 {
        for x in 21..40u16 {
            if g.get(x, y) == Cell::Open {
                right_open += 1;
            }
        }
    }
    assert!(
        right_open > 200,
        "the right half closed with balls inside: only {right_open} open"
    );
}

/// No ball may end up inside a closed cell. This is the most important
/// invariant of the closing logic: if it breaks, the ball gets buried.
#[test]
fn no_ball_ends_up_inside_a_closed_cell() {
    let spec = level_spec(
        40,
        24,
        vec![
            ball(8.0, 6.0, 7.0, 7.0),
            ball(30.0, 16.0, -7.0, 7.0),
            ball(20.0, 12.0, 7.0, -7.0),
        ],
        false,
    );
    let mut st = GameState::new(spec);

    // We draw walls repeatedly at varied positions.
    let mut frame = 0usize;
    for round in 0..40 {
        let cell = ((5 + (round * 3) % 30) as u16, (4 + (round * 5) % 18) as u16);
        let axis = if round % 2 == 0 {
            WallAxis::Vertical
        } else {
            WallAxis::Horizontal
        };
        let (next, _) = step(&st, PlayerInput::StartWall { cell, axis }, DT);
        st = next;

        for _ in 0..300 {
            let (next, _) = step(&st, PlayerInput::None, DT);
            st = next;
            frame += 1;

            // INVARIANT: each ball's cell can never be closed.
            for b in &st.balls {
                let (cx, cy) = (b.pos.x.floor() as i64, b.pos.y.floor() as i64);
                if st.arena.grid.in_bounds(cx, cy) {
                    let c = st.arena.grid.get(cx as u16, cy as u16);
                    assert_ne!(
                        c,
                        Cell::Filled,
                        "frame {frame}: ball buried in a Filled cell ({cx}, {cy})"
                    );
                    assert_ne!(
                        c,
                        Cell::Solid,
                        "frame {frame}: ball buried in a Solid cell ({cx}, {cy})"
                    );
                }
            }
            if st.builders.is_empty() {
                break;
            }
        }
    }
}

/// The conquered-area percentage must never go backwards nor exceed 1.0.
/// It is what the HUD shows: if it oscillates, the player sees nonsense.
#[test]
fn the_conquered_area_is_monotonic_and_valid() {
    let spec = level_spec(
        36,
        20,
        vec![ball(10.0, 6.0, 7.0, 7.0), ball(26.0, 14.0, -7.0, 7.0)],
        false,
    );
    let mut st = GameState::new(spec);
    let mut previous = st.arena.grid.filled_ratio();

    for round in 0..25 {
        let cell = ((4 + (round * 4) % 28) as u16, (3 + (round * 3) % 15) as u16);
        let axis = if round % 2 == 0 {
            WallAxis::Horizontal
        } else {
            WallAxis::Vertical
        };
        let (next, _) = step(&st, PlayerInput::StartWall { cell, axis }, DT);
        st = next;

        for _ in 0..300 {
            let (next, _) = step(&st, PlayerInput::None, DT);
            st = next;
            let r = st.arena.grid.filled_ratio();
            assert!(
                r.is_finite() && (0.0..=1.0).contains(&r),
                "filled_ratio out of range: {r}"
            );
            assert!(
                r >= previous - 1e-4,
                "the conquered area WENT BACKWARDS: {previous} -> {r}"
            );
            previous = r;
            if st.builders.is_empty() {
                break;
            }
        }
    }
}
