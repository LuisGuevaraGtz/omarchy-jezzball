//! Rule from the original JezzBall: each half of the wall "converts" (gets
//! fixed) as soon as its front touches a wall, and from that moment on it is
//! IMMUNE. Only the half that is still growing can cost a life.
//!
//! Previously the wall was all-or-nothing: a ball touching any point destroyed
//! the whole line, including the part already anchored to the wall.

use jezzball_core::grid::Cell;
use jezzball_core::level::{ArenaShape, ArenaSpec, BallKind, BallSpawn, LevelKind, LevelSpec};
use jezzball_core::state::{step, GameEvent, GameState, PlayerInput};
use jezzball_core::wall::WallAxis;

const DT: f32 = 1.0 / 60.0;

fn spec(w: u16, h: u16, balls: Vec<BallSpawn>, wall_speed: f32) -> LevelSpec {
    LevelSpec {
        id: 1,
        name: "t".into(),
        world: 1,
        kind: LevelKind::Standard,
        arena: ArenaSpec {
            w,
            h,
            shape: ArenaShape::Rect,
            obstacles: vec![],
        },
        balls,
        target_ratio: 0.9,
        lives: 3,
        wall_speed,
        time_limit: None,
        powerups_enabled: false,
        objectives: vec![],
        purist: false,
        seed: 3,
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

/// A still ball (minimal velocity) placed on the already fixed half must not
/// cost lives.
#[test]
fn the_half_anchored_to_the_wall_is_immune() {
    // Wide, low arena. VERTICAL wall from a row close to the top border: the
    // `lo` front reaches the top right away and gets fixed; the `hi` front
    // keeps descending for a long while.
    let st = GameState::new(spec(30, 40, vec![ball(25.0, 30.0, 0.5, 0.5)], 6.0));
    let (st, _) = step(
        &st,
        PlayerInput::StartWall {
            cell: (10, 2),
            axis: WallAxis::Vertical,
        },
        DT,
    );

    // We advance just enough for the `lo` front to touch the top border.
    let mut st = st;
    for _ in 0..60 {
        st = step(&st, PlayerInput::None, DT).0;
    }
    assert_eq!(st.builders.len(), 1, "the wall should still be growing");
    let b = &st.builders[0];
    assert!(
        b.lo_done,
        "the lo front should have touched the border by now"
    );
    assert!(!b.hi_done, "the hi front should still be growing");

    // The upper half (rows 0..=2 of column 10) must already be FIXED in the
    // grid, not merely "painted".
    for y in 0..2u16 {
        assert_eq!(
            st.arena.grid.get(10, y),
            Cell::Filled,
            "cell (10, {y}) of the anchored half should be fixed"
        );
    }

    // And it must not show up as vulnerable.
    let vulnerable = st.builders[0].vulnerable_cells(&st.arena.grid);
    for y in 0..2u16 {
        assert!(
            !vulnerable.contains(&(10, y)),
            "cell (10, {y}) is still vulnerable despite being anchored"
        );
    }
    assert!(
        !vulnerable.is_empty(),
        "the half that is still growing should be vulnerable"
    );
}

/// If the ball hits the half that is STILL GROWING, a life is lost but the
/// already anchored half stays in the arena.
#[test]
fn hitting_the_live_half_leaves_the_anchored_one_standing() {
    // Ball placed at the bottom, in the wall's column, travelling towards it.
    let st = GameState::new(spec(30, 40, vec![ball(10.5, 30.0, 0.0001, -9.0)], 6.0));
    let (st, _) = step(
        &st,
        PlayerInput::StartWall {
            cell: (10, 2),
            axis: WallAxis::Vertical,
        },
        DT,
    );

    let mut st = st;
    let mut life_lost = false;
    for _ in 0..900 {
        let (next, events) = step(&st, PlayerInput::None, DT);
        st = next;
        if events.iter().any(|e| matches!(e, GameEvent::LifeLost)) {
            life_lost = true;
            break;
        }
        if st.builders.is_empty() {
            break;
        }
    }

    assert!(
        life_lost,
        "the ball should have hit the live half and cost a life"
    );
    assert_eq!(st.lives, 2, "exactly one life less should remain");

    // THE KEY POINT: the upper half, which was already anchored to the wall,
    // is still there. Previously it was wiped along with the rest of the wall.
    for y in 0..2u16 {
        assert_eq!(
            st.arena.grid.get(10, y),
            Cell::Filled,
            "the anchored half (10, {y}) was lost when the other half was hit"
        );
    }
}

/// When both fronts reach their limit, the wall consolidates and splits the
/// area as always (we are not breaking the existing behaviour).
#[test]
fn with_both_halves_anchored_the_wall_splits_the_area() {
    // Both balls live on the right; the vertical wall on column 8 must close
    // the left strip.
    let st = GameState::new(spec(
        30,
        20,
        vec![ball(20.0, 8.0, 6.0, 6.0), ball(24.0, 12.0, -6.0, 6.0)],
        24.0,
    ));
    let (st, _) = step(
        &st,
        PlayerInput::StartWall {
            cell: (8, 10),
            axis: WallAxis::Vertical,
        },
        DT,
    );

    let mut st = st;
    let mut consolidated = false;
    for _ in 0..1200 {
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
    assert!(consolidated, "the wall did not consolidate");

    // The wall's entire column must be fixed.
    for y in 0..20u16 {
        assert_eq!(
            st.arena.grid.get(8, y),
            Cell::Filled,
            "the wall column is not complete at y={y}"
        );
    }
    // And the left strip (with no balls) must have been closed.
    let mut left_open = 0;
    for y in 0..20u16 {
        for x in 0..8u16 {
            if st.arena.grid.get(x, y) == Cell::Open {
                left_open += 1;
            }
        }
    }
    assert_eq!(
        left_open, 0,
        "the left strip did not close: {left_open} open cells"
    );
}

/// A horizontal wall behaves the same way (the rule does not depend on the axis).
#[test]
fn the_rule_holds_for_horizontal_walls() {
    let st = GameState::new(spec(40, 30, vec![ball(30.0, 25.0, 0.5, 0.5)], 6.0));
    let (st, _) = step(
        &st,
        PlayerInput::StartWall {
            cell: (2, 10),
            axis: WallAxis::Horizontal,
        },
        DT,
    );

    let mut st = st;
    for _ in 0..60 {
        st = step(&st, PlayerInput::None, DT).0;
    }
    assert_eq!(st.builders.len(), 1);
    assert!(
        st.builders[0].lo_done,
        "the lo front should have touched the left border"
    );
    for x in 0..2u16 {
        assert_eq!(
            st.arena.grid.get(x, 10),
            Cell::Filled,
            "cell ({x}, 10) of the anchored half should be fixed"
        );
    }
}
