//! Reproduction of the physics failures reported while playing.
//!
//! These tests were written BEFORE the fix, to demonstrate the bug.

use jezzball_core::ball::Ball;
use jezzball_core::grid::{Cell, Grid};
use jezzball_core::level::BallKind;
use jezzball_core::rng::Rng64;

/// Open 10x10 arena.
fn arena() -> Grid {
    Grid::new(10, 10)
}

/// BUG 1 — After bouncing off a wall, the ball ends up resting exactly at
/// `cell - radius`, so the edge of its AABB falls RIGHT on the blocking cell.
/// On the next frame, already travelling in the opposite direction, the sweep
/// "sees" that same cell again and treats it as a block, which teleports it
/// TO THE OTHER SIDE of the wall and flips the velocity once more.
///
/// On-screen symptom: balls stick, jitter, or pass through walls.
#[test]
fn bouncing_neither_teleports_nor_crosses_the_wall() {
    let mut g = arena();
    // Vertical wall on column 5.
    for y in 0..10 {
        g.set(5, y, Cell::Filled);
    }
    let mut rng = Rng64::new(1);

    // Ball moving towards +X, to the left of the wall.
    let mut b = Ball::new(0, 4.0, 4.5, 6.0, 0.0, BallKind::Normal, 1.0);
    let radius = b.radius;

    // We simulate 120 frames at 60 FPS. The ball must NEVER cross column 5.
    for frame in 0..120 {
        b.step(&g, &mut rng, 1.0 / 60.0);
        assert!(
            b.pos.x + radius <= 5.0 + 1e-3,
            "frame {frame}: the ball crossed the wall (x={}, radius={radius})",
            b.pos.x
        );
        assert!(
            b.pos.x - radius >= -1e-3,
            "frame {frame}: the ball left through the left edge (x={})",
            b.pos.x
        );
    }
}

/// BUG 1 (variant) — The bounce must preserve SPEED. If the ball ends up
/// jittering against the wall, the velocity is flipped several times per
/// frame and the motion stops being that of classic JezzBall.
#[test]
fn bouncing_preserves_speed_and_actually_advances() {
    let mut g = arena();
    for y in 0..10 {
        g.set(5, y, Cell::Filled);
    }
    let mut rng = Rng64::new(1);
    let mut b = Ball::new(0, 4.0, 4.5, 6.0, 0.0, BallKind::Normal, 1.0);
    let speed0 = b.speed();

    let mut min_x = f32::MAX;
    for _ in 0..240 {
        b.step(&g, &mut rng, 1.0 / 60.0);
        min_x = min_x.min(b.pos.x);
        assert!(
            (b.speed() - speed0).abs() < 1e-3,
            "the speed changed: {} -> {}",
            speed0,
            b.speed()
        );
    }
    // After bouncing it must have travelled back to the left,
    // not stayed stuck to the wall.
    assert!(
        min_x < 3.0,
        "the ball stayed stuck to the wall (min_x={min_x}); it should have bounced and travelled"
    );
}

/// BUG 2 — A cell that gets filled BEHIND the ball (the normal case: the wall
/// consolidates right where the ball has just passed) must not push it nor
/// flip its velocity: it only blocks what lies ahead.
#[test]
fn filled_cell_behind_does_not_push_the_ball() {
    let mut g = arena();
    let mut rng = Rng64::new(1);
    // Ball at the centre of cell 4, travelling towards +X.
    let mut b = Ball::new(0, 4.5, 4.5, 6.0, 0.0, BallKind::Normal, 1.0);
    b.step(&g, &mut rng, 1.0 / 60.0);
    let vel_before = b.vel.x;
    let x_before = b.pos.x;

    // Now the cell BEHIND the ball gets filled.
    g.set(4, 4, Cell::Filled);
    b.step(&g, &mut rng, 1.0 / 60.0);

    assert!(
        b.vel.x > 0.0,
        "the ball flipped its velocity because of a cell behind it ({vel_before} -> {})",
        b.vel.x
    );
    assert!(
        b.pos.x >= x_before,
        "the ball moved back because of a cell behind it ({x_before} -> {})",
        b.pos.x
    );
}

/// BUG 3 — In a corner, the ball bounces on both axes and must stay inside
/// the arena, without getting trapped or being flung out.
#[test]
fn corner_bounce_keeps_the_ball_inside() {
    let g = arena();
    let mut rng = Rng64::new(7);
    // Towards the top-left corner.
    let mut b = Ball::new(0, 1.0, 1.0, -9.0, -9.0, BallKind::Normal, 1.0);
    let r = b.radius;
    for frame in 0..300 {
        b.step(&g, &mut rng, 1.0 / 60.0);
        assert!(
            b.pos.x - r >= -1e-3 && b.pos.y - r >= -1e-3,
            "frame {frame}: the ball left through the corner ({}, {})",
            b.pos.x,
            b.pos.y
        );
        assert!(
            b.pos.x + r <= 10.0 + 1e-3 && b.pos.y + r <= 10.0 + 1e-3,
            "frame {frame}: the ball left through the opposite side ({}, {})",
            b.pos.x,
            b.pos.y
        );
    }
}

/// BUG 4 — A ball boxed into a single-cell corridor must bounce cleanly from
/// side to side, without jittering or escaping.
#[test]
fn narrow_corridor_bounces_cleanly() {
    let mut g = arena();
    // Horizontal corridor one cell high on row 4, between x=1 and x=8.
    for x in 0..10 {
        for y in 0..10 {
            if y != 4 {
                g.set(x, y, Cell::Filled);
            }
        }
    }
    g.set(0, 4, Cell::Filled);
    g.set(9, 4, Cell::Filled);

    let mut rng = Rng64::new(3);
    let mut b = Ball::new(0, 4.5, 4.5, 7.0, 0.0, BallKind::Normal, 1.0);
    let r = b.radius;
    for frame in 0..600 {
        b.step(&g, &mut rng, 1.0 / 60.0);
        assert!(
            b.pos.y - r >= 4.0 - 1e-3 && b.pos.y + r <= 5.0 + 1e-3,
            "frame {frame}: the ball left the corridor (y={})",
            b.pos.y
        );
        assert!(
            b.pos.x - r >= 1.0 - 1e-3 && b.pos.x + r <= 9.0 + 1e-3,
            "frame {frame}: the ball left through the end caps (x={})",
            b.pos.x
        );
    }
}
