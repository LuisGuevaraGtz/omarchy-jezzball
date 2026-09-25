//! Risk/reward scoring and combos (ARCHITECTURE.md §5).
//!
//! For every closed region of `n` cells in an arena with `open` open cells
//! before the closing:
//!
//! ```text
//! frac      = n / open                       // 0..1
//! size_mult = lerp(3.0, 0.4, frac)           // small = risky = pays more
//! risk      = 1.0 + 0.35 * balls_near        // balls within 4 cells of the wall
//! speed_b   = 1.0 + 0.10 * max_ball_speed
//! base      = 100.0 * n.sqrt()
//! points    = base * size_mult * risk * speed_b * combo_mult
//! ```

/// Combo window in seconds: every wall consolidated without losing a life
/// inside this window raises the multiplier.
pub const COMBO_WINDOW: f32 = 4.0;
/// Cap of the combo multiplier (x1 -> x2 -> ... -> x8).
pub const COMBO_MAX: u8 = 8;

/// Combo multiplier and its internal clock.
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

    /// Raises the multiplier by one level (up to `COMBO_MAX`).
    pub fn bump(&mut self) {
        if self.multiplier < COMBO_MAX {
            self.multiplier += 1;
        }
    }

    /// Resets to x1 and clears the clock (on losing a life or when the
    /// window expires).
    pub fn reset(&mut self) {
        self.multiplier = 1;
        self.timer = 0.0;
    }

    /// Advances the clock by `dt` seconds. Returns `true` if the window
    /// expired (the multiplier goes back to x1).
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

/// Points awarded for closing a region of `n` cells.
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

/// Linear interpolation `a + (b - a) * t`, with `t` in `[0, 1]`.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
