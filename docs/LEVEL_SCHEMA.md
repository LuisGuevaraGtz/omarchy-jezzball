# Appendix A — Level data schema (normative)

Level workers and the `jezzball-core` crate MUST use exactly these types.
Any change has to propagate to both sides.

## Rust types (defined in `jezzball-core/src/level.rs`)

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LevelSpec {
    pub id: u16,                  // 1-based, unique within the mode
    pub name: String,             // short, displayable in the HUD
    pub world: u8,                // 0 = Original; 1..=6 = Enhanced worlds
    pub kind: LevelKind,
    pub arena: ArenaSpec,
    pub balls: Vec<BallSpawn>,
    pub target_ratio: f32,        // 0.0..1.0, typically 0.75
    pub lives: u8,
    pub wall_speed: f32,          // cells/sec per front, typically 22.0
    pub time_limit: Option<f32>,  // seconds; None = no clock
    pub powerups_enabled: bool,
    pub objectives: Vec<Objective>, // max 3; empty in Original
    pub purist: bool,             // true => no combos, no power-ups, no objectives
    pub seed: u64,                // seed for the deterministic PRNG
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum LevelKind { Standard, Boss, SpeedRun, Chaos }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArenaSpec {
    pub w: u16,                   // cells, 32..=96
    pub h: u16,                   // cells, 20..=60
    pub shape: ArenaShape,
    pub obstacles: Vec<Obstacle>, // pre-placed Solid cells
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum ArenaShape {
    Rect,                         // standard rectangle
    Wide,                         // wide aspect ratio
    Tall,                         // tall aspect ratio
    Irregular { notch: u16 },     // corners notched by `notch` cells
    Circle,                       // inscribed ellipse; outside = Solid
    Maze { density: u8 },         // 0..=100, corridors generated from `seed`
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Obstacle {
    Block { x: u16, y: u16, w: u16, h: u16 },
    // moves in a straight line, bouncing; destroys walls under construction
    Mover { x: f32, y: f32, w: u16, h: u16, vx: f32, vy: f32 },
    // a zone that can NEVER be filled or sealed (counts as non-fillable)
    NoSplit { x: u16, y: u16, w: u16, h: u16 },
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct BallSpawn {
    pub x: f32, pub y: f32,       // cells
    pub vx: f32, pub vy: f32,     // cells/sec
    pub kind: BallKind,
    pub radius_mul: f32,          // 1.0 = normal
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum BallKind { Normal, Fast, Erratic, Splitter, Heavy }

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Objective {
    ClearRatio(f32), NoLivesLost, UnderTime(f32),
    MinScore(u32), KeepCombo(u8), NoPowerUps,
}
```

## On-disk format

`assets/levels/original.ron` and `assets/levels/enhanced.ron` each contain a
`Vec<LevelSpec>` in RON. Minimal valid example:

```ron
[
    (
        id: 1,
        name: "Primeros Rebotes",
        world: 0,
        kind: Standard,
        arena: (w: 64, h: 40, shape: Rect, obstacles: []),
        balls: [
            (x: 20.0, y: 12.0, vx: 7.0, vy: 6.0, kind: Normal, radius_mul: 1.0),
            (x: 44.0, y: 28.0, vx: -6.0, vy: 7.0, kind: Normal, radius_mul: 1.0),
        ],
        target_ratio: 0.75,
        lives: 3,
        wall_speed: 22.0,
        time_limit: None,
        powerups_enabled: false,
        objectives: [],
        purist: true,
        seed: 1001,
    ),
]
```

## Generation rules

### Original (10 levels, `original.ron`)
- `world: 0`, `kind: Standard`, `purist: true`, `objectives: []`,
  `powerups_enabled: false`, `obstacles: []`, `shape: Rect`, `time_limit: None`.
- Balls: level N has `1 + N` balls (level 1 → 2 balls ... level 10 → 11).
  This reproduces the original JezzBall curve.
- All balls are `Normal`, `radius_mul: 1.0`.
- Base speed 8.0 cells/s at level 1, rising ~4% per level (capped at 11.5).
- `lives`: 3 on levels 1-3, 4 on 4-7, 5 on 8-10 (the original grants
  lives = level; we bound it so the game stays fair).
- `target_ratio: 0.75`, fixed.
- `wall_speed`: 22.0, fixed.
- Starting positions: spread out, never closer than 4 cells to an edge nor
  closer than 5 cells to each other. Velocities must have non-zero components
  and must not be exactly equal across balls (this avoids dull symmetries).
- `seed`: 1000 + id.

### Enhanced (60 levels, `enhanced.ron`)
`world = 1 + (id-1)/10`, i.e. ids 1-10 → world 1, 11-20 → world 2, and so on.
`purist: false` always. `seed`: 2000 + id.

Special levels within each world:
- level 5 of every world → `kind: SpeedRun` with `time_limit: Some(...)`.
- level 10 of every world → `kind: Boss` (worlds 1, 2, 4) or `Chaos`
  (worlds 3, 5, 6).

Per world:
- **W1 Classic (1-10)**: like Original but with `purist: false`, combos
  active, 1-2 objectives per level, no obstacles, `shape: Rect`. Balls 2→6.
- **W2 Speed (11-20)**: speeds 12-18 cells/s, some `Fast` balls,
  `time_limit` on half the levels, `wall_speed` 24-28.
  `shape` alternates Rect/Wide/Tall. Balls 3→7.
- **W3 Obstacles (21-30)**: 2-6 `Block`s per level, `Mover`s appear from 25
  onwards, one `NoSplit` from 27 onwards. `shape` includes Irregular.
  Balls 3→6. Lower `target_ratio` to 0.70 wherever there is a lot of `Solid`.
- **W4 Special balls (31-40)**: gradual introduction — 31-32 `Erratic`,
  33-34 `Splitter`, 35-36 `Heavy`, 37-40 mixed. Max 2 `Heavy` per level,
  max 2 `Splitter`. Mix in light obstacles. `shape` includes Circle.
- **W5 Power-ups (41-50)**: `powerups_enabled: true`, difficulty clearly
  above W4 so the power-up is a necessity rather than a gift.
  The `NoPowerUps` objective appears on 2-3 levels as an optional challenge.
  Varied `shape`.
- **W6 Advanced challenges (51-60)**: everything combined, `shape: Maze` on
  3+ levels, `target_ratio` up to 0.82, `lives` 2-3, 3 objectives per level,
  level 60 = the final `Chaos`, the hardest in the game.

Objectives: 2-3 per level in Enhanced, and consistent with the level itself
(don't ask for `UnderTime(30.0)` on a level that reasonably takes 90 s;
don't ask for `NoPowerUps` if `powerups_enabled: false`;
`ClearRatio` must be > `target_ratio`).

### Mandatory validations
No ball spawn inside an `Obstacle` or outside the arena shape.
No ball with `vx == 0.0 && vy == 0.0`.
`target_ratio` within 0.60..=0.85. `lives` within 1..=5. `wall_speed` within
18.0..=30.0.
`objectives.len() <= 3`. Consecutive ids with no gaps, starting at 1.
