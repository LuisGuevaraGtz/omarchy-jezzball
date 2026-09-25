//! Partición y cierre de área (ARCHITECTURE.md §4).
//!
//! Es el corazón del JezzBall: al consolidarse un muro, las regiones sin bola
//! se cierran. Un fallo aquí se nota inmediatamente al jugar (área que no se
//! cierra, o que se cierra con una bola inside).

use jezzball_core::grid::Cell;
use jezzball_core::level::{
    ArenaShape, ArenaSpec, BallKind, BallSpawn, LevelKind, LevelSpec, Objective,
};
use jezzball_core::state::{step, GameEvent, GameState, PlayerInput};
use jezzball_core::wall::WallAxis;

const DT: f32 = 1.0 / 60.0;

fn spec_con(w: u16, h: u16, balls: Vec<BallSpawn>, purist: bool) -> LevelSpec {
    LevelSpec {
        id: 1,
        name: "t".into(),
        world: if purist { 0 } else { 1 },
        kind: LevelKind::Standard,
        arena: ArenaSpec {
            w,
            h,
            shape: ArenaShape::Rect,
            obstacles: vec![],
        },
        balls,
        target_ratio: 0.75,
        lives: 3,
        wall_speed: 22.0,
        time_limit: None,
        powerups_enabled: false,
        objectives: vec![Objective::NoLivesLost],
        purist,
        seed: 7,
    }
}

fn bola(x: f32, y: f32, vx: f32, vy: f32) -> BallSpawn {
    BallSpawn {
        x,
        y,
        vx,
        vy,
        kind: BallKind::Normal,
        radius_mul: 1.0,
    }
}

/// Avanza hasta que el muro se consolida, devolviendo el estado y si hubo
/// evento `WallCompleted`.
fn hasta_consolidar(mut st: GameState, max_frames: usize) -> (GameState, bool) {
    let mut completado = false;
    for _ in 0..max_frames {
        let (next, events) = step(&st, PlayerInput::None, DT);
        st = next;
        if events
            .iter()
            .any(|e| matches!(e, GameEvent::WallCompleted { .. }))
        {
            completado = true;
            break;
        }
    }
    (st, completado)
}

/// Una región sin bolas se cierra por completo; la que tiene bola, no.
#[test]
fn closes_only_the_region_without_balls() {
    // Arena 40x20. Las dos bolas viven en la MITAD DERECHA (x > 20).
    let spec = spec_con(
        40,
        20,
        vec![bola(30.0, 8.0, 6.0, 6.0), bola(34.0, 12.0, -6.0, 6.0)],
        false,
    );
    let st = GameState::new(spec);
    // Muro vertical en la columna 20: parte la arena en dos mitades.
    let (st, _) = step(
        &st,
        PlayerInput::StartWall {
            cell: (20, 10),
            axis: WallAxis::Vertical,
        },
        DT,
    );
    let (st, completado) = hasta_consolidar(st, 2000);
    assert!(completado, "el muro nunca se consolido");

    // La mitad izquierda (sin bolas) debe quedar cerrada.
    let g = &st.arena.grid;
    let mut izq_abiertas = 0;
    for y in 0..20u16 {
        for x in 0..20u16 {
            if g.get(x, y) == Cell::Open {
                izq_abiertas += 1;
            }
        }
    }
    assert_eq!(
        izq_abiertas, 0,
        "la mitad izquierda no se cerro: quedan {izq_abiertas} celdas abiertas"
    );

    // La mitad derecha (con las bolas) debe seguir abierta en su mayoría.
    let mut der_abiertas = 0;
    for y in 0..20u16 {
        for x in 21..40u16 {
            if g.get(x, y) == Cell::Open {
                der_abiertas += 1;
            }
        }
    }
    assert!(
        der_abiertas > 200,
        "la mitad derecha se cerro con bolas inside: solo {der_abiertas} abiertas"
    );
}

/// Ninguna bola puede acabar inside de una celda cerrada. Es la invariante
/// más importante del cierre: si se rompe, la bola queda enterrada.
#[test]
fn no_ball_ends_up_inside_a_closed_cell() {
    let spec = spec_con(
        40,
        24,
        vec![
            bola(8.0, 6.0, 7.0, 7.0),
            bola(30.0, 16.0, -7.0, 7.0),
            bola(20.0, 12.0, 7.0, -7.0),
        ],
        false,
    );
    let mut st = GameState::new(spec);

    // Trazamos muros repetidamente en posiciones variadas.
    let mut frame = 0usize;
    for ronda in 0..40 {
        let cell = ((5 + (ronda * 3) % 30) as u16, (4 + (ronda * 5) % 18) as u16);
        let axis = if ronda % 2 == 0 {
            WallAxis::Vertical
        } else {
            WallAxis::Horizontal
        };
        let (next, _) = step(&st, PlayerInput::StartWall { cell, axis }, DT);
        st = next;

        for _ in 0..300 {
            let (next, _) = step(&st, PlayerInput::None, DT);
            st = next;
            frame += 1;

            // INVARIANTE: la celda de cada bola nunca puede estar cerrada.
            for b in &st.balls {
                let (cx, cy) = (b.pos.x.floor() as i64, b.pos.y.floor() as i64);
                if st.arena.grid.in_bounds(cx, cy) {
                    let c = st.arena.grid.get(cx as u16, cy as u16);
                    assert_ne!(
                        c,
                        Cell::Filled,
                        "frame {frame}: bola enterrada en celda Filled ({cx}, {cy})"
                    );
                    assert_ne!(
                        c,
                        Cell::Solid,
                        "frame {frame}: bola enterrada en celda Solid ({cx}, {cy})"
                    );
                }
            }
            if st.builders.is_empty() {
                break;
            }
        }
    }
}

/// El porcentaje de área conquistada nunca debe retroceder ni pasar de 1.0.
/// Es lo que muestra el HUD: si oscila, el jugador ve un número absurdo.
#[test]
fn el_area_conquistada_es_monotona_y_valida() {
    let spec = spec_con(
        36,
        20,
        vec![bola(10.0, 6.0, 7.0, 7.0), bola(26.0, 14.0, -7.0, 7.0)],
        false,
    );
    let mut st = GameState::new(spec);
    let mut anterior = st.arena.grid.filled_ratio();

    for ronda in 0..25 {
        let cell = ((4 + (ronda * 4) % 28) as u16, (3 + (ronda * 3) % 15) as u16);
        let axis = if ronda % 2 == 0 {
            WallAxis::Horizontal
        } else {
            WallAxis::Vertical
        };
        let (next, _) = step(&st, PlayerInput::StartWall { cell, axis }, DT);
        st = next;

        for _ in 0..300 {
            let (next, _) = step(&st, PlayerInput::None, DT);
            st = next;
            let r = st.arena.grid.filled_ratio();
            assert!(
                r.is_finite() && (0.0..=1.0).contains(&r),
                "filled_ratio fuera de rango: {r}"
            );
            assert!(
                r >= anterior - 1e-4,
                "el area conquistada RETROCEDIO: {anterior} -> {r}"
            );
            anterior = r;
            if st.builders.is_empty() {
                break;
            }
        }
    }
}
