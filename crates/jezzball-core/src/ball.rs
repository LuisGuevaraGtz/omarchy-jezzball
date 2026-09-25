//! Continuous balls (ARCHITECTURE.md §2 and §8).
//!
//! Balls move in `f32` cell coordinates. Movement uses semi-implicit
//! integration with collision resolution ON SEPARATE AXES against the grid:
//! move on X, bounce if the destination cell is not `Open`; move on Y, the
//! same. That produces the classic 45-degree JezzBall bounce without
//! tunnelling, and with no tunnels as long as `|vel| * dt < 0.5` cells. When
//! `|vel| * dt >= 0.5`, internal substeps are applied (max. 8).

use crate::geom::Vec2;
use crate::grid::Grid;
use crate::level::BallKind;
use crate::rng::Rng64;

/// How many seconds pass between each rotation of an `Erratic` ball's
/// velocity by +/-30 degrees.
const ERRATIC_INTERVAL: f32 = 1.5;
/// +/-30 degrees in radians.
const ERRATIC_ANGLE: f32 = std::f32::consts::PI / 6.0;

/// Maximum distance (cells) per substep with no risk of tunnelling.
const MAX_STEP_DISTANCE: f32 = 0.5;
/// Cap of substeps per frame.
const MAX_SUBSTEPS: usize = 8;
/// Slack for cell boundary comparisons: after a bounce the ball rests
/// EXACTLY on a boundary, and without this tolerance floating-point error
/// decides at random whether it keeps colliding or not.
const EPS: f32 = 1e-4;

#[derive(Clone, Debug, PartialEq)]
pub struct Ball {
    pub id: u32,
    pub pos: Vec2,
    pub vel: Vec2,
    pub kind: BallKind,
    pub radius: f32,
    /// Previous position: lets us use a swept AABB to detect impacts against
    /// walls under construction without tunnelling between frames.
    pub prev: Vec2,
    erratic_timer: f32,
}

impl Ball {
    /// Creates a ball from a `BallSpawn`. The velocity and the radius depend
    /// on the kind (`BallKind::speed_mult` / `base_radius`).
    pub fn new(id: u32, x: f32, y: f32, vx: f32, vy: f32, kind: BallKind, radius_mul: f32) -> Self {
        let pos = Vec2::new(x, y);
        let vel = Vec2::new(vx, vy) * kind.speed_mult();
        Ball {
            id,
            pos,
            vel,
            kind,
            radius: kind.base_radius() * radius_mul,
            prev: pos,
            erratic_timer: 0.0,
        }
    }

    pub fn speed(&self) -> f32 {
        self.vel.length()
    }

    /// Integrates the ball by one `dt` step.
    ///
    /// - If the kind is `Erratic`, every `ERRATIC_INTERVAL` seconds it
    ///   rotates its velocity by +/-30 degrees using the core's
    ///   deterministic PRNG.
    /// - If `|vel| * dt >= 0.5` it splits the step into substeps so it does
    ///   not go through cells by overshooting.
    pub fn step(&mut self, grid: &Grid, rng: &mut Rng64, dt: f32) {
        if dt <= 0.0 {
            return;
        }

        // Snapshot of the previous position for the swept AABB: `state` uses
        // `prev`/`pos` to detect impacts against walls under construction
        // without tunnelling between frames.
        self.prev = self.pos;

        if self.kind == BallKind::Erratic {
            self.erratic_timer += dt;
            while self.erratic_timer >= ERRATIC_INTERVAL {
                self.erratic_timer -= ERRATIC_INTERVAL;
                let angle = if rng.next_f32() < 0.5 {
                    -ERRATIC_ANGLE
                } else {
                    ERRATIC_ANGLE
                };
                self.vel = self.vel.rotate(angle);
            }
        }

        let distance = self.speed() * dt;
        let substeps = if distance >= MAX_STEP_DISTANCE {
            ((distance / MAX_STEP_DISTANCE).ceil() as usize).clamp(1, MAX_SUBSTEPS)
        } else {
            1
        };
        let substep_dt = dt / substeps as f32;

        for _ in 0..substeps {
            self.move_axis(grid, true, substep_dt);
            self.move_axis(grid, false, substep_dt);
        }
    }

    /// Resolves the movement on one axis with axis separation.
    /// `is_x = true` moves (and bounces) the X axis.
    ///
    /// Only blockers that are IN FRONT OF the ball's current leading edge are
    /// considered. That is the difference between bouncing and getting stuck:
    /// after a bounce the ball rests right against the cell (its edge touches
    /// the boundary), and if the next sweep looked at that very same cell it
    /// would treat it as blocking again, inverting the velocity every frame
    /// (jitter) or pushing the ball to the other side (going through the
    /// wall). The same happens when a wall consolidates BEHIND the ball: it
    /// must not push it nor reverse its direction, only stop whatever is
    /// ahead of it.
    fn move_axis(&mut self, grid: &Grid, is_x: bool, dt: f32) {
        let velocity = if is_x { self.vel.x } else { self.vel.y };
        if velocity == 0.0 {
            return;
        }

        let current = if is_x { self.pos.x } else { self.pos.y };
        let along = current + velocity * dt;

        // Leading edge: the side of the ball that advances, before and after.
        // Any blocker that is not strictly in front of the current edge is
        // ignored (we have already left it behind, or we are resting on it).
        let (lead_now, lead_next) = if velocity > 0.0 {
            (current + self.radius, along + self.radius)
        } else {
            (current - self.radius, along - self.radius)
        };

        // Transverse range swept by the ball (the perpendicular axis).
        //
        // It is shrunk by EPS at both ends on purpose. After bouncing, the
        // ball rests with its edge EXACTLY on a cell boundary (e.g.
        // x + radius == 20.0 in an arena 20 wide). Without this shrinking,
        // `floor()` of that edge returns the cell beyond the wall — outside
        // the grid, or the wall itself — and the PERPENDICULAR axis reads it
        // as a blocker: the ball bounces on the other axis too and shoots
        // back the way it came, as if it had hit a corner while being in the
        // middle of a flat wall. By shrinking the range, only the cells the
        // ball REALLY overlaps are considered, not the ones it merely touches
        // tangentially.
        let (c0, c1) = if is_x {
            (
                self.pos.y - self.radius + EPS,
                self.pos.y + self.radius - EPS,
            )
        } else {
            (
                self.pos.x - self.radius + EPS,
                self.pos.x + self.radius - EPS,
            )
        };
        let cell_c0 = c0.floor() as i64;
        let cell_c1 = c1.floor() as i64;

        // Cells that the leading edge crosses in this step.
        let mut blocked: Option<i64> = None;
        if velocity > 0.0 {
            // `lead_now` may sit exactly on a cell boundary (ball resting
            // after a bounce): we start at the next cell.
            let first = lead_now.floor() as i64;
            let last = lead_next.floor() as i64;
            'outer: for a in first..=last {
                // Ignore the cell we are already resting on.
                if (a as f32) < lead_now - EPS {
                    continue;
                }
                for c in cell_c0..=cell_c1 {
                    let (cx, cy) = if is_x { (a, c) } else { (c, a) };
                    if !grid.is_passable(cx, cy) {
                        blocked = Some(a);
                        break 'outer;
                    }
                }
            }
        } else {
            let first = lead_now.floor() as i64;
            let last = lead_next.floor() as i64;
            'outer: for a in (last..=first).rev() {
                // Ignore the cell we are already resting on.
                if ((a + 1) as f32) > lead_now + EPS {
                    continue;
                }
                for c in cell_c0..=cell_c1 {
                    let (cx, cy) = if is_x { (a, c) } else { (c, a) };
                    if !grid.is_passable(cx, cy) {
                        blocked = Some(a);
                        break 'outer;
                    }
                }
            }
        }

        let mut result = along;
        if let Some(cell) = blocked {
            // Rest the ball's edge against the blocking cell.
            let limit = if velocity > 0.0 {
                cell as f32 - self.radius
            } else {
                (cell + 1) as f32 + self.radius
            };
            result = limit;
            if is_x {
                self.vel.x = -self.vel.x;
            } else {
                self.vel.y = -self.vel.y;
            }
        }

        if is_x {
            self.pos.x = result;
        } else {
            self.pos.y = result;
        }
    }
}
