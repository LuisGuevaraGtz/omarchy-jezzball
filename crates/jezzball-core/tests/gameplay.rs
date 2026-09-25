//! Mandatory gameplay tests (ARCHITECTURE.md §15):
//! default game, victory, small-vs-large scoring, combo and determinism
//! (600 steps).

mod common;

use common::{ball, frames, spec, DT};
use jezzball_core::level::{BallKind, Objective};
use jezzball_core::powerup::PowerUpKind;
use jezzball_core::state::{step, GameEvent, GamePhase, GameState, PlayerInput};
use jezzball_core::wall::WallAxis;

/// The default game starts in Running and loses neither lives nor points.
#[test]
fn default_running() {
    let s = GameState::new(spec(
        30,
        10,
        vec![],
        vec![ball(15.0, 5.0, 2.0, 1.0, BallKind::Normal)],
        22.0,
        0.9,
        3,
        false,
        vec![],
        false,
    ));
    let (s, evs) = frames(&s, 50);

    assert_eq!(s.phase, GamePhase::Running);
    assert_eq!(s.lives, 3);
    assert_eq!(s.score, 0);
    assert!(evs.is_empty(), "no events with `None`: {evs:?}");
    assert_eq!(s.balls.len(), 1);
}

/// Three walls close 3 regions and the resulting area exceeds `target_ratio`
/// (0.75): `LevelCleared` fires with the star for the objective met.
#[test]
fn level_cleared() {
    let mut s = GameState::new(spec(
        20,
        10,
        vec![],
        vec![
            ball(19.0, 1.0, 0.0, 0.0, BallKind::Normal),
            ball(19.0, 8.0, 0.0, 0.0, BallKind::Normal),
        ],
        30.0,
        0.75,
        3,
        false,
        vec![Objective::ClearRatio(0.8)],
        false,
    ));
    let mut all = Vec::new();

    for col in [5u16, 10, 15] {
        let (n, e) = step(
            &s,
            PlayerInput::StartWall {
                cell: (col, 0),
                axis: WallAxis::Vertical,
            },
            DT,
        );
        s = n;
        all.extend(e);
        let (n, e) = frames(&s, 30);
        s = n;
        all.extend(e);
    }

    assert!(all
        .iter()
        .any(|e| matches!(e, GameEvent::LevelCleared { stars: 1 })));
    assert_eq!(s.phase, GamePhase::Won);
    assert_eq!(s.stars, 1);
    assert!(s.score > 0);
    assert!(
        s.arena.grid.filled_ratio() >= 0.75,
        "ratio = {}",
        s.arena.grid.filled_ratio()
    );
}

/// Risk/reward: closing a tiny pocket pays MORE than closing a large region
/// (same `balls_near` and speed: static balls far away).
#[test]
fn small_vs_large_scoring() {
    let small = single_wall_points(5, 0);
    let large = single_wall_points(24, 0);

    assert_eq!(small.0, 50, "5-column pocket");
    assert_eq!(large.0, 240, "24-column pocket");
    assert!(
        small.1 > large.1,
        "closing {} cells ({}) must pay more than {} cells ({})",
        small.0,
        small.1,
        large.0,
        large.1
    );
}

/// Closes exactly ONE region with a vertical wall and returns
/// `(closed_cells, points)`.
fn single_wall_points(col: u16, _row: u16) -> (u32, u32) {
    let s = GameState::new(spec(
        30,
        10,
        vec![],
        vec![
            ball(29.0, 2.0, 0.0, 0.0, BallKind::Normal),
            ball(29.0, 8.0, 0.0, 0.0, BallKind::Normal),
        ],
        30.0,
        0.99,
        3,
        false,
        vec![],
        false,
    ));
    let (s, e) = step(
        &s,
        PlayerInput::StartWall {
            cell: (col, 0),
            axis: WallAxis::Vertical,
        },
        DT,
    );
    let (_, evs) = frames(&s, 200);
    let mut completed = Vec::new();
    for e in e.into_iter().chain(evs) {
        if let GameEvent::WallCompleted { points, cells } = e {
            completed.push((cells, points));
        }
    }
    assert_eq!(completed.len(), 1, "only one wall must complete");
    completed[0]
}

/// Two consecutive walls inside the window (4 s) raise the combo to x3, and it
/// only resets when the window expires without consolidating more walls.
#[test]
fn combo() {
    let mut s = GameState::new(spec(
        40,
        10,
        vec![],
        vec![
            ball(38.0, 4.0, 0.0, 0.0, BallKind::Normal),
            ball(38.0, 8.0, 0.0, 0.0, BallKind::Normal),
        ],
        30.0,
        0.9,
        3,
        false,
        vec![],
        false,
    ));
    let mut all = Vec::new();

    let (n, e) = step(
        &s,
        PlayerInput::StartWall {
            cell: (4, 0),
            axis: WallAxis::Vertical,
        },
        DT,
    );
    s = n;
    all.extend(e);
    let (n, e) = frames(&s, 40);
    s = n;
    all.extend(e);

    // Second wall: horizontal on row 2, which can close the upper region
    // (no balls) and still fall inside the combo window.
    let (n, e) = step(
        &s,
        PlayerInput::StartWall {
            cell: (20, 2),
            axis: WallAxis::Horizontal,
        },
        DT,
    );
    s = n;
    all.extend(e);
    let (n, e) = frames(&s, 40);
    s = n;
    all.extend(e);

    assert!(all.iter().any(|e| matches!(e, GameEvent::ComboUp(2))));
    assert!(all.iter().any(|e| matches!(e, GameEvent::ComboUp(3))));
    assert_eq!(s.combo.multiplier, 3);
    assert_eq!(s.max_combo_reached, 3);
    assert_ne!(s.phase, GamePhase::Won);

    // The window expires with no new walls: multiplier back to x1 + ComboReset.
    let (s, evs) = frames(&s, 160);
    assert!(evs.contains(&GameEvent::ComboReset));
    assert_eq!(s.combo.multiplier, 1);
}

/// Determinism: same seed + same input over 600 steps => identical state
/// (includes an `Erratic` ball and power-ups to exercise the PRNG).
#[test]
fn determinism_600_steps() {
    let spec = spec(
        24,
        12,
        vec![],
        vec![
            ball(4.0, 6.0, 5.0, 3.0, BallKind::Normal),
            ball(20.0, 6.0, -5.0, 4.0, BallKind::Normal),
            ball(8.0, 2.0, 3.0, 2.0, BallKind::Erratic),
        ],
        16.0,
        0.99,
        3,
        true,
        vec![],
        false,
    );
    let script = vec![
        (
            0u32,
            PlayerInput::StartWall {
                cell: (3, 5),
                axis: WallAxis::Vertical,
            },
        ),
        (25, PlayerInput::UsePowerUp(PowerUpKind::Freeze)),
        (
            60,
            PlayerInput::StartWall {
                cell: (12, 2),
                axis: WallAxis::Horizontal,
            },
        ),
        (100, PlayerInput::UsePowerUp(PowerUpKind::Shield)),
    ];

    let a = run_script(&spec, &script);
    let b = run_script(&spec, &script);

    assert_eq!(a, b, "two runs with the same seed must match");
}

/// Runs the script (input on the exact frame, padded with `None`) until
/// 600 steps of `DT` are completed. Returns the final state.
fn run_script(spec: &jezzball_core::level::LevelSpec, script: &[(u32, PlayerInput)]) -> GameState {
    let mut s = GameState::new(spec.clone());
    // Deterministic inventory to exercise `UsePowerUp`.
    s.inventory.push(PowerUpKind::Freeze);
    s.inventory.push(PowerUpKind::Shield);

    let mut frame = 0u32;
    for &(at, input) in script {
        while frame < at && s.phase == GamePhase::Running {
            let (n, _) = step(&s, PlayerInput::None, DT);
            s = n;
            frame += 1;
        }
        if s.phase == GamePhase::Running {
            let (n, _) = step(&s, input, DT);
            s = n;
            frame += 1;
        }
    }
    while frame < 600 && s.phase == GamePhase::Running {
        let (n, _) = step(&s, PlayerInput::None, DT);
        s = n;
        frame += 1;
    }
    s
}
