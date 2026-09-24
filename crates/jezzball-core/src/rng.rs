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
    ///
    /// Si `hi < lo` el rango está vacío y se devuelve `lo`. Es importante que
    /// no panique: todas las llamadas del motor construyen `hi` como
    /// `longitud - 1`, así que una colección vacía daría `hi = -1` con
    /// `lo = 0`. Antes eso hacía `span = 0` y el `% span` era una división
    /// por cero — un cierre del juego en release, donde el `debug_assert`
    /// ya no existe.
    pub fn range_i64(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            return lo;
        }
        // `wrapping_sub` evita desbordar con rangos extremos; el resultado
        // sigue siendo el ancho correcto interpretado como u64.
        let span = (hi.wrapping_sub(lo) as u64).wrapping_add(1);
        if span == 0 {
            // Rango de ancho 2^64: cualquier valor sirve.
            return self.next_u64() as i64;
        }
        lo.wrapping_add((self.next_u64() % span) as i64)
    }
}
