//! The bonus level with the Omarchy signature must be genuinely playable, not
//! just parseable: its obstacles draw the logo, and a badly placed logo could
//! leave balls trapped inside a block or make the level impossible.

use jezzball_core::level::LevelSpec;
use jezzball_core::state::{step, GamePhase, GameState, PlayerInput};

const DT: f32 = 1.0 / 60.0;

fn load_omarchy_level() -> LevelSpec {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/levels/enhanced.ron"
    );
    let text = std::fs::read_to_string(path).expect("assets/levels/enhanced.ron is readable");
    let levels: Vec<LevelSpec> = ron::from_str(&text).expect("enhanced.ron parses");
    levels
        .into_iter()
        .find(|l| l.id == 61)
        .expect("level 61 exists (the Omarchy signature)")
}

#[test]
fn the_omarchy_level_starts_with_every_ball_on_an_open_cell() {
    let spec = load_omarchy_level();
    let s = GameState::new(spec);

    assert!(!s.balls.is_empty(), "the level must have balls");
    for (i, b) in s.balls.iter().enumerate() {
        let (cx, cy) = (b.pos.x as u16, b.pos.y as u16);
        assert!(
            s.arena.grid.is_open(cx, cy),
            "ball {i} spawns inside a logo obstacle, at cell ({cx},{cy})"
        );
    }
}

#[test]
fn the_logo_leaves_enough_playable_space() {
    let spec = load_omarchy_level();
    let (w, h) = (spec.arena.w as u32, spec.arena.h as u32);
    let s = GameState::new(spec);

    let free = s.arena.grid.open_count() as f32 / (w * h) as f32;

    // If the logo took up nearly the whole arena, the level would be unplayable.
    assert!(
        free > 0.45,
        "the logo leaves only {:.0}% of the arena free; the level would be unplayable",
        free * 100.0
    );
}

#[test]
fn the_omarchy_level_simulates_without_panic_and_balls_stay_inside() {
    let spec = load_omarchy_level();
    let (aw, ah) = (spec.arena.w as f32, spec.arena.h as f32);
    let mut s = GameState::new(spec);

    // 30 simulated seconds at 60 FPS: the balls bounce off the logo thousands
    // of times. If the logo geometry had a badly sealed gap, a ball would end
    // up outside the arena or inside a block.
    for _ in 0..1800 {
        let (next, _) = step(&s, PlayerInput::None, DT);
        s = next;

        if s.phase != GamePhase::Running {
            break;
        }
        for (i, b) in s.balls.iter().enumerate() {
            assert!(
                b.pos.x >= 0.0 && b.pos.y >= 0.0 && b.pos.x <= aw && b.pos.y <= ah,
                "ball {i} left the arena: ({}, {})",
                b.pos.x,
                b.pos.y
            );
            let (cx, cy) = (b.pos.x as u16, b.pos.y as u16);
            assert!(
                s.arena.grid.is_open(cx, cy),
                "ball {i} ended up inside a logo block, at ({cx},{cy})"
            );
        }
    }
}
