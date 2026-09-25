//! Puntuación riesgo/recompensa y combos (ARCHITECTURE.md §5).
//!
//! Por cada región cerrada de `n` celdas en una arena con `open` celdas
//! abiertas antes del cierre:
//!
//! ```text
//! frac      = n / open                       // 0..1
//! size_mult = lerp(3.0, 0.4, frac)           // pequeño = arriesgado = paga más
//! risk      = 1.0 + 0.35 * balls_near        // bolas a <4 celdas del muro
//! speed_b   = 1.0 + 0.10 * max_ball_speed
//! base      = 100.0 * n.sqrt()
//! points    = base * size_mult * risk * speed_b * combo_mult
//! ```

/// Ventana de combo en segundos: cada muro consolidado sin perder vida inside
/// de esta ventana sube el multiplicador.
pub const COMBO_WINDOW: f32 = 4.0;
/// Tope del multiplicador de combo (x1 → x2 → ... → x8).
pub const COMBO_MAX: u8 = 8;

/// Multiplicador de combo y su reloj interno.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Combo {
    pub multiplier: u8,
    pub timer: f32,
}

impl Combo {
    pub fn new() -> Self {
        Combo {
            multiplier: 1,
            timer: 0.0,
        }
    }

    /// Sube un nivel el multiplicador (hasta `COMBO_MAX`).
    pub fn bump(&mut self) {
        if self.multiplier < COMBO_MAX {
            self.multiplier += 1;
        }
    }

    /// Reinicia a x1 y limpia el reloj (al perder vida o al expirar la ventana).
    pub fn reset(&mut self) {
        self.multiplier = 1;
        self.timer = 0.0;
    }

    /// Avanza el reloj `dt` segundos. Devuelve `true` si la ventana expiró
    /// (el multiplicador vuelve a x1).
    pub fn tick(&mut self, dt: f32) -> bool {
        self.timer += dt;
        if self.timer >= COMBO_WINDOW {
            self.reset();
            true
        } else {
            false
        }
    }
}

impl Default for Combo {
    fn default() -> Self {
        Combo::new()
    }
}

/// Puntos que otorga cerrar una región de `n` celdas.
pub fn region_points(
    n: u32,
    open: u32,
    balls_near: u32,
    max_ball_speed: f32,
    combo_multiplier: u8,
) -> u32 {
    let frac = if open == 0 {
        0.0
    } else {
        n as f32 / open as f32
    };
    let size_mult = lerp(3.0, 0.4, frac);
    let risk = 1.0 + 0.35 * balls_near as f32;
    let speed_b = 1.0 + 0.10 * max_ball_speed;
    let base = 100.0 * (n as f32).sqrt();
    (base * size_mult * risk * speed_b * combo_multiplier as f32)
        .round()
        .max(0.0) as u32
}

/// Interpolación lineal `a + (b - a) * t`, con `t` en `[0, 1]`.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
