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
#[derive(Clone, Debug, PartialEq)]
pub struct WallBuilder {
    pub axis: WallAxis,
    pub origin: (u16, u16),   // celda donde el jugador pulsó
    pub lo: f32,              // frente que crece hacia -X/-Y (en celdas)
    pub hi: f32,              // frente que crece hacia +X/+Y
    pub lo_done: bool,        // llegó a borde/obstáculo
    pub hi_done: bool,
    pub speed: f32,           // celdas por segundo, por frente
    pub shielded: bool,       // power-up Escudo: absorbe 1 impacto
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
            speed,
            shielded,
        }
    }

    /// Celdas cubiertas actualmente por el segmento `[ceil(lo), floor(hi)]`.
    pub fn cells(&self) -> Vec<(u16, u16)> {
        let start = self.lo.ceil() as i64;
        let end = self.hi.floor() as i64;
        let mut out = Vec::new();
        for i in start..=end {
            let cell = match self.axis {
                WallAxis::Horizontal => (i as u16, self.origin.1),
                WallAxis::Vertical => (self.origin.0, i as u16),
            };
            out.push(cell);
        }
        out
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
            // Celda que el frente hi empieza a pisar este paso.
            let entry = new_hi.floor() as i64;
            if entry >= max_coord as i64 {
                // Llegó al borde: completa el último tramo y termina.
                self.hi = max_coord;
                self.hi_done = true;
            } else if !self.front_open(grid, entry) {
                // Apretado contra la celda bloqueante (sin cubrirla).
                self.hi = entry as f32 - EPS;
                self.hi_done = true;
            } else {
                self.hi = new_hi;
            }
        }

        if !self.lo_done {
            let new_lo = self.lo - self.speed * dt;
            if new_lo <= 0.0 {
                // Alcanzó el borde izquierdo/superior.
                self.lo = 0.0;
                self.lo_done = true;
            } else {
                // Celdas recién pisadas por el frente lo: las del intervalo
                // [ceil(new_lo), ceil(self.lo)).
                let old_start = self.lo.ceil() as i64;
                let new_start = new_lo.ceil() as i64;
                let mut blocked: Option<i64> = None;
                for coord in new_start..old_start {
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
                } else {
                    self.lo = new_lo.max(0.0);
                }
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