//! PRNG determinista propio (xorshift64*) exigido por ARCHITECTURE.md §1.
//!
//! No usamos `rand` ni entropía del SO: el mismo seed produce la misma
//! secuencia y, con él, la misma partida. `LevelSpec::seed` alimenta a los
//! RNG del laberinto, del zizagueo de las bolas `Erratic` y de los spawns
//! de power-ups.

/// Generador xorshift64*.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rng64 {
    state: u64,
}

impl Rng64 {
    /// Crea el PRNG a partir de una semilla.
    /// Un seed 0 se redirige a una constante para que el estado nunca quede a 0
    /// (xorshift degenera si el estado es cero).
    pub fn new(seed: u64) -> Self {
        let state = if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        };
        Rng64 { state }
    }

    /// xorshift64*: paso de mezcla + multiplicación final.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// `f32` uniforme en `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// `f32` uniforme en `[lo, hi)`.
    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.next_f32() * (hi - lo)
    }

    /// `i64` uniforme en `[lo, hi]` (ambos inclusive).
    pub fn range_i64(&mut self, lo: i64, hi: i64) -> i64 {
        debug_assert!(hi >= lo);
        let span = (hi - lo + 1) as u64;
        lo + (self.next_u64() % span) as i64
    }
}
