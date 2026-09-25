//! Reproduction of the "as if it were a corner" bounce on a FLAT wall.
//!
//! Symptom reported while playing: the ball hits the middle of a wall (not a
//! corner) and goes back exactly the way it came, instead of reflecting.
//! After several bounces like that, it enters a closed cycle.
//!
//! A correct bounce off a vertical wall flips ONLY `vx`; off a horizontal one,
//! ONLY `vy`. Flipping both is the corner bounce, and it should only happen
//! when the ball really reaches a corner.

use jezzball_core::ball::Ball;
use jezzball_core::grid::{Cell, Grid};
use jezzball_core::level::BallKind;
use jezzball_core::rng::Rng64;

const DT: f32 = 1.0 / 60.0;

/// 20x20 arena with implicit borders (outside the grid is not traversable).
fn arena() -> Grid {
    Grid::new(20, 20)
}

/// Advances until the first sign change on either axis is detected and returns
/// `(change_in_x, change_in_y)` for THAT specific frame.
fn first_bounce(b: &mut Ball, g: &Grid, max_frames: usize) -> (bool, bool) {
    let mut rng = Rng64::new(1);
    for _ in 0..max_frames {
        let vx_before = b.vel.x;
        let vy_before = b.vel.y;
        b.step(g, &mut rng, DT);
        let changed_x = vx_before.signum() != b.vel.x.signum();
        let changed_y = vy_before.signum() != b.vel.y.signum();
        if changed_x || changed_y {
            return (changed_x, changed_y);
        }
    }
    panic!("the ball did not bounce within {max_frames} frames");
}

/// RIGHT WALL (vertical): must flip only `vx`.
#[test]
fn right_vertical_wall_flips_only_vx() {
    let g = arena();
    // Pure diagonal towards the right and down, away from the corners.
    let mut b = Ball::new(0, 15.0, 10.0, 8.0, 8.0, BallKind::Normal, 1.0);
    let (cx, cy) = first_bounce(&mut b, &g, 600);
    assert!(cx, "it did not flip vx when hitting the right wall");
    assert!(
        !cy,
        "IT ALSO FLIPPED vy: corner-style bounce on a flat wall \
         (the ball goes back the way it came)"
    );
}

/// LEFT WALL.
#[test]
fn left_vertical_wall_flips_only_vx() {
    let g = arena();
    let mut b = Ball::new(0, 5.0, 10.0, -8.0, 8.0, BallKind::Normal, 1.0);
    let (cx, cy) = first_bounce(&mut b, &g, 600);
    assert!(cx, "it did not flip vx when hitting the left wall");
    assert!(
        !cy,
        "IT ALSO FLIPPED vy: corner-style bounce on a flat wall"
    );
}

/// TOP WALL (horizontal): must flip only `vy`.
#[test]
fn top_horizontal_wall_flips_only_vy() {
    let g = arena();
    let mut b = Ball::new(0, 10.0, 5.0, 8.0, -8.0, BallKind::Normal, 1.0);
    let (cx, cy) = first_bounce(&mut b, &g, 600);
    assert!(cy, "it did not flip vy when hitting the top wall");
    assert!(
        !cx,
        "IT ALSO FLIPPED vx: corner-style bounce on a flat wall"
    );
}

/// BOTTOM WALL.
#[test]
fn bottom_horizontal_wall_flips_only_vy() {
    let g = arena();
    let mut b = Ball::new(0, 10.0, 15.0, 8.0, 8.0, BallKind::Normal, 1.0);
    let (cx, cy) = first_bounce(&mut b, &g, 600);
    assert!(cy, "it did not flip vy when hitting the bottom wall");
    assert!(
        !cx,
        "IT ALSO FLIPPED vx: corner-style bounce on a flat wall"
    );
}

/// Consolidated INNER wall (not the arena border): same criterion.
#[test]
fn inner_vertical_wall_flips_only_vx() {
    let mut g = arena();
    for y in 0..20 {
        g.set(14, y, Cell::Filled);
    }
    let mut b = Ball::new(0, 10.0, 6.0, 8.0, 8.0, BallKind::Normal, 1.0);
    let (cx, cy) = first_bounce(&mut b, &g, 600);
    assert!(cx, "it did not flip vx when hitting the inner wall");
    assert!(!cy, "IT ALSO FLIPPED vy against a flat inner wall");
}

/// The ball must NOT cycle into a poor trajectory given a normal start.
///
/// Important nuance: an EXACT 45° diagonal launched from the centre of a
/// square box always traces the same line. That is geometry, not a bug:
/// reflecting a diagonal off orthogonal walls yields another diagonal, and
/// with that perfect symmetry the orbit closes. That is why the levels do not
/// place the balls at symmetric positions (see `tools/gen_levels.py`).
///
/// What WOULD be a bug is the ball folding back on its own steps from an
/// arbitrary position, which is what happened when a bounce off a flat wall
/// flipped both components.
#[test]
fn the_trajectory_does_not_loop_back_on_itself() {
    let g = arena();
    let mut rng = Rng64::new(5);
    // Non-symmetric starting point, like the ones in the real levels.
    let mut b = Ball::new(0, 7.3, 11.8, 9.0, 9.0, BallKind::Normal, 1.0);

    let mut visited = std::collections::HashSet::new();
    for _ in 0..1800 {
        b.step(&g, &mut rng, DT);
        visited.insert((b.pos.x.floor() as i32, b.pos.y.floor() as i32));
    }

    assert!(
        visited.len() > 40,
        "the ball cycled: it only visited {} distinct cells in 30 seconds",
        visited.len()
    );
}

/// With a pure diagonal, each bounce must change only ONE component.
/// This test runs through a long game and verifies that both are NEVER
/// flipped at once unless the ball is really in a corner.
#[test]
fn never_flips_both_axes_outside_a_corner() {
    let g = arena();
    let mut rng = Rng64::new(11);
    let mut b = Ball::new(0, 6.7, 13.2, 8.0, 8.0, BallKind::Normal, 1.0);
    let r = b.radius;

    for frame in 0..3000 {
        let vx0 = b.vel.x;
        let vy0 = b.vel.y;
        b.step(&g, &mut rng, DT);
        let cx = vx0.signum() != b.vel.x.signum();
        let cy = vy0.signum() != b.vel.y.signum();
        if cx && cy {
            // Only acceptable if it touches two walls at once (a real corner).
            let on_wall_x = b.pos.x - r <= 0.05 || b.pos.x + r >= 20.0 - 0.05;
            let on_wall_y = b.pos.y - r <= 0.05 || b.pos.y + r >= 20.0 - 0.05;
            assert!(
                on_wall_x && on_wall_y,
                "frame {frame}: it flipped BOTH axes without being in a corner \
                 (pos = {:.3}, {:.3})",
                b.pos.x,
                b.pos.y
            );
        }
    }
}
