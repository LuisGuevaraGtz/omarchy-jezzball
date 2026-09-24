//! Rejilla lógica de la arena: fuente de verdad para "qué está cerrado" y
//! para el % de área (ARCHITECTURE.md §2). Las coordenadas de celda son
//! `(columna, fila)` con `(0, 0)` arriba a la izquierda.

/// Estado de una celda de la rejilla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    /// Vacía: las bolas la atraviesan y los frentes de muro pueden crecer.
    Open,
    /// Rellenada por un muro consolidado o por el cierre de una región.
    Filled,
    /// Obstáculo fijo o forma de la arena: ni bolas ni muros la cruzan.
    Solid,
}

/// Rejilla `w × h` de celdas, más la máscara de zonas `NoSplit`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    pub w: u16,
    pub h: u16,
    cells: Vec<Cell>,
    // Verdadero en las celdas marcadas como `NoSplit`.
    //
    // Semántica de las celdas NoSplit:
    //   - son transitables por las bolas (como `Open`),
    //   - NUNCA pueden rellenarse ni cerrarse (nunca pasan a `Filled`),
    //   - bloquean los frentes de muro y el flood fill (impiden quedar
    //     encerradas), y
    //   - NO cuentan como "fillable": restan del denominador de
    //     `filled_ratio` (documentado también en LEVEL_SCHEMA.md).
    no_split: Vec<bool>,
}

impl Grid {
    /// Rejilla totalmente abierta.
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

    /// ¿El punto de rejilla `(x, y)` cae dentro de la arena?
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

    /// ¿Puede una bola pasar por la celda `(x, y)`? Devuelve `false` fuera de
    /// la arena (los bordes son muros). Las celdas `NoSplit` son transitables.
    pub fn is_passable(&self, x: i64, y: i64) -> bool {
        match self.in_bounds(x, y) {
            false => false,
            true => {
                let (u, v) = (x as u16, y as u16);
                self.get(u, v) == Cell::Open || self.is_no_split(u, v)
            }
        }
    }

    /// ¿Puede un frente de muro crecer sobre esta celda? Solo `Open` y que no
    /// sea `NoSplit` (los frentes se detienen en Filled/Solid/NoSplit/borde).
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

    /// Celdas que pueden rellenarse para ganar. Las `Solid` y las `NoSplit`
    /// NO cuentan (son el denominador de `filled_ratio`).
    pub fn fillable_count(&self) -> usize {
        self.cells.len() - self.solid_count() - self.no_split_count()
    }

    /// `filled / fillable`. Si no hay nada rellenable, devuelve 0.
    pub fn filled_ratio(&self) -> f32 {
        let fillable = self.fillable_count();
        if fillable == 0 {
            0.0
        } else {
            self.filled_count() as f32 / fillable as f32
        }
    }

    /// Flood fill 4-conexo sobre celdas `Open` que no sean `NoSplit`.
    /// Devuelve una lista de regiones, cada una con sus celdas.
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
