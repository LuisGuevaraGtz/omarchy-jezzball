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
/// Holgura para comparaciones de frontera de celda: tras un rebote la bola
/// queda apoyada EXACTAMENTE sobre una frontera, y sin esta tolerancia el
/// error de coma flotante decide al azar si sigue chocando o no.
const EPS: f32 = 1e-4;

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
    ///
    /// Sólo se consideran bloqueos que estén POR DELANTE del borde de avance
    /// actual de la bola. Es la diferencia entre rebotar y quedarse pegado:
    /// tras un rebote la bola queda apoyada justo contra la celda (su borde
    /// toca la frontera), y si el barrido siguiente volviera a mirar esa misma
    /// celda la daría por bloqueante otra vez, invirtiendo la velocidad cada
    /// frame (vibración) o empujándola al otro lado (atravesar el muro).
    /// Lo mismo ocurre cuando un muro se consolida DETRÁS de la bola: no debe
    /// empujarla ni invertir su sentido, sólo frenar lo que tenga delante.
    fn move_axis(&mut self, grid: &Grid, is_x: bool, dt: f32) {
        let velocity = if is_x { self.vel.x } else { self.vel.y };
        if velocity == 0.0 {
            return;
        }

        let current = if is_x { self.pos.x } else { self.pos.y };
        let along = current + velocity * dt;

        // Borde de ataque: el lado de la bola que avanza, antes y después.
        // Todo bloqueo que no esté estrictamente por delante del borde actual
        // se ignora (ya lo hemos dejado atrás o estamos apoyados en él).
        let (lead_now, lead_next) = if velocity > 0.0 {
            (current + self.radius, along + self.radius)
        } else {
            (current - self.radius, along - self.radius)
        };

        // Rango transversal que barre la bola (el eje perpendicular).
        //
        // Se encoge por EPS en ambos extremos a propósito. Tras rebotar, la
        // bola queda apoyada con su borde EXACTAMENTE sobre una frontera de
        // celda (p. ej. x + radio == 20.0 en una arena de 20 de ancho). Sin
        // este encogimiento, `floor()` de ese borde devuelve la celda de más
        // allá del muro —fuera de la rejilla, o el propio muro— y el eje
        // PERPENDICULAR la interpreta como bloqueo: la bola rebota también en
        // el otro eje y sale despedida por donde vino, como si hubiera
        // chocado en una esquina estando en mitad de una pared plana.
        // Encogiendo el rango sólo se consideran las celdas que la bola
        // solapa DE VERDAD, no las que toca de forma tangente.
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

        // Celdas que el borde de ataque cruza en este paso.
        let mut blocked: Option<i64> = None;
        if velocity > 0.0 {
            // `lead_now` puede estar exactamente sobre una frontera de celda
            // (bola apoyada tras rebotar): empezamos en la celda siguiente.
            let first = lead_now.floor() as i64;
            let last = lead_next.floor() as i64;
            'outer: for a in first..=last {
                // Ignorar la celda en la que ya estamos apoyados.
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
                // Ignorar la celda en la que ya estamos apoyados.
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
