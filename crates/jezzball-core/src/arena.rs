//! La arena: rejilla materializada a partir de un `ArenaSpec`.
//!
//! `ArenaShape::Circle` y `ArenaShape::Maze` se materializan A CELDAS
//! `Solid` aquí mismo, en el constructor, usando el seed del nivel para el
//! laberinto. El resto del motor no sabe nada de la forma: solo trabaja con
//! `grid`. Los `Obstacle::Block` son `Solid` estáticos, los `NoSplit` se
//! marcan en la rejilla y los `Mover` se desplazan ocupando celdas `Solid`
//! mientras los atraviesan (ARCHITECTURE.md §2 y LEVEL_SCHEMA.md).

use crate::grid::{Cell, Grid};
use crate::level::{ArenaShape, ArenaSpec, Obstacle};
use crate::rng::Rng64;

/// Obstáculo móvil (`Obstacle::Mover`): se desplaza en línea recta, rebota
/// en los bordes de la arena y ocupa celdas `Solid` mientras lo atraviesa.
/// Si roza un muro en construcción lo destruye SIN coste de vidas
/// (decisión de diseño documentada: perder vidas es exclusivo de los
/// impactos de bola, ARCHITECTURE.md §3).
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

/// Arena lista para simular. `grid` es la fuente de verdad de ocupación.
#[derive(Clone, Debug, PartialEq)]
pub struct Arena {
    pub grid: Grid,
    pub shape: ArenaShape,
    movers: Vec<Mover>,
    /// Celdas que son `Solid` por la forma o por un `Block`: nunca se
    /// restauran a `Open` al pasar un `Mover`.
    static_solid: Vec<(u16, u16)>,
}

impl Arena {
    /// Materializa la forma y los obstáculos de un `ArenaSpec`.
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

    /// Avanza los `Mover`, rebota en los bordes y rasteriza sus celdas a
    /// `Solid`. Devuelve la lista de celdas que ocupan este frame (para que
    /// `state` destruya muros en construcción al ser rozados).
    pub fn update_movers(&mut self, dt: f32) -> Vec<(u16, u16)> {
        if self.movers.is_empty() {
            return Vec::new();
        }

        // 1) Avanzar y rebotar; calcular las celdas que ocuparán.
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

        // 2) Unión de la ocupación nueva (para no borrar celdas compartidas).
        let mut occupied = Vec::new();
        for cells in &new_cells {
            occupied.extend(cells.iter().copied());
        }

        // 3) Restaurar a `Open` las celdas viejas que ya no están ocupadas,
        //    salvo las pertenecientes a la forma o a un `Block` estático.
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

        // 4) Pintar la ocupación nueva como `Solid` (sin tocar sólidos estáticos).
        for (cx, cy) in &occupied {
            if !self.static_solid.contains(&(*cx, *cy)) {
                self.grid.set(*cx, *cy, Cell::Solid);
            }
        }

        // 5) Recordar las celdas actuales por mover.
        for (i, m) in self.movers.iter_mut().enumerate() {
            m.last_cells = new_cells[i].clone();
        }

        occupied
    }

    /// Celdas ocupadas ahora mismo por algún `Mover`.
    pub fn mover_cells_now(&self) -> Vec<(u16, u16)> {
        let mut out = Vec::new();
        for m in &self.movers {
            out.extend(m.last_cells.iter().copied());
        }
        out
    }
}

/// Celdas que cubre el rectángulo del `Mover`, recortadas a la arena.
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

/// `Irregular { notch }`: recorta `notch` celdas diagonales en cada esquina.
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

/// `Circle`: elipse inscrita; todo lo que quede fuera es `Solid`.
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

/// `Maze { density }`: laberinto (DFS con backtracking) generado con el PRNG
/// determinista sembrado desde `LevelSpec::seed`. `density` (0..=100) abre
/// pasadizos extra: más bucles y menos callejones cuanto mayor sea.
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

    // Arenas menores de 3 celdas no admiten laberinto: se dejan sólidas.
    if wu < 3 || hu < 3 {
        return;
    }

    // Todas las celdas inician como `Solid`; el DFS va abriendo pasajes.
    for y in 0..hu {
        for x in 0..wu {
            grid.set(x as u16, y as u16, Cell::Solid);
            static_solid.push((x as u16, y as u16));
        }
    }

    // Vértices del laberinto en coordenadas impares: (2i+1, 2j+1). El
    // número de vértices por dimensión es `w/2` (división entera): la cota
    // `(w+1).div_ceil(2)` producía un vértice extra en arenas de ancho/alto
    // par cuyo centro `2i+1 >= w` escribía fuera de la rejilla (pánico en
    // `Grid::set` al construir los niveles Maze 52/55/58 de Enhanced).
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
                // Arrancamos el muro intermedio (punto medio entre los dos).
                let wall_x = (2 * ci + 1 + 2 * ni).div_ceil(2);
                let wall_y = (2 * cj + 1 + 2 * nj).div_ceil(2);
                grid.set(wall_x as u16, wall_y as u16, Cell::Open);
                grid.set((2 * ni + 1) as u16, (2 * nj + 1) as u16, Cell::Open);
                stack.push((ni, nj));
            }
        }
    }

    // Los pasajes abiertos dejan de ser sólidos estáticos (para que un Mover
    // que pase pueda restaurarlos).
    static_solid.retain(|&(x, y)| grid.get(x, y) == Cell::Solid);

    // Pasadizos extra según densidad: cuantos más, más "abierto" el laberinto.
    let extra = (density as u32) * 3;
    for _ in 0..extra {
        let x = rng.range_i64(0, (wu - 1) as i64) as u16;
        let y = rng.range_i64(0, (hu - 1) as i64) as u16;
        grid.set(x, y, Cell::Open);
    }
    static_solid.retain(|&(x, y)| grid.get(x, y) == Cell::Solid);
}

/// Fisher-Yates con `rng` (determinista).
fn shuffle<T>(items: &mut [T], rng: &mut Rng64) {
    for i in (1..items.len()).rev() {
        let j = rng.range_i64(0, i as i64) as usize;
        items.swap(i, j);
    }
}
