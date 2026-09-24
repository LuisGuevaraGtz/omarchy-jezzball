//! Tests obligatorios de bolas-contra-muros y obstáculos (ARCHITECTURE.md §3):
//! escudo (1 absorción + destrucción), Heavy ignorando el escudo y Mover
//! destruyendo muros SIN coste de vidas.

mod common;

use common::{ball, frames, spec, DT};
use jezzball_core::level::{BallKind, Obstacle};
use jezzball_core::powerup::PowerUpKind;
use jezzball_core::state::{step, GameEvent, GamePhase, GameState, PlayerInput};
use jezzball_core::wall::WallAxis;

/// Escudo: una bola cruza el muro en construcción varias veces mientras este
/// se alarga despacio. La PRIMERA pasada la absorbe el escudo (muro intacto,
/// vidas intactas); la SEGUNDA destruye el muro y cuesta una vida.
#[test]
fn shield_two_impacts() {
    let mut s = GameState::new(spec(
        30,
        10,
        vec![],
        vec![ball(15.5, 5.0, -7.5, 0.0, BallKind::Normal)],
        0.5,
        0.99,
        3,
        false,
        vec![],
        false,
    ));
    s.inventory.push(PowerUpKind::Shield);
    let (n, _) = step(&s, PlayerInput::UsePowerUp(PowerUpKind::Shield), DT);
    s = n;
    assert!(
        s.pending_shield,
        "el escudo queda pendiente del próximo muro"
    );

    let (n, e) = step(
        &s,
        PlayerInput::StartWall {
            cell: (15, 0),
            axis: WallAxis::Vertical,
        },
        DT,
    );
    s = n;
    assert_eq!(s.lives, 3);
    assert_eq!(s.builders.len(), 1, "el muro sigue construyéndose");
    assert!(
        e.contains(&GameEvent::WallStarted),
        "el muro ha arrancado: {e:?}"
    );

    // Avanzamos HASTA el momento en que la primera pasada de la bola consume
    // el escudo (muro vivo, vida intacta), sin depender de frames exactos.
    let mut all = Vec::new();
    let mut absorbed = false;
    for _ in 0..800 {
        if s.phase != GamePhase::Running {
            break;
        }
        let was_shielded = s.builders.iter().any(|b| b.shielded);
        let (n, e) = step(&s, PlayerInput::None, DT);
        s = n;
        all.extend(e);
        let wall_alive = !s.builders.is_empty();
        let shield_gone = s.builders.iter().any(|b| !b.shielded);
        if was_shielded && wall_alive && shield_gone {
            absorbed = true;
            break;
        }
    }
    assert!(absorbed, "el escudo debe absorver la primera pasada");
    assert_eq!(s.lives, 3, "el escudo absorbe sin coste de vidas");
    assert!(!s.builders.is_empty(), "el muro NO se ha destruido aún");
    assert!(!all.contains(&GameEvent::LifeLost));

    // Segunda pasada: destruye el muro y cuesta una vida.
    for _ in 0..800 {
        if s.phase != GamePhase::Running {
            break;
        }
        let (n, e) = step(&s, PlayerInput::None, DT);
        s = n;
        all.extend(e);
        if all.iter().filter(|e| **e == GameEvent::LifeLost).count() >= 1 {
            break;
        }
    }
    assert_eq!(s.lives, 2);
    assert!(s.builders.is_empty(), "el muro quedó destruido");
    assert_eq!(
        all.iter().filter(|e| **e == GameEvent::LifeLost).count(),
        1,
        "exactamente una vida perdida"
    );
    assert!(
        !all.iter()
            .any(|e| matches!(e, GameEvent::WallCompleted { .. })),
        "el muro NO debe haberse consolidado"
    );
    assert_eq!(s.phase, GamePhase::Running);
}

/// Heavy ignora el escudo: destruye el muro en construcción a la PRIMERA
/// pasada, aunque esté protegido.
#[test]
fn heavy_ignores_shield() {
    let mut s = GameState::new(spec(
        30,
        10,
        vec![],
        vec![ball(15.5, 0.5, 0.0, 0.0, BallKind::Heavy)],
        22.0,
        0.9,
        3,
        false,
        vec![],
        false,
    ));
    s.inventory.push(PowerUpKind::Shield);
    let (n, _) = step(&s, PlayerInput::UsePowerUp(PowerUpKind::Shield), DT);
    s = n;

    let (s, e) = step(
        &s,
        PlayerInput::StartWall {
            cell: (15, 0),
            axis: WallAxis::Vertical,
        },
        DT,
    );

    assert!(e.contains(&GameEvent::LifeLost));
    assert_eq!(s.lives, 2);
    assert!(s.builders.is_empty(), "Heavy destruye a la primera pasada");
    assert!(!e
        .iter()
        .any(|e| matches!(e, GameEvent::WallCompleted { .. })));

    let (s, evs) = frames(&s, 100);
    assert_eq!(s.phase, GamePhase::Running);
    assert!(!evs
        .iter()
        .any(|e| matches!(e, GameEvent::WallCompleted { .. })));
}

/// Mover: el obstáculo cruza el muro en construcción y lo destruye SIN coste
/// de vidas (las vidas son exclusivas de los impactos de bola, §3).
#[test]
fn mover_destroys_wall_without_life_loss() {
    let s = GameState::new(spec(
        30,
        10,
        vec![Obstacle::Mover {
            x: 2.0,
            y: 2.0,
            w: 12,
            h: 1,
            vx: 20.0,
            vy: 0.0,
        }],
        // Una bola estática, lejos: evita la derrota por "arena vacía"
        // (check_end: sin bolas => GameOver) sin interferir con el Mover.
        vec![ball(27.0, 8.0, 0.0, 0.0, BallKind::Normal)],
        20.0,
        0.99,
        3,
        false,
        vec![],
        false,
    ));
    let mut all = Vec::new();
    let (s, e) = step(
        &s,
        PlayerInput::StartWall {
            cell: (20, 2),
            axis: WallAxis::Horizontal,
        },
        DT,
    );
    all.extend(e);
    let (s, e) = frames(&s, 60);
    all.extend(e);

    assert!(all.iter().any(|e| matches!(e, GameEvent::WallStarted)));
    assert!(all.iter().any(|e| matches!(e, GameEvent::WallBlocked)));
    assert!(
        !all.contains(&GameEvent::LifeLost),
        "el Mover no cuesta vidas"
    );
    assert!(!all
        .iter()
        .any(|e| matches!(e, GameEvent::WallCompleted { .. })));
    assert_eq!(s.lives, 3);
    assert!(
        s.builders.is_empty(),
        "el muro quedó destruido por el Mover"
    );
    assert_eq!(s.phase, GamePhase::Running);
}
