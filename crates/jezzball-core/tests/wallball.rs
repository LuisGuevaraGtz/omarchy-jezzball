//! Mandatory balls-against-walls and obstacles tests (ARCHITECTURE.md §3):
//! shield (1 absorption + destruction), Heavy ignoring the shield, and Mover
//! destroying walls WITHOUT costing lives.

mod common;

use common::{ball, frames, spec, DT};
use jezzball_core::level::{BallKind, Obstacle};
use jezzball_core::powerup::PowerUpKind;
use jezzball_core::state::{step, GameEvent, GamePhase, GameState, PlayerInput};
use jezzball_core::wall::WallAxis;

/// Shield: a ball crosses the wall under construction several times while it
/// grows slowly. The FIRST pass is absorbed by the shield (wall intact, lives
/// intact); the SECOND destroys the wall and costs a life.
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
        "the shield stays pending for the next wall"
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
    assert_eq!(s.builders.len(), 1, "the wall is still being built");
    assert!(
        e.contains(&GameEvent::WallStarted),
        "the wall has started: {e:?}"
    );

    // We advance UP TO the moment when the ball's first pass consumes the
    // shield (wall alive, life intact), without depending on exact frames.
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
    assert!(absorbed, "the shield must absorb the first pass");
    assert_eq!(s.lives, 3, "the shield absorbs at no cost in lives");
    assert!(
        !s.builders.is_empty(),
        "the wall has NOT been destroyed yet"
    );
    assert!(!all.contains(&GameEvent::LifeLost));

    // Second pass: destroys the wall and costs a life.
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
    assert!(s.builders.is_empty(), "the wall was destroyed");
    assert_eq!(
        all.iter().filter(|e| **e == GameEvent::LifeLost).count(),
        1,
        "exactly one life lost"
    );
    assert!(
        !all.iter()
            .any(|e| matches!(e, GameEvent::WallCompleted { .. })),
        "the wall must NOT have consolidated"
    );
    assert_eq!(s.phase, GamePhase::Running);
}

/// Heavy ignores the shield: it destroys the wall under construction on the
/// FIRST pass, even if it is protected.
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
    assert!(s.builders.is_empty(), "Heavy destroys it on the first pass");
    assert!(!e
        .iter()
        .any(|e| matches!(e, GameEvent::WallCompleted { .. })));

    let (s, evs) = frames(&s, 100);
    assert_eq!(s.phase, GamePhase::Running);
    assert!(!evs
        .iter()
        .any(|e| matches!(e, GameEvent::WallCompleted { .. })));
}

/// Mover: the obstacle crosses the wall under construction and destroys it
/// WITHOUT costing lives (lives are exclusive to ball impacts, §3).
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
        // A static ball, far away: avoids the "empty arena" defeat
        // (check_end: no balls => GameOver) without interfering with the Mover.
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
        "the Mover does not cost lives"
    );
    assert!(!all
        .iter()
        .any(|e| matches!(e, GameEvent::WallCompleted { .. })));
    assert_eq!(s.lives, 3);
    assert!(s.builders.is_empty(), "the wall was destroyed by the Mover");
    assert_eq!(s.phase, GamePhase::Running);
}
