//! Logical arena grid: the source of truth for "what is closed" and for the
//! area percentage (ARCHITECTURE.md §2). Cell coordinates are
//! `(column, row)` with `(0, 0)` at the top left.

/// State of a grid cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    /// Empty: balls pass through it and wall fronts can grow over it.
    Open,
    /// Filled in by a consolidated wall or by the closing of a region.
    Filled,
    /// Fixed obstacle or arena shape: neither balls nor walls cross it.
    Solid,
}

/// `w x h` grid of cells, plus the mask of `NoSplit` zones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    pub w: u16,
    pub h: u16,
    cells: Vec<Cell>,
    // True on the cells marked as `NoSplit`.
    //
    // Semantics of NoSplit cells:
    //   - they are traversable by balls (like `Open`),
    //   - they can NEVER be filled or closed (they never become `Filled`),
    //   - they block wall fronts and the flood fill (which prevents them
    //     from being enclosed), and
    //   - they do NOT count as "fillable": they are subtracted from the
    //     denominator of `filled_ratio` (also documented in
    //     LEVEL_SCHEMA.md).
    no_split: Vec<bool>,
}

impl Grid {
    /// Fully open grid.
    pub fn new(w: u16, h: u16) -> Self {
        let area = (w as usize) * (h as usize);
        Grid {
            w,
            h,
            cells: vec![Cell::Open; area],
            no_split: vec![false; area],
        }
    }

    pub fn idx(&self, x: u16, y: u16) -> usize {
        (y as usize) * (self.w as usize) + x as usize
    }

    /// Does the grid point `(x, y)` fall inside the arena?
    pub fn in_bounds(&self, x: i64, y: i64) -> bool {
        x >= 0 && y >= 0 && (x as u64) < self.w as u64 && (y as u64) < self.h as u64
    }

    pub fn get(&self, x: u16, y: u16) -> Cell {
        self.cells[self.idx(x, y)]
    }

    pub fn set(&mut self, x: u16, y: u16, cell: Cell) {
        let i = self.idx(x, y);
        self.cells[i] = cell;
    }

    pub fn is_open(&self, x: u16, y: u16) -> bool {
        self.get(x, y) == Cell::Open
    }

    pub fn is_solid(&self, x: u16, y: u16) -> bool {
        self.get(x, y) == Cell::Solid
    }

    pub fn is_no_split(&self, x: u16, y: u16) -> bool {
        self.no_split[self.idx(x, y)]
    }

    pub fn set_no_split(&mut self, x: u16, y: u16) {
        let i = self.idx(x, y);
        self.no_split[i] = true;
    }

    /// Can a ball pass through cell `(x, y)`? Returns `false` outside the
    /// arena (the borders are walls). `NoSplit` cells are traversable.
    pub fn is_passable(&self, x: i64, y: i64) -> bool {
        match self.in_bounds(x, y) {
            false => false,
            true => {
                let (u, v) = (x as u16, y as u16);
                self.get(u, v) == Cell::Open || self.is_no_split(u, v)
            }
        }
    }

    /// Can a wall front grow over this cell? Only `Open` cells that are not
    /// `NoSplit` (fronts stop at Filled/Solid/NoSplit/border).
    pub fn is_wall_open(&self, x: i64, y: i64) -> bool {
        if !self.in_bounds(x, y) {
            return false;
        }
        let (u, v) = (x as u16, y as u16);
        self.get(u, v) == Cell::Open && !self.is_no_split(u, v)
    }

    pub fn open_count(&self) -> usize {
        self.cells.iter().filter(|c| **c == Cell::Open).count()
    }

    pub fn filled_count(&self) -> usize {
        self.cells.iter().filter(|c| **c == Cell::Filled).count()
    }

    pub fn solid_count(&self) -> usize {
        self.cells.iter().filter(|c| **c == Cell::Solid).count()
    }

    pub fn no_split_count(&self) -> usize {
        self.no_split.iter().filter(|b| **b).count()
    }

    /// Cells that can be filled in to win. `Solid` and `NoSplit` cells do
    /// NOT count (they are the denominator of `filled_ratio`).
    pub fn fillable_count(&self) -> usize {
        self.cells.len() - self.solid_count() - self.no_split_count()
    }

    /// `filled / fillable`. If there is nothing fillable, returns 0.
    pub fn filled_ratio(&self) -> f32 {
        let fillable = self.fillable_count();
        if fillable == 0 {
            0.0
        } else {
            self.filled_count() as f32 / fillable as f32
        }
    }

    /// 4-connected flood fill over `Open` cells that are not `NoSplit`.
    /// Returns a list of regions, each one with its cells.
    pub fn open_regions(&self) -> Vec<Vec<(u16, u16)>> {
        let mut visited = vec![false; self.cells.len()];
        let mut regions = Vec::new();
        for y in 0..self.h {
            for x in 0..self.w {
                let i = self.idx(x, y);
                if visited[i] || self.cells[i] != Cell::Open || self.no_split[i] {
                    continue;
                }
                visited[i] = true;
                let mut stack = vec![(x, y)];
                let mut region = Vec::new();
                while let Some((cx, cy)) = stack.pop() {
                    region.push((cx, cy));
                    for (dx, dy) in [(1i64, 0), (0, 1), (-1, 0), (0, -1)] {
                        let (nx, ny) = (cx as i64 + dx, cy as i64 + dy);
                        if self.in_bounds(nx, ny) {
                            let (ux, uy) = (nx as u16, ny as u16);
                            let ni = self.idx(ux, uy);
                            if !visited[ni] && self.cells[ni] == Cell::Open && !self.no_split[ni] {
                                visited[ni] = true;
                                stack.push((ux, uy));
                            }
                        }
                    }
                }
                regions.push(region);
            }
        }
        regions
    }
}
