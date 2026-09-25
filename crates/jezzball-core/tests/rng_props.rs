//! Properties of the engine's deterministic PRNG.
//!
//! `Rng64` is the core's only source of randomness: reproducing a game
//! exactly from the same seed depends on it. These tests pin down its
//! contract, including the degenerate ranges that used to divide by
//! zero.

use jezzball_core::rng::Rng64;

/// An empty range (`hi < lo`) must return `lo`, not panic.
/// Every call site in the engine builds `hi` as `length - 1`, so an empty
/// collection yields `hi = -1` with `lo = 0`.
#[test]
fn empty_range_does_not_panic() {
    let mut rng = Rng64::new(1);
    assert_eq!(rng.range_i64(0, -1), 0);
    assert_eq!(rng.range_i64(5, 4), 5);
    assert_eq!(rng.range_i64(-3, -10), -3);
    // Single-value range.
    assert_eq!(rng.range_i64(7, 7), 7);
}

/// Extreme ranges: must neither overflow nor panic.
#[test]
fn extreme_ranges_do_not_panic() {
    let mut rng = Rng64::new(99);
    // Over the full i64 range any value is valid; what is being checked is
    // that it does not panic and that it does not always return the same thing.
    let samples: Vec<i64> = (0..1000)
        .map(|_| rng.range_i64(i64::MIN, i64::MAX))
        .collect();
    assert!(
        samples.windows(2).any(|w| w[0] != w[1]),
        "range_i64 over the full range returned a constant value"
    );
    let _ = rng.range_i64(i64::MIN, 0);
    let _ = rng.range_i64(0, i64::MAX);
    assert_eq!(rng.range_i64(i64::MIN, i64::MIN), i64::MIN);
    assert_eq!(rng.range_i64(i64::MAX, i64::MAX), i64::MAX);
}

/// The returned values ALWAYS fall inside the requested range.
#[test]
fn always_within_range() {
    let mut rng = Rng64::new(2024);
    for lo in [-50i64, -1, 0, 1, 37] {
        for width in [0i64, 1, 2, 7, 63, 1000] {
            let hi = lo + width;
            for _ in 0..200 {
                let v = rng.range_i64(lo, hi);
                assert!(
                    v >= lo && v <= hi,
                    "range_i64({lo}, {hi}) returned {v}, outside the range"
                );
            }
        }
    }
}

/// Determinism: the same seed produces exactly the same sequence.
/// This is the property the whole engine and its tests rely on.
#[test]
fn same_seed_same_sequence() {
    for seed in [0u64, 1, 42, 1000, u64::MAX] {
        let mut a = Rng64::new(seed);
        let mut b = Rng64::new(seed);
        for _ in 0..500 {
            assert_eq!(a.next_u64(), b.next_u64());
            assert_eq!(a.range_i64(0, 63), b.range_i64(0, 63));
            assert_eq!(a.next_f32(), b.next_f32());
        }
    }
}

/// `next_f32` always in [0, 1).
#[test]
fn f32_stays_in_the_unit_range() {
    let mut rng = Rng64::new(7);
    for _ in 0..5000 {
        let v = rng.next_f32();
        assert!((0.0..1.0).contains(&v), "next_f32 returned {v}");
    }
}

/// Different seeds produce different sequences (basic sanity: a PRNG that
/// ignored the seed would make every level identical).
#[test]
fn different_seeds_differ() {
    let mut a = Rng64::new(1);
    let mut b = Rng64::new(2);
    let sa: Vec<u64> = (0..32).map(|_| a.next_u64()).collect();
    let sb: Vec<u64> = (0..32).map(|_| b.next_u64()).collect();
    assert_ne!(sa, sb);
}
