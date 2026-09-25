//! Regla del JezzBall original: cada mitad del muro se "convierte" (se fija)
//! en cuanto su frente toca una pared, y a partir de ese momento es INMUNE.
//! Sólo la mitad que todavía está creciendo puede costar una vida.
//!
//! Antes el muro era todo-o-nada: una bola tocando cualquier punto destruía
//! la línea completa, incluida la parte ya anclada en la pared.

use jezzball_core::grid::Cell;
use jezzball_core::level::{ArenaShape, ArenaSpec, BallKind, BallSpawn, LevelKind, LevelSpec};
use jezzball_core::state::{step, GameEvent, GameState, PlayerInput};
use jezzball_core::wall::WallAxis;

const DT: f32 = 1.0 / 60.0;

fn spec(w: u16, h: u16, balls: Vec<BallSpawn>, wall_speed: f32) -> LevelSpec {
    LevelSpec {
        id: 1,
        name: "t".into(),
        world: 1,
        kind: LevelKind::Standard,
        arena: ArenaSpec {
            w,
            h,
            shape: ArenaShape::Rect,
            obstacles: vec![],
        },
        balls,
        target_ratio: 0.9,
        lives: 3,
        wall_speed,
        time_limit: None,
        powerups_enabled: false,
        objectives: vec![],
        purist: false,
        seed: 3,
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

/// Una bola quieta (velocidad mínima) colocada sobre la mitad ya fijada no
/// debe costar vidas.
#[test]
fn la_mitad_anclada_en_la_pared_es_inmune() {
    // Arena ancha y baja. Muro VERTICAL desde una fila cercana al borde
    // superior: el frente `lo` llega arriba enseguida y se fija; el frente
    // `hi` sigue bajando mucho rato.
    let st = GameState::new(spec(30, 40, vec![bola(25.0, 30.0, 0.5, 0.5)], 6.0));
    let (st, _) = step(
        &st,
        PlayerInput::StartWall {
            cell: (10, 2),
            axis: WallAxis::Vertical,
        },
        DT,
    );

    // Avanzamos lo justo para que el frente `lo` toque el borde superior.
    let mut st = st;
    for _ in 0..60 {
        st = step(&st, PlayerInput::None, DT).0;
    }
    assert_eq!(st.builders.len(), 1, "el muro deberia seguir creciendo");
    let b = &st.builders[0];
    assert!(b.lo_done, "el frente lo deberia haber tocado el borde ya");
    assert!(!b.hi_done, "el frente hi deberia seguir creciendo");

    // La mitad superior (filas 0..=2 de la columna 10) ya debe estar FIJADA
    // en la rejilla, no simplemente "pintada".
    for y in 0..2u16 {
        assert_eq!(
            st.arena.grid.get(10, y),
            Cell::Filled,
            "la celda (10, {y}) de la mitad anclada deberia estar fijada"
        );
    }

    // Y no debe figurar como vulnerable.
    let vulnerable = st.builders[0].vulnerable_cells(&st.arena.grid);
    for y in 0..2u16 {
        assert!(
            !vulnerable.contains(&(10, y)),
            "la celda (10, {y}) sigue siendo vulnerable pese a estar anclada"
        );
    }
    assert!(
        !vulnerable.is_empty(),
        "la mitad que aun crece deberia ser vulnerable"
    );
}

/// Si la bola golpea la mitad que AÚN CRECE, se pierde una vida pero la
/// mitad ya anclada permanece en la arena.
#[test]
fn al_golpear_la_mitad_viva_sobrevive_la_anclada() {
    // Bola situada abajo, en la columna del muro, viajando hacia él.
    let st = GameState::new(spec(30, 40, vec![bola(10.5, 30.0, 0.0001, -9.0)], 6.0));
    let (st, _) = step(
        &st,
        PlayerInput::StartWall {
            cell: (10, 2),
            axis: WallAxis::Vertical,
        },
        DT,
    );

    let mut st = st;
    let mut vida_perdida = false;
    for _ in 0..900 {
        let (next, events) = step(&st, PlayerInput::None, DT);
        st = next;
        if events.iter().any(|e| matches!(e, GameEvent::LifeLost)) {
            vida_perdida = true;
            break;
        }
        if st.builders.is_empty() {
            break;
        }
    }

    assert!(
        vida_perdida,
        "la bola deberia haber golpeado la mitad viva y costado una vida"
    );
    assert_eq!(st.lives, 2, "deberia quedar exactamente una vida menos");

    // LA CLAVE: la mitad superior, que ya estaba anclada en la pared, sigue
    // ahí. Antes se borraba junto con el resto del muro.
    for y in 0..2u16 {
        assert_eq!(
            st.arena.grid.get(10, y),
            Cell::Filled,
            "la mitad anclada (10, {y}) se perdio al golpear la otra mitad"
        );
    }
}

/// Cuando ambos frentes llegan a su límite, el muro se consolida y parte el
/// área como siempre (no rompemos el comportamiento existente).
#[test]
fn with_both_halves_anchored_the_wall_splits_the_area() {
    // Las dos bolas viven a la derecha; el muro vertical en la columna 8
    // debe cerrar la franja izquierda.
    let st = GameState::new(spec(
        30,
        20,
        vec![bola(20.0, 8.0, 6.0, 6.0), bola(24.0, 12.0, -6.0, 6.0)],
        24.0,
    ));
    let (st, _) = step(
        &st,
        PlayerInput::StartWall {
            cell: (8, 10),
            axis: WallAxis::Vertical,
        },
        DT,
    );

    let mut st = st;
    let mut completado = false;
    for _ in 0..1200 {
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
    assert!(completado, "el muro no se consolido");

    // La columna del muro entera debe estar fijada.
    for y in 0..20u16 {
        assert_eq!(
            st.arena.grid.get(8, y),
            Cell::Filled,
            "la columna del muro no quedo completa en y={y}"
        );
    }
    // Y la franja izquierda (sin bolas) debe haberse cerrado.
    let mut abiertas_izq = 0;
    for y in 0..20u16 {
        for x in 0..8u16 {
            if st.arena.grid.get(x, y) == Cell::Open {
                abiertas_izq += 1;
            }
        }
    }
    assert_eq!(
        abiertas_izq, 0,
        "la franja izquierda no se cerro: {abiertas_izq} celdas abiertas"
    );
}

/// Un muro horizontal se comporta igual (la regla no depende del eje).
#[test]
fn the_rule_holds_for_horizontal_walls() {
    let st = GameState::new(spec(40, 30, vec![bola(30.0, 25.0, 0.5, 0.5)], 6.0));
    let (st, _) = step(
        &st,
        PlayerInput::StartWall {
            cell: (2, 10),
            axis: WallAxis::Horizontal,
        },
        DT,
    );

    let mut st = st;
    for _ in 0..60 {
        st = step(&st, PlayerInput::None, DT).0;
    }
    assert_eq!(st.builders.len(), 1);
    assert!(
        st.builders[0].lo_done,
        "el frente lo deberia haber tocado el borde izquierdo"
    );
    for x in 0..2u16 {
        assert_eq!(
            st.arena.grid.get(x, 10),
            Cell::Filled,
            "la celda ({x}, 10) de la mitad anclada deberia estar fijada"
        );
    }
}
