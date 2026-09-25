//! Reproducción de los fallos de física reportados al jugar.
//!
//! Estos tests se escribieron ANTES del arreglo, para demostrar el bug.

use jezzball_core::ball::Ball;
use jezzball_core::grid::{Cell, Grid};
use jezzball_core::level::BallKind;
use jezzball_core::rng::Rng64;

/// Arena 10x10 abierta.
fn arena() -> Grid {
    Grid::new(10, 10)
}

/// BUG 1 — Tras rebotar contra un muro, la bola queda apoyada exactamente en
/// `celda - radio`, de modo que el borde de su AABB cae JUSTO sobre la celda
/// bloqueante. Al frame siguiente, viajando ya en sentido contrario, el
/// barrido vuelve a "ver" esa misma celda y la trata como bloqueo, con lo que
/// la teletransporta AL OTRO LADO del muro e invierte otra vez la velocidad.
///
/// Síntoma en pantalla: las bolas se pegan, vibran o cruzan muros.
#[test]
fn rebote_no_teletransporta_ni_atraviesa_el_muro() {
    let mut g = arena();
    // Muro vertical en la columna 5.
    for y in 0..10 {
        g.set(5, y, Cell::Filled);
    }
    let mut rng = Rng64::new(1);

    // Bola avanzando hacia +X, a la izquierda del muro.
    let mut b = Ball::new(0, 4.0, 4.5, 6.0, 0.0, BallKind::Normal, 1.0);
    let radius = b.radius;

    // Simulamos 120 frames a 60 FPS. La bola NUNCA debe cruzar la columna 5.
    for frame in 0..120 {
        b.step(&g, &mut rng, 1.0 / 60.0);
        assert!(
            b.pos.x + radius <= 5.0 + 1e-3,
            "frame {frame}: la bola atraveso el muro (x={}, radio={radius})",
            b.pos.x
        );
        assert!(
            b.pos.x - radius >= -1e-3,
            "frame {frame}: la bola salio por el borde izquierdo (x={})",
            b.pos.x
        );
    }
}

/// BUG 1 (variante) — El rebote debe conservar la RAPIDEZ. Si la bola se
/// queda vibrando contra la pared, la velocidad se invierte varias veces por
/// frame y el movimiento deja de ser el del JezzBall clásico.
#[test]
fn rebote_conserva_la_rapidez_y_avanza_de_verdad() {
    let mut g = arena();
    for y in 0..10 {
        g.set(5, y, Cell::Filled);
    }
    let mut rng = Rng64::new(1);
    let mut b = Ball::new(0, 4.0, 4.5, 6.0, 0.0, BallKind::Normal, 1.0);
    let speed0 = b.speed();

    let mut min_x = f32::MAX;
    for _ in 0..240 {
        b.step(&g, &mut rng, 1.0 / 60.0);
        min_x = min_x.min(b.pos.x);
        assert!(
            (b.speed() - speed0).abs() < 1e-3,
            "la rapidez cambio: {} -> {}",
            speed0,
            b.speed()
        );
    }
    // Tras rebotar debe haber viajado de vuelta hacia la izquierda,
    // no quedarse pegada al muro.
    assert!(
        min_x < 3.0,
        "la bola se quedo pegada al muro (min_x={min_x}); deberia haber rebotado y viajado"
    );
}

/// BUG 2 — Una celda que se rellena DETRÁS de la bola (caso normal: el muro
/// se consolida justo donde la bola acaba de pasar) no debe empujarla ni
/// invertir su velocidad: sólo bloquea lo que hay por delante.
#[test]
fn filled_cell_behind_does_not_push_the_ball() {
    let mut g = arena();
    let mut rng = Rng64::new(1);
    // Bola en el centro de la celda 4, viajando hacia +X.
    let mut b = Ball::new(0, 4.5, 4.5, 6.0, 0.0, BallKind::Normal, 1.0);
    b.step(&g, &mut rng, 1.0 / 60.0);
    let vel_antes = b.vel.x;
    let x_antes = b.pos.x;

    // Ahora se rellena la celda que la bola tiene DETRÁS.
    g.set(4, 4, Cell::Filled);
    b.step(&g, &mut rng, 1.0 / 60.0);

    assert!(
        b.vel.x > 0.0,
        "la bola invirtio su velocidad por una celda que tenia detras ({vel_antes} -> {})",
        b.vel.x
    );
    assert!(
        b.pos.x >= x_antes,
        "la bola retrocedio por una celda que tenia detras ({x_antes} -> {})",
        b.pos.x
    );
}

/// BUG 3 — En una esquina, la bola rebota en ambos ejes y debe seguir inside
/// de la arena, sin quedar atrapada ni salir despedida.
#[test]
fn rebote_en_esquina_mantiene_la_bola_dentro() {
    let g = arena();
    let mut rng = Rng64::new(7);
    // Hacia la esquina superior izquierda.
    let mut b = Ball::new(0, 1.0, 1.0, -9.0, -9.0, BallKind::Normal, 1.0);
    let r = b.radius;
    for frame in 0..300 {
        b.step(&g, &mut rng, 1.0 / 60.0);
        assert!(
            b.pos.x - r >= -1e-3 && b.pos.y - r >= -1e-3,
            "frame {frame}: la bola salio por la esquina ({}, {})",
            b.pos.x,
            b.pos.y
        );
        assert!(
            b.pos.x + r <= 10.0 + 1e-3 && b.pos.y + r <= 10.0 + 1e-3,
            "frame {frame}: la bola salio por el lado opuesto ({}, {})",
            b.pos.x,
            b.pos.y
        );
    }
}

/// BUG 4 — Una bola encajonada en un pasillo de una sola celda debe rebotar
/// limpiamente de lado a lado, sin vibrar ni escaparse.
#[test]
fn pasillo_estrecho_rebota_limpio() {
    let mut g = arena();
    // Pasillo horizontal de 1 celda de height en la fila 4, entre x=1 y x=8.
    for x in 0..10 {
        for y in 0..10 {
            if y != 4 {
                g.set(x, y, Cell::Filled);
            }
        }
    }
    g.set(0, 4, Cell::Filled);
    g.set(9, 4, Cell::Filled);

    let mut rng = Rng64::new(3);
    let mut b = Ball::new(0, 4.5, 4.5, 7.0, 0.0, BallKind::Normal, 1.0);
    let r = b.radius;
    for frame in 0..600 {
        b.step(&g, &mut rng, 1.0 / 60.0);
        assert!(
            b.pos.y - r >= 4.0 - 1e-3 && b.pos.y + r <= 5.0 + 1e-3,
            "frame {frame}: la bola se salio del pasillo (y={})",
            b.pos.y
        );
        assert!(
            b.pos.x - r >= 1.0 - 1e-3 && b.pos.x + r <= 9.0 + 1e-3,
            "frame {frame}: la bola se salio por los topes (x={})",
            b.pos.x
        );
    }
}
