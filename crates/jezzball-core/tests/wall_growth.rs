//! Crecimiento de los frentes de muro (ARCHITECTURE.md §3).
//!
//! Un frente debe detenerse en la PRIMERA celda bloqueante que encuentra.
//! Con `wall_speed` alta (hasta 30 celdas/s) y un frame largo, un frente
//! puede avanzar varias celdas de golpe: si sólo se comprueba la celda de
//! destino, salta por encima de muros y obstáculos.

use jezzball_core::grid::{Cell, Grid};
use jezzball_core::wall::{WallAxis, WallBuilder};

/// El frente `hi` no debe saltarse un obstáculo aunque avance varias celdas
/// en un solo paso.
#[test]
fn frente_hi_no_atraviesa_obstaculo_con_paso_grande() {
    let mut g = Grid::new(40, 20);
    // Bloque sólido en la columna 12, fila 10.
    g.set(12, 10, Cell::Solid);

    let mut b = WallBuilder::new(WallAxis::Horizontal, (5, 10), 30.0, false);
    // Un frame largo (~3 celdas a 30 celdas/s): el frente pasa de 5 a ~14,
    // saltándose la columna 12 si sólo se mira la celda de destino.
    b.advance(&g, 0.3);

    let celdas = b.cells();
    assert!(
        !celdas.iter().any(|&(x, y)| x >= 12 && y == 10),
        "el frente atraveso el obstaculo de la columna 12: {celdas:?}"
    );
    assert!(
        b.hi_done,
        "el frente deberia haberse detenido en el obstaculo"
    );
}

/// Mismo caso para el frente `lo` (que ya barría el intervalo, pero se
/// verifica para que no se rompa en el futuro).
#[test]
fn frente_lo_no_atraviesa_obstaculo_con_paso_grande() {
    let mut g = Grid::new(40, 20);
    g.set(8, 10, Cell::Solid);

    let mut b = WallBuilder::new(WallAxis::Horizontal, (18, 10), 30.0, false);
    b.advance(&g, 0.5);

    let celdas = b.cells();
    assert!(
        !celdas.iter().any(|&(x, y)| x <= 8 && y == 10),
        "el frente lo atraveso el obstaculo de la columna 8: {celdas:?}"
    );
    assert!(b.lo_done, "el frente lo deberia haberse detenido");
}

/// Un frente jamás debe cubrir una celda que no sea transitable para muros.
#[test]
fn covered_cells_are_always_valid() {
    let mut g = Grid::new(30, 30);
    // Un tablero con obstáculos dispersos.
    for i in 0..30 {
        if i % 7 == 0 {
            g.set(i, 15, Cell::Filled);
        }
    }

    for origin_x in [1u16, 5, 10, 16, 22, 29] {
        if !g.is_wall_open(origin_x as i64, 15) {
            continue;
        }
        let mut b = WallBuilder::new(WallAxis::Horizontal, (origin_x, 15), 28.0, false);
        for _ in 0..200 {
            b.advance(&g, 1.0 / 60.0);
            for (x, y) in b.cells() {
                assert!(
                    g.is_wall_open(x as i64, y as i64),
                    "el frente desde x={origin_x} cubrio la celda no valida ({x}, {y})"
                );
            }
            if b.is_done() {
                break;
            }
        }
    }
}

/// Un muro en una arena totalmente abierta debe llegar a ambos bordes y
/// cubrir la fila completa.
#[test]
fn wall_in_open_arena_covers_the_whole_line() {
    let g = Grid::new(20, 12);
    let mut b = WallBuilder::new(WallAxis::Vertical, (10, 6), 25.0, false);
    for _ in 0..600 {
        b.advance(&g, 1.0 / 60.0);
        if b.is_done() {
            break;
        }
    }
    assert!(b.is_done(), "el muro nunca termino de crecer");
    let celdas = b.cells();
    assert_eq!(
        celdas.len(),
        12,
        "el muro vertical deberia cubrir las 12 filas, cubrio {}",
        celdas.len()
    );
}

/// Arena mínima: no debe panicar ni cubrir fuera de rango.
#[test]
fn arena_minima_no_panica() {
    let g = Grid::new(3, 3);
    let mut b = WallBuilder::new(WallAxis::Horizontal, (1, 1), 30.0, false);
    for _ in 0..120 {
        b.advance(&g, 1.0 / 60.0);
        for (x, y) in b.cells() {
            assert!(x < 3 && y < 3, "celda fuera de la arena: ({x}, {y})");
        }
        if b.is_done() {
            break;
        }
    }
}
