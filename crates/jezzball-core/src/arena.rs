//! The arena: grid materialized from an `ArenaSpec`.
//!
//! `ArenaShape::Circle` and `ArenaShape::Maze` are materialized INTO `Solid`
//! CELLS right here, in the constructor, using the level's seed for the maze.
//! The rest of the engine knows nothing about the shape: it only works with
//! `grid`. `Obstacle::Block`s are static `Solid`s, the `NoSplit`s are marked
//! on the grid and the `Mover`s travel around occupying `Solid` cells as they
//! pass through them (ARCHITECTURE.md §2 and LEVEL_SCHEMA.md).

use crate::grid::{Cell, Grid};
use crate::level::{ArenaShape, ArenaSpec, Obstacle};
use crate::rng::Rng64;

/// Moving obstacle (`Obstacle::Mover`): it travels in a straight line,
/// bounces off the arena borders and occupies `Solid` cells as it goes
/// through them. If it brushes a wall under construction it destroys it with
/// NO cost in lives (documented design decision: losing lives is exclusive to
/// ball impacts, ARCHITECTURE.md §3).
#[derive(Clone, Debug, PartialEq)]
pub struct Mover {
    pub x: f32,
    pub y: f32,
    pub w: u16,
    pub h: u16,
    pub vx: f32,
    pub vy: f32,
    last_cells: Vec<(u16, u16)>,
}

/// Arena ready to simulate. `grid` is the source of truth for occupancy.
#[derive(Clone, Debug, PartialEq)]
pub struct Arena {
    pub grid: Grid,
    pub shape: ArenaShape,
    movers: Vec<Mover>,
    /// Cells that are `Solid` because of the shape or because of a `Block`:
    /// they are never restored to `Open` when a `Mover` passes by.
    static_solid: Vec<(u16, u16)>,
}

impl Arena {
    /// Materializes the shape and the obstacles of an `ArenaSpec`.
    pub fn from_spec(spec: &ArenaSpec, seed: u64) -> Arena {
        let mut grid = Grid::new(spec.w, spec.h);
        let mut static_solid = Vec::new();

        match spec.shape {
            ArenaShape::Rect | ArenaShape::Wide | ArenaShape::Tall => {}
            ArenaShape::Irregular { notch } => {
                materialize_notches(&mut grid, &mut static_solid, spec.w, spec.h, notch);
            }
            ArenaShape::Circle => {
                materialize_circle(&mut grid, &mut static_solid, spec.w, spec.h);
            }
            ArenaShape::Maze { density } => {
                materialize_maze(&mut grid, &mut static_solid, spec.w, spec.h, density, seed);
            }
        }

        let mut movers = Vec::new();
        for obstacle in &spec.obstacles {
            match *obstacle {
                Obstacle::Block { x, y, w, h } => {
                    fill_solid(&mut grid, &mut static_solid, x, y, w, h);
                }
                Obstacle::Mover { x, y, w, h, vx, vy } => {
                    movers.push(Mover {
                        x,
                        y,
                        w,
                        h,
                        vx,
                        vy,
                        last_cells: Vec::new(),
                    });
                }
                Obstacle::NoSplit { x, y, w, h } => {
                    mark_no_split(&mut grid, x, y, w, h);
                }
            }
        }

        Arena {
            grid,
            shape: spec.shape,
            movers,
            static_solid,
        }
    }

    /// Advances the `Mover`s, bounces them off the borders and rasterizes
    /// their cells to `Solid`. Returns the list of cells they occupy this
    /// frame (so that `state` can destroy walls under construction when they
    /// are brushed).
    pub fn update_movers(&mut self, dt: f32) -> Vec<(u16, u16)> {
        if self.movers.is_empty() {
            return Vec::new();
        }

        // 1) Advance and bounce; compute the cells they will occupy.
        let mut new_cells: Vec<Vec<(u16, u16)>> = Vec::with_capacity(self.movers.len());
        for m in &mut self.movers {
            m.x += m.vx * dt;
            m.y += m.vy * dt;
            let max_x = (self.grid.w as f32) - (m.w as f32);
            let max_y = (self.grid.h as f32) - (m.h as f32);
            if m.x < 0.0 {
                m.x = 0.0;
                m.vx = -m.vx;
            } else if m.x > max_x {
                m.x = max_x;
                m.vx = -m.vx;
            }
            if m.y < 0.0 {
                m.y = 0.0;
                m.vy = -m.vy;
            } else if m.y > max_y {
                m.y = max_y;
                m.vy = -m.vy;
            }
            new_cells.push(mover_cells(m, self.grid.w, self.grid.h));
        }

        // 2) Union of the new occupancy (so shared cells are not cleared).
        let mut occupied = Vec::new();
        for cells in &new_cells {
            occupied.extend(cells.iter().copied());
        }

        // 3) Restore to `Open` the old cells that are no longer occupied,
        //    except those belonging to the shape or to a static `Block`.
        let old_cells: Vec<Vec<(u16, u16)>> = self
            .movers
            .iter_mut()
            .map(|m| std::mem::take(&mut m.last_cells))
            .collect();
        for old in old_cells {
            for (cx, cy) in old {
                let still_occupied = occupied.contains(&(cx, cy));
                let static_solid = self.static_solid.contains(&(cx, cy));
                if !still_occupied && !static_solid && self.grid.get(cx, cy) == Cell::Solid {
                    self.grid.set(cx, cy, Cell::Open);
                }
            }
        }

        // 4) Paint the new occupancy as `Solid` (without touching static
        //    solids).
        for (cx, cy) in &occupied {
            if !self.static_solid.contains(&(*cx, *cy)) {
                self.grid.set(*cx, *cy, Cell::Solid);
            }
        }

        // 5) Remember the current cells per mover.
        for (i, m) in self.movers.iter_mut().enumerate() {
            m.last_cells = new_cells[i].clone();
        }

        occupied
    }

    /// Cells occupied right now by some `Mover`.
    pub fn mover_cells_now(&self) -> Vec<(u16, u16)> {
        let mut out = Vec::new();
        for m in &self.movers {
            out.extend(m.last_cells.iter().copied());
        }
        out
    }
}

/// Cells covered by the `Mover`'s rectangle, clipped to the arena.
fn mover_cells(m: &Mover, gw: u16, gh: u16) -> Vec<(u16, u16)> {
    let x0 = m.x.floor() as i64;
    let y0 = m.y.floor() as i64;
    let x1 = (m.x + m.w as f32 - 1.0).floor() as i64;
    let y1 = (m.y + m.h as f32 - 1.0).floor() as i64;
    let mut out = Vec::new();
    for cy in y0.max(0)..=y1.min(gh as i64 - 1) {
        for cx in x0.max(0)..=x1.min(gw as i64 - 1) {
            out.push((cx as u16, cy as u16));
        }
    }
    out
}

fn mark_solid(grid: &mut Grid, static_solid: &mut Vec<(u16, u16)>, x: u16, y: u16) {
    grid.set(x, y, Cell::Solid);
    static_solid.push((x, y));
}

fn fill_solid(grid: &mut Grid, static_solid: &mut Vec<(u16, u16)>, x: u16, y: u16, w: u16, h: u16) {
    for cy in y..y + h {
        for cx in x..x + w {
            if cx < grid.w && cy < grid.h {
                mark_solid(grid, static_solid, cx, cy);
            }
        }
    }
}

fn mark_no_split(grid: &mut Grid, x: u16, y: u16, w: u16, h: u16) {
    for cy in y..y + h {
        for cx in x..x + w {
            if cx < grid.w && cy < grid.h {
                grid.set_no_split(cx, cy);
            }
        }
    }
}

/// `Irregular { notch }`: cuts `notch` diagonal cells off each corner.
fn materialize_notches(
    grid: &mut Grid,
    static_solid: &mut Vec<(u16, u16)>,
    w: u16,
    h: u16,
    notch: u16,
) {
    let n = notch as i64;
    let w_max = (w - 1) as i64;
    let h_max = (h - 1) as i64;
    for y in 0..h {
        for x in 0..w {
            let (i, j) = (x as i64, y as i64);
            let tl = i + j < n;
            let tr = (w_max - i) + j < n;
            let bl = i + (h_max - j) < n;
            let br = (w_max - i) + (h_max - j) < n;
            if tl || tr || bl || br {
                mark_solid(grid, static_solid, x, y);
            }
        }
    }
}

/// `Circle`: inscribed ellipse; everything left outside is `Solid`.
fn materialize_circle(grid: &mut Grid, static_solid: &mut Vec<(u16, u16)>, w: u16, h: u16) {
    let cx = (w - 1) as f32 / 2.0;
    let cy = (h - 1) as f32 / 2.0;
    let rx = w as f32 / 2.0;
    let ry = h as f32 / 2.0;
    for y in 0..h {
        for x in 0..w {
            let nx = (x as f32 + 0.5 - cx) / rx;
            let ny = (y as f32 + 0.5 - cy) / ry;
            if nx * nx + ny * ny > 1.0 {
                mark_solid(grid, static_solid, x, y);
            }
        }
    }
}

/// `Maze { density }`: maze (DFS with backtracking) generated with the
/// deterministic PRNG seeded from `LevelSpec::seed`. `density` (0..=100)
/// opens extra passages: the higher it is, the more loops and the fewer dead
/// ends.
fn materialize_maze(
    grid: &mut Grid,
    static_solid: &mut Vec<(u16, u16)>,
    w: u16,
    h: u16,
    density: u8,
    seed: u64,
) {
    let wu = w as usize;
    let hu = h as usize;

    // Arenas smaller than 3 cells cannot hold a maze: they are left solid.
    if wu < 3 || hu < 3 {
        return;
    }

    // Every cell starts as `Solid`; the DFS goes on opening passages.
    for y in 0..hu {
        for x in 0..wu {
            grid.set(x as u16, y as u16, Cell::Solid);
            static_solid.push((x as u16, y as u16));
        }
    }

    // Maze vertices at odd coordinates: (2i+1, 2j+1). The number of vertices
    // per dimension is `w/2` (integer division): the bound
    // `(w+1).div_ceil(2)` produced an extra vertex in arenas of even
    // width/height whose centre `2i+1 >= w` wrote outside the grid (panic in
    // `Grid::set` when building the Enhanced Maze levels 52/55/58).
    let cols = wu / 2;
    let rows = hu / 2;
    let mut rng = Rng64::new(seed);
    let mut visited = vec![false; rows * cols];
    let mut stack = vec![(0usize, 0usize)];
    visited[0] = true;
    grid.set(1, 1, Cell::Open);

    while let Some((ci, cj)) = stack.pop() {
        let mut neighbours: Vec<(usize, usize)> = Vec::new();
        for (di, dj) in [(2i64, 0), (0, 2), (-2, 0), (0, -2)] {
            let (ni, nj) = (ci as i64 + di, cj as i64 + dj);
            if ni >= 0 && nj >= 0 && (ni as usize) < cols && (nj as usize) < rows {
                neighbours.push((ni as usize, nj as usize));
            }
        }
        shuffle(&mut neighbours, &mut rng);
        for (ni, nj) in neighbours {
            if !visited[nj * cols + ni] {
                visited[nj * cols + ni] = true;
                // Tear out the wall in between (midpoint between the two).
                let wall_x = (2 * ci + 1 + 2 * ni).div_ceil(2);
                let wall_y = (2 * cj + 1 + 2 * nj).div_ceil(2);
                grid.set(wall_x as u16, wall_y as u16, Cell::Open);
                grid.set((2 * ni + 1) as u16, (2 * nj + 1) as u16, Cell::Open);
                stack.push((ni, nj));
            }
        }
    }

    // The opened passages stop being static solids (so that a Mover passing
    // through can restore them).
    static_solid.retain(|&(x, y)| grid.get(x, y) == Cell::Solid);

    // Extra passages according to density: the more there are, the more
    // "open" the maze.
    let extra = (density as u32) * 3;
    for _ in 0..extra {
        let x = rng.range_i64(0, (wu - 1) as i64) as u16;
        let y = rng.range_i64(0, (hu - 1) as i64) as u16;
        grid.set(x, y, Cell::Open);
    }
    static_solid.retain(|&(x, y)| grid.get(x, y) == Cell::Solid);
}

/// Fisher-Yates with `rng` (deterministic).
fn shuffle<T>(items: &mut [T], rng: &mut Rng64) {
    for i in (1..items.len()).rev() {
        let j = rng.range_i64(0, i as i64) as usize;
        items.swap(i, j);
    }
}
