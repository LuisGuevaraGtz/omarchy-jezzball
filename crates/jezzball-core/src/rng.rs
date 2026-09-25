//! Our own deterministic PRNG (xorshift64*) required by ARCHITECTURE.md §1.
//!
//! We do not use `rand` nor OS entropy: the same seed produces the same
//! sequence and, with it, the same game. `LevelSpec::seed` feeds the RNGs of
//! the maze, of the zigzagging of `Erratic` balls and of the power-up
//! spawns.

/// xorshift64* generator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rng64 {
    state: u64,
}

impl Rng64 {
    /// Creates the PRNG from a seed.
    /// A seed of 0 is redirected to a constant so the state is never 0
    /// (xorshift degenerates if the state is zero).
    pub fn new(seed: u64) -> Self {
        let state = if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        };
        Rng64 { state }
    }

    /// xorshift64*: mixing step + final multiplication.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform `f32` in `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform `f32` in `[lo, hi)`.
    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.next_f32() * (hi - lo)
    }

    /// Uniform `i64` in `[lo, hi]` (both inclusive).
    ///
    /// If `hi < lo` the range is empty and `lo` is returned. It is important
    /// that this does not panic: every call in the engine builds `hi` as
    /// `length - 1`, so an empty collection would give `hi = -1` with
    /// `lo = 0`. That used to make `span = 0`, and the `% span` was a
    /// division by zero — a crash of the game in release, where the
    /// `debug_assert` no longer exists.
    pub fn range_i64(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            return lo;
        }
        // `wrapping_sub` avoids overflowing with extreme ranges; the result
        // is still the correct width when interpreted as u64.
        let span = (hi.wrapping_sub(lo) as u64).wrapping_add(1);
        if span == 0 {
            // Range of width 2^64: any value will do.
            return self.next_u64() as i64;
        }
        lo.wrapping_add((self.next_u64() % span) as i64)
    }
}
