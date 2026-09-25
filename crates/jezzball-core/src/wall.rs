//! Muros (ARCHITECTURE.md §3).
//!
//! Un `WallBuilder` es el muro EN CONSTRUCCIÓN: crece en AMBAS direcciones
//! desde su celda de origen a la vez (un frente hacia coordenadas decrecientes
//! `lo`, otro hacia crecientes `hi`), a `speed` celdas por segundo y por
//! frente. Cada frente se detiene de forma independiente al tocar
//! `Filled`/`Solid`/`NoSplit` o el borde. Solo cuando `lo_done && hi_done`
//! está listo para consolidarse.

use crate::grid::Grid;

/// Eje de crecimiento de un muro.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WallAxis {
    Horizontal, // crece a lo largo de las columnas (misma fila que el origen)
    Vertical,   // crece a lo largo de las filas (misma columna que el origen)
}

impl WallAxis {
    /// Coordenada máxima (inclusive) que puede alcanzar un frente de este eje.
    pub fn max_coord(self, w: u16, h: u16) -> f32 {
        match self {
            WallAxis::Horizontal => (w - 1) as f32,
            WallAxis::Vertical => (h - 1) as f32,
        }
    }
}

/// Muro que el jugador está trazando en estos momentos.
///
/// Regla del JezzBall original: cada mitad se "convierte" (queda fijada) en
/// cuanto su frente alcanza una pared u obstáculo, y desde ese momento es
/// inmune a las bolas. Sólo la mitad que sigue creciendo puede costar una
/// vida. `lo_sealed`/`hi_sealed` registran qué mitades ya se fijaron.
#[derive(Clone, Debug, PartialEq)]
pub struct WallBuilder {
    pub axis: WallAxis,
    pub origin: (u16, u16), // celda donde el jugador pulsó
    pub lo: f32,            // frente que crece hacia -X/-Y (en celdas)
    pub hi: f32,            // frente que crece hacia +X/+Y
    pub lo_done: bool,      // llegó a borde/obstáculo
    pub hi_done: bool,
    /// La mitad `lo` ya se volcó a la rejilla como `Filled` (es inmune).
    pub lo_sealed: bool,
    /// La mitad `hi` ya se volcó a la rejilla como `Filled` (es inmune).
    pub hi_sealed: bool,
    pub speed: f32,     // celdas por segundo, por frente
    pub shielded: bool, // power-up Escudo: absorbe 1 impacto
}

/// Pequeña tolerancia para "apretar" un frente contra la celda bloqueante sin
/// cubrirla: si el frente se detiene en la celda `c`, lo dejamos en `c - EPS`.
const EPS: f32 = 1e-3;

impl WallBuilder {
    /// Crea un muro empezando en su origen (un solo frente con `lo == hi`).
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

    /// Celdas cubiertas actualmente por el segmento `[ceil(lo), floor(hi)]`.
    pub fn cells(&self) -> Vec<(u16, u16)> {
        self.cells_between(self.lo.ceil() as i64, self.hi.floor() as i64)
    }

    /// Celdas del segmento entre dos coordenadas del eje de crecimiento.
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

    /// Coordenada del origen en el eje de crecimiento.
    fn origin_coord(&self) -> i64 {
        match self.axis {
            WallAxis::Horizontal => self.origin.0 as i64,
            WallAxis::Vertical => self.origin.1 as i64,
        }
    }

    /// Celdas de la mitad `lo`: desde el frente hasta el origen (excluido).
    pub fn lo_cells(&self) -> Vec<(u16, u16)> {
        self.cells_between(self.lo.ceil() as i64, self.origin_coord() - 1)
    }

    /// Celdas de la mitad `hi`: desde el origen (incluido) hasta el frente.
    /// El origen se asigna a esta mitad para que ninguna celda quede huérfana.
    pub fn hi_cells(&self) -> Vec<(u16, u16)> {
        self.cells_between(self.origin_coord(), self.hi.floor() as i64)
    }

    /// Celdas que una bola PUEDE destruir: sólo las de las mitades que aún
    /// están creciendo. Una mitad ya sellada se comporta como muro normal.
    ///
    /// No se filtra por el estado de la rejilla: las celdas de una mitad viva
    /// nunca se han volcado a ella, y filtrar por `is_open` haría desaparecer
    /// las que un `Mover` esté pisando en ese instante (marcadas `Solid`
    /// temporalmente), que son precisamente las que deben detectar el choque.
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

    /// ¿Queda alguna mitad viva (no sellada)? Si no, el muro ya está entero
    /// en la rejilla y no puede costar vidas.
    pub fn has_live_half(&self) -> bool {
        !self.lo_sealed || !self.hi_sealed
    }

    /// ¿Ambos frentes llegaron a su límite? Solo entonces se consolida.
    pub fn is_done(&self) -> bool {
        self.lo_done && self.hi_done
    }

    /// Avanza los dos frentes `dt` segundos. Un frente que ya terminó no se
    /// mueve. Este avance es puro contra la rejilla actual.
    pub fn advance(&mut self, grid: &Grid, dt: f32) {
        let max_coord = self.axis.max_coord(grid.w, grid.h);

        if !self.hi_done {
            let new_hi = self.hi + self.speed * dt;
            // Barremos TODAS las celdas que el frente pisa en este paso, no
            // sólo la de destino: con `speed` alta (hasta 30 celdas/s) y un
            // frame largo el frente avanza varias celdas de golpe, y mirar
            // únicamente el destino le permitía saltar por encima de muros y
            // obstáculos intermedios.
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
                // Apretado contra la celda bloqueante (sin cubrirla).
                self.hi = cell as f32 - EPS;
                self.hi_done = true;
            } else if new_end >= max_coord as i64 {
                // Llegó al borde: completa el último tramo y termina.
                self.hi = max_coord;
                self.hi_done = true;
            } else {
                self.hi = new_hi;
            }
        }

        if !self.lo_done {
            let new_lo = self.lo - self.speed * dt;
            // Celdas recién pisadas por el frente lo: las del intervalo
            // [ceil(new_lo), ceil(self.lo)). Se comprueban SIEMPRE, incluso
            // al llegar al borde: si hay un obstáculo por el camino, el frente
            // debe pararse ahí y no en la celda 0.
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
                // Frente apoyado a la derecha de `cell`: la cobertura
                // arranca en `cell + 1`, que no cubre la celda bloqueante.
                self.lo = (cell + 1) as f32;
                self.lo_done = true;
            } else if new_lo <= 0.0 {
                // Alcanzó el borde izquierdo/superior sin encontrar nada.
                self.lo = 0.0;
                self.lo_done = true;
            } else {
                self.lo = new_lo;
            }
        }
    }

    /// ¿El frente por la coordenada `coord` tiene celda abierta para crecer?
    fn front_open(&self, grid: &Grid, coord: i64) -> bool {
        match self.axis {
            WallAxis::Horizontal => grid.is_wall_open(coord, self.origin.1 as i64),
            WallAxis::Vertical => grid.is_wall_open(self.origin.0 as i64, coord),
        }
    }
}

/// Muro ya consolidado (registro histórico usado por la capa de render/audio).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wall {
    pub axis: WallAxis,
    pub origin: (u16, u16),
    pub cells: u32,
    pub points: u32,
}
