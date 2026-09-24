//! Propiedades del PRNG determinista del motor.
//!
//! `Rng64` es la única fuente de aleatoriedad del core: de él depende que una
//! partida con la misma semilla se reproduzca exactamente igual. Estos tests
//! fijan su contrato, incluidos los rangos degenerados que antes dividían
//! por cero.

use jezzball_core::rng::Rng64;

/// Un rango vacío (`hi < lo`) debe devolver `lo`, no panicar.
/// Todas las llamadas del motor construyen `hi` como `longitud - 1`, así que
/// una colección vacía produce `hi = -1` con `lo = 0`.
#[test]
fn rango_vacio_no_panica() {
    let mut rng = Rng64::new(1);
    assert_eq!(rng.range_i64(0, -1), 0);
    assert_eq!(rng.range_i64(5, 4), 5);
    assert_eq!(rng.range_i64(-3, -10), -3);
    // Rango de un solo valor.
    assert_eq!(rng.range_i64(7, 7), 7);
}

/// Rangos extremos: no deben desbordar ni panicar.
#[test]
fn rangos_extremos_no_panican() {
    let mut rng = Rng64::new(99);
    // Con el rango completo de i64 cualquier valor es válido; lo que se
    // comprueba es que no panique y que no devuelva siempre lo mismo.
    let muestras: Vec<i64> = (0..1000)
        .map(|_| rng.range_i64(i64::MIN, i64::MAX))
        .collect();
    assert!(
        muestras.windows(2).any(|w| w[0] != w[1]),
        "range_i64 sobre el rango completo devolvio un valor constante"
    );
    let _ = rng.range_i64(i64::MIN, 0);
    let _ = rng.range_i64(0, i64::MAX);
    assert_eq!(rng.range_i64(i64::MIN, i64::MIN), i64::MIN);
    assert_eq!(rng.range_i64(i64::MAX, i64::MAX), i64::MAX);
}

/// Los valores devueltos SIEMPRE caen dentro del rango pedido.
#[test]
fn siempre_dentro_del_rango() {
    let mut rng = Rng64::new(2024);
    for lo in [-50i64, -1, 0, 1, 37] {
        for width in [0i64, 1, 2, 7, 63, 1000] {
            let hi = lo + width;
            for _ in 0..200 {
                let v = rng.range_i64(lo, hi);
                assert!(
                    v >= lo && v <= hi,
                    "range_i64({lo}, {hi}) devolvio {v}, fuera del rango"
                );
            }
        }
    }
}

/// Determinismo: la misma semilla produce exactamente la misma secuencia.
/// Es la propiedad de la que depende todo el motor y sus tests.
#[test]
fn misma_semilla_misma_secuencia() {
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

/// `next_f32` siempre en [0, 1).
#[test]
fn f32_en_rango_unitario() {
    let mut rng = Rng64::new(7);
    for _ in 0..5000 {
        let v = rng.next_f32();
        assert!((0.0..1.0).contains(&v), "next_f32 devolvio {v}");
    }
}

/// Semillas distintas producen secuencias distintas (cordura básica: un PRNG
/// que ignore la semilla haría todos los niveles idénticos).
#[test]
fn semillas_distintas_difieren() {
    let mut a = Rng64::new(1);
    let mut b = Rng64::new(2);
    let sa: Vec<u64> = (0..32).map(|_| a.next_u64()).collect();
    let sb: Vec<u64> = (0..32).map(|_| b.next_u64()).collect();
    assert_ne!(sa, sb);
}
