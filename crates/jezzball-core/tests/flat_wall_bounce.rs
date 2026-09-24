//! Reproducción del rebote "como si fuera esquina" en una pared PLANA.
//!
//! Síntoma reportado al jugar: la bola choca contra el centro de una pared
//! (no en una esquina) y se vuelve exactamente por donde vino, en lugar de
//! reflejarse. Tras varios rebotes así, entra en un ciclo cerrado.
//!
//! Un rebote correcto contra una pared vertical invierte SOLO `vx`; contra
//! una horizontal, SOLO `vy`. Invertir ambas es el rebote de esquina, y sólo
//! debe ocurrir cuando la bola realmente llega a una esquina.

use jezzball_core::ball::Ball;
use jezzball_core::grid::{Cell, Grid};
use jezzball_core::level::BallKind;
use jezzball_core::rng::Rng64;

const DT: f32 = 1.0 / 60.0;

/// Arena 20x20 con bordes implícitos (fuera de la rejilla no es transitable).
fn arena() -> Grid {
    Grid::new(20, 20)
}

/// Avanza hasta detectar el primer cambio de signo en algún eje y devuelve
/// `(cambio_en_x, cambio_en_y)` de ESE frame concreto.
fn primer_rebote(b: &mut Ball, g: &Grid, max_frames: usize) -> (bool, bool) {
    let mut rng = Rng64::new(1);
    for _ in 0..max_frames {
        let vx_antes = b.vel.x;
        let vy_antes = b.vel.y;
        b.step(g, &mut rng, DT);
        let cambio_x = vx_antes.signum() != b.vel.x.signum();
        let cambio_y = vy_antes.signum() != b.vel.y.signum();
        if cambio_x || cambio_y {
            return (cambio_x, cambio_y);
        }
    }
    panic!("la bola no reboto en {max_frames} frames");
}

/// PARED DERECHA (vertical): debe invertir sólo `vx`.
#[test]
fn pared_vertical_derecha_invierte_solo_vx() {
    let g = arena();
    // Diagonal pura hacia la derecha y abajo, lejos de las esquinas.
    let mut b = Ball::new(0, 15.0, 10.0, 8.0, 8.0, BallKind::Normal, 1.0);
    let (cx, cy) = primer_rebote(&mut b, &g, 600);
    assert!(cx, "no invirtio vx al chocar con la pared derecha");
    assert!(
        !cy,
        "INVIRTIO TAMBIEN vy: rebote de esquina en una pared plana \
         (la bola se vuelve por donde vino)"
    );
}

/// PARED IZQUIERDA.
#[test]
fn pared_vertical_izquierda_invierte_solo_vx() {
    let g = arena();
    let mut b = Ball::new(0, 5.0, 10.0, -8.0, 8.0, BallKind::Normal, 1.0);
    let (cx, cy) = primer_rebote(&mut b, &g, 600);
    assert!(cx, "no invirtio vx al chocar con la pared izquierda");
    assert!(!cy, "INVIRTIO TAMBIEN vy: rebote de esquina en pared plana");
}

/// PARED SUPERIOR (horizontal): debe invertir sólo `vy`.
#[test]
fn pared_horizontal_superior_invierte_solo_vy() {
    let g = arena();
    let mut b = Ball::new(0, 10.0, 5.0, 8.0, -8.0, BallKind::Normal, 1.0);
    let (cx, cy) = primer_rebote(&mut b, &g, 600);
    assert!(cy, "no invirtio vy al chocar con la pared superior");
    assert!(!cx, "INVIRTIO TAMBIEN vx: rebote de esquina en pared plana");
}

/// PARED INFERIOR.
#[test]
fn pared_horizontal_inferior_invierte_solo_vy() {
    let g = arena();
    let mut b = Ball::new(0, 10.0, 15.0, 8.0, 8.0, BallKind::Normal, 1.0);
    let (cx, cy) = primer_rebote(&mut b, &g, 600);
    assert!(cy, "no invirtio vy al chocar con la pared inferior");
    assert!(!cx, "INVIRTIO TAMBIEN vx: rebote de esquina en pared plana");
}

/// Muro INTERIOR consolidado (no el borde de la arena): mismo criterio.
#[test]
fn muro_interior_vertical_invierte_solo_vx() {
    let mut g = arena();
    for y in 0..20 {
        g.set(14, y, Cell::Filled);
    }
    let mut b = Ball::new(0, 10.0, 6.0, 8.0, 8.0, BallKind::Normal, 1.0);
    let (cx, cy) = primer_rebote(&mut b, &g, 600);
    assert!(cx, "no invirtio vx al chocar con el muro interior");
    assert!(!cy, "INVIRTIO TAMBIEN vy contra un muro interior plano");
}

/// La bola NO debe ciclarse en una trayectoria pobre con una salida normal.
///
/// Matiz importante: una diagonal de 45° EXACTA lanzada desde el centro de
/// una caja cuadrada recorre siempre la misma línea. Eso es geometría, no un
/// fallo: reflejar una diagonal en paredes ortogonales devuelve otra diagonal,
/// y con esa simetría perfecta la órbita se cierra. Por eso los niveles no
/// colocan las bolas en posiciones simétricas (ver `tools/gen_levels.py`).
///
/// Lo que sí sería un fallo es que la bola se plegara sobre sus pasos desde
/// una posición cualquiera, que es lo que ocurría cuando el rebote contra una
/// pared plana invertía las dos componentes.
#[test]
fn trayectoria_no_se_cicla_sobre_si_misma() {
    let g = arena();
    let mut rng = Rng64::new(5);
    // Punto de partida no simétrico, como los de los niveles reales.
    let mut b = Ball::new(0, 7.3, 11.8, 9.0, 9.0, BallKind::Normal, 1.0);

    let mut visitadas = std::collections::HashSet::new();
    for _ in 0..1800 {
        b.step(&g, &mut rng, DT);
        visitadas.insert((b.pos.x.floor() as i32, b.pos.y.floor() as i32));
    }

    assert!(
        visitadas.len() > 40,
        "la bola se ciclo: solo visito {} celdas distintas en 30 segundos",
        visitadas.len()
    );
}

/// Con una diagonal pura, cada rebote debe cambiar UNA sola componente.
/// Este test recorre una partida larga y verifica que NUNCA se invierten las
/// dos a la vez salvo que la bola esté de verdad en una esquina.
#[test]
fn nunca_invierte_ambos_ejes_fuera_de_una_esquina() {
    let g = arena();
    let mut rng = Rng64::new(11);
    let mut b = Ball::new(0, 6.7, 13.2, 8.0, 8.0, BallKind::Normal, 1.0);
    let r = b.radius;

    for frame in 0..3000 {
        let vx0 = b.vel.x;
        let vy0 = b.vel.y;
        b.step(&g, &mut rng, DT);
        let cx = vx0.signum() != b.vel.x.signum();
        let cy = vy0.signum() != b.vel.y.signum();
        if cx && cy {
            // Sólo es aceptable si toca dos paredes a la vez (esquina real).
            let en_pared_x = b.pos.x - r <= 0.05 || b.pos.x + r >= 20.0 - 0.05;
            let en_pared_y = b.pos.y - r <= 0.05 || b.pos.y + r >= 20.0 - 0.05;
            assert!(
                en_pared_x && en_pared_y,
                "frame {frame}: invirtio AMBOS ejes sin estar en una esquina \
                 (pos = {:.3}, {:.3})",
                b.pos.x,
                b.pos.y
            );
        }
    }
}
