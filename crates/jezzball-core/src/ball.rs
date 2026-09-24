//! Bolas continuas (ARCHITECTURE.md §2 y §8).
//!
//! Las bolas se mueven en coordenadas de celda `f32`. El movimiento usa
//! integración semi-implícita con resolución de colisión POR EJES SEPARADOS
//! contra la rejilla: mover en X, rebotar si la celda destino no es `Open`;
//! mover en Y, idéntico. Eso produce el rebote de 45° clásico del JezzBall
//! sin tuneleo, y sin túneles mientras `|vel| * dt < 0.5` celdas. Cuando
//! `|vel| * dt >= 0.5` se aplican substeps internos (máx. 8).

use crate::geom::Vec2;
use crate::grid::Grid;
use crate::level::BallKind;
use crate::rng::Rng64;

/// Cada cuántos segundos una bola `Erratic` rota su velocidad ±30°.
const ERRATIC_INTERVAL: f32 = 1.5;
/// ±30° en radianes.
const ERRATIC_ANGLE: f32 = std::f32::consts::PI / 6.0;

/// Distancia máxima (celdas) por substep sin riesgo de túnel.
const MAX_STEP_DISTANCE: f32 = 0.5;
/// Tope de substeps por frame.
const MAX_SUBSTEPS: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub struct Ball {
    pub id: u32,
    pub pos: Vec2,
    pub vel: Vec2,
    pub kind: BallKind,
    pub radius: f32,
    /// Posición anterior: permite usar un AABB barrido para detectar impactos
    /// contra muros en construcción sin túneles entre frames.
    pub prev: Vec2,
    erratic_timer: f32,
}

impl Ball {
    /// Crea una bola a partir de un `BallSpawn`. La velocidad y el radio
    /// dependen del tipo (`BallKind::speed_mult` / `base_radius`).
    pub fn new(
        id: u32,
        x: f32,
        y: f32,
        vx: f32,
        vy: f32,
        kind: BallKind,
        radius_mul: f32,
    ) -> Self {
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

    /// Integra la bola un paso `dt`.
    ///
    /// - Si el tipo es `Erratic`, cada `ERRATIC_INTERVAL` segundos rota su
    ///   velocidad ±30° usando el PRNG determinista del core.
    /// - Si `|vel| * dt >= 0.5` divide el paso en substeps para no atravesar
    ///   celdas por alto.
    pub fn step(&mut self, grid: &Grid, rng: &mut Rng64, dt: f32) {
        if dt <= 0.0 {
            return;
        }

        // Snapshot de la posición previa para el AABB barrido: el `state`
        // usa `prev`/`pos` para detectar impactos contra muros en construcción
        // sin túneles entre frames.
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

    /// Resuelve el movimiento en un eje con separación de ejes.
    /// `is_x = true` mueve (e rebota) el eje X.
    fn move_axis(&mut self, grid: &Grid, is_x: bool, dt: f32) {
        let velocity = if is_x { self.vel.x } else { self.vel.y };
        if velocity == 0.0 {
            return;
        }

        // Proyección del AABB en la posición candidata.
        let along = if is_x {
            self.pos.x + velocity * dt
        } else {
            self.pos.y + velocity * dt
        };
        let (a0, a1) = (along - self.radius, along + self.radius);
        let (c0, c1) = if is_x {
            (self.pos.y - self.radius, self.pos.y + self.radius)
        } else {
            (self.pos.x - self.radius, self.pos.x + self.radius)
        };

        let cell_a0 = a0.floor() as i64;
        let cell_a1 = a1.floor() as i64;
        let cell_c0 = c0.floor() as i64;
        let cell_c1 = c1.floor() as i64;

        // Buscamos el primer bloqueo en la dirección de avance: si vamos en
        // positivo, el primer bloqueo de menor coordenada; si vamos en
        // negativo, el de mayor coordenada (el que la bola toca primero).
        let mut blocked: Option<i64> = None;
        if velocity > 0.0 {
            'outer: for a in cell_a0..=cell_a1 {
                for c in cell_c0..=cell_c1 {
                    let (cx, cy) = if is_x { (a, c) } else { (c, a) };
                    if !grid.is_passable(cx, cy) {
                        blocked = Some(a);
                        break 'outer;
                    }
                }
            }
        } else {
            'outer: for a in (cell_a0..=cell_a1).rev() {
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
            // Apoyar el borde de la bola contra la celda bloqueante.
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