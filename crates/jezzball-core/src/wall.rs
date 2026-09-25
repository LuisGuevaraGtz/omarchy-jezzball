//! Walls (ARCHITECTURE.md §3).
//!
//! A `WallBuilder` is the wall UNDER CONSTRUCTION: it grows in BOTH
//! directions from its origin cell at the same time (one front towards
//! decreasing coordinates `lo`, the other towards increasing ones `hi`), at
//! `speed` cells per second and per front. Each front stops independently
//! when it touches `Filled`/`Solid`/`NoSplit` or the border. Only when
//! `lo_done && hi_done` is it ready to consolidate.

use crate::grid::Grid;

/// Growth axis of a wall.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WallAxis {
    Horizontal, // grows along the columns (same row as the origin)
    Vertical,   // grows along the rows (same column as the origin)
}

impl WallAxis {
    /// Maximum coordinate (inclusive) a front on this axis can reach.
    pub fn max_coord(self, w: u16, h: u16) -> f32 {
        match self {
            WallAxis::Horizontal => (w - 1) as f32,
            WallAxis::Vertical => (h - 1) as f32,
        }
    }
}

/// Wall the player is currently drawing.
///
/// Rule from the original JezzBall: each half "sets" (becomes fixed) as soon
/// as its front reaches a wall or obstacle, and from that moment it is
/// immune to balls. Only the half that is still growing can cost a life.
/// `lo_sealed`/`hi_sealed` record which halves have already set.
#[derive(Clone, Debug, PartialEq)]
pub struct WallBuilder {
    pub axis: WallAxis,
    pub origin: (u16, u16), // cell where the player clicked
    pub lo: f32,            // front growing towards -X/-Y (in cells)
    pub hi: f32,            // front growing towards +X/+Y
    pub lo_done: bool,      // reached a border/obstacle
    pub hi_done: bool,
    /// The `lo` half has already been committed to the grid as `Filled`
    /// (it is immune).
    pub lo_sealed: bool,
    /// The `hi` half has already been committed to the grid as `Filled`
    /// (it is immune).
    pub hi_sealed: bool,
    pub speed: f32,     // cells per second, per front
    pub shielded: bool, // Shield power-up: absorbs 1 impact
}

/// Small tolerance to "press" a front against the blocking cell without
/// covering it: if the front stops at cell `c`, we leave it at `c - EPS`.
const EPS: f32 = 1e-3;

impl WallBuilder {
    /// Creates a wall starting at its origin (a single front with
    /// `lo == hi`).
    pub fn new(axis: WallAxis, origin: (u16, u16), speed: f32, shielded: bool) -> Self {
        let coord = match axis {
            WallAxis::Horizontal => origin.0 as f32,
            WallAxis::Vertical => origin.1 as f32,
        };
        WallBuilder {
            axis,
            origin,
            lo: coord,
            hi: coord,
            lo_done: false,
            hi_done: false,
            lo_sealed: false,
            hi_sealed: false,
            speed,
            shielded,
        }
    }

    /// Cells currently covered by the segment `[ceil(lo), floor(hi)]`.
    pub fn cells(&self) -> Vec<(u16, u16)> {
        self.cells_between(self.lo.ceil() as i64, self.hi.floor() as i64)
    }

    /// Cells of the segment between two coordinates of the growth axis.
    fn cells_between(&self, start: i64, end: i64) -> Vec<(u16, u16)> {
        let mut out = Vec::new();
        for i in start..=end {
            if i < 0 {
                continue;
            }
            let cell = match self.axis {
                WallAxis::Horizontal => (i as u16, self.origin.1),
                WallAxis::Vertical => (self.origin.0, i as u16),
            };
            out.push(cell);
        }
        out
    }

    /// Coordinate of the origin along the growth axis.
    fn origin_coord(&self) -> i64 {
        match self.axis {
            WallAxis::Horizontal => self.origin.0 as i64,
            WallAxis::Vertical => self.origin.1 as i64,
        }
    }

    /// Cells of the `lo` half: from the front up to the origin (excluded).
    pub fn lo_cells(&self) -> Vec<(u16, u16)> {
        self.cells_between(self.lo.ceil() as i64, self.origin_coord() - 1)
    }

    /// Cells of the `hi` half: from the origin (included) up to the front.
    /// The origin is assigned to this half so that no cell is left orphaned.
    pub fn hi_cells(&self) -> Vec<(u16, u16)> {
        self.cells_between(self.origin_coord(), self.hi.floor() as i64)
    }

    /// Cells a ball CAN destroy: only those of the halves that are still
    /// growing. A half that has already been sealed behaves like a normal
    /// wall.
    ///
    /// We do not filter by grid state: the cells of a live half have never
    /// been committed to it, and filtering by `is_open` would make the ones a
    /// `Mover` happens to be standing on at that instant disappear (they are
    /// temporarily marked `Solid`), and those are precisely the ones that
    /// must detect the collision.
    pub fn vulnerable_cells(&self, _grid: &Grid) -> Vec<(u16, u16)> {
        let mut out = Vec::new();
        if !self.lo_sealed {
            out.extend(self.lo_cells());
        }
        if !self.hi_sealed {
            out.extend(self.hi_cells());
        }
        out
    }

    /// Is there any live (unsealed) half left? If not, the wall is already
    /// fully in the grid and cannot cost lives.
    pub fn has_live_half(&self) -> bool {
        !self.lo_sealed || !self.hi_sealed
    }

    /// Have both fronts reached their limit? Only then does it consolidate.
    pub fn is_done(&self) -> bool {
        self.lo_done && self.hi_done
    }

    /// Advances both fronts by `dt` seconds. A front that has already
    /// finished does not move. This advance is pure against the current grid.
    pub fn advance(&mut self, grid: &Grid, dt: f32) {
        let max_coord = self.axis.max_coord(grid.w, grid.h);

        if !self.hi_done {
            let new_hi = self.hi + self.speed * dt;
            // We sweep ALL the cells the front steps on during this step, not
            // just the destination one: with a high `speed` (up to 30
            // cells/s) and a long frame the front advances several cells at
            // once, and looking only at the destination let it jump over
            // intervening walls and obstacles.
            let old_end = self.hi.floor() as i64;
            let new_end = new_hi.floor() as i64;
            let mut blocked: Option<i64> = None;
            for coord in (old_end + 1)..=new_end {
                if !self.front_open(grid, coord) {
                    blocked = Some(coord);
                    break;
                }
            }
            if let Some(cell) = blocked {
                // Pressed against the blocking cell (without covering it).
                self.hi = cell as f32 - EPS;
                self.hi_done = true;
            } else if new_end >= max_coord as i64 {
                // Reached the border: complete the last stretch and finish.
                self.hi = max_coord;
                self.hi_done = true;
            } else {
                self.hi = new_hi;
            }
        }

        if !self.lo_done {
            let new_lo = self.lo - self.speed * dt;
            // Cells just stepped on by the lo front: those in the interval
            // [ceil(new_lo), ceil(self.lo)). They are ALWAYS checked, even
            // when reaching the border: if there is an obstacle along the
            // way, the front must stop there and not at cell 0.
            let old_start = self.lo.ceil() as i64;
            let new_start = new_lo.ceil().max(0.0) as i64;
            let mut blocked: Option<i64> = None;
            for coord in (new_start..old_start).rev() {
                if !self.front_open(grid, coord) {
                    blocked = Some(coord);
                    break;
                }
            }
            if let Some(cell) = blocked {
                // Front resting to the right of `cell`: the coverage starts
                // at `cell + 1`, which does not cover the blocking cell.
                self.lo = (cell + 1) as f32;
                self.lo_done = true;
            } else if new_lo <= 0.0 {
                // Reached the left/top border without finding anything.
                self.lo = 0.0;
                self.lo_done = true;
            } else {
                self.lo = new_lo;
            }
        }
    }

    /// Does the front at coordinate `coord` have an open cell to grow into?
    fn front_open(&self, grid: &Grid, coord: i64) -> bool {
        match self.axis {
            WallAxis::Horizontal => grid.is_wall_open(coord, self.origin.1 as i64),
            WallAxis::Vertical => grid.is_wall_open(self.origin.0 as i64, coord),
        }
    }
}

/// Already consolidated wall (historical record used by the render/audio
/// layer).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wall {
    pub axis: WallAxis,
    pub origin: (u16, u16),
    pub cells: u32,
    pub points: u32,
}
