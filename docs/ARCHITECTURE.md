# Omarchy-Jezzball — Architecture Contract

Normative document. Every worker implements AGAINST this contract.
Anything not covered here is decided in the crate that uses it and documented
there.

## 0. Principles

- Native, lightweight, no Electron, no heavyweight engine, no extra runtime.
- Wayland-first: no mandatory X11 dependencies.
- Keyboard-first: all menus and meta-actions are keyboard-driven; the mouse
  only draws walls.
- Terminal/dark aesthetic, consistent with the active Omarchy theme.
- Game logic = deterministic, pure, testable, free of I/O and rendering.

## 1. Workspace

```
Omarchy-Jezzball/
├─ Cargo.toml                 # virtual workspace, members = core/app
├─ crates/
│  ├─ jezzball-core/          # PURE LOGIC. zero render/OS deps.
│  │  ├─ src/
│  │  │  ├─ lib.rs
│  │  │  ├─ geom.rs           # Vec2, Rect, Aabb, helpers
│  │  │  ├─ ball.rs           # Ball, BallKind, integration + bounce
│  │  │  ├─ wall.rs           # Wall, WallBuilder (wall under construction)
│  │  │  ├─ grid.rs           # per-cell occupancy + flood fill
│  │  │  ├─ arena.rs          # Arena, layouts, partitioning
│  │  │  ├─ level.rs          # LevelSpec, World, Objective, Star
│  │  │  ├─ score.rs          # risk/reward, combos
│  │  │  ├─ powerup.rs        # PowerUp, timed effects
│  │  │  └─ state.rs          # GameState + step() + apply_input()
│  │  └─ tests/
│  ├─ jezzball-app/           # macroquad: render, input, audio, save
│  │  └─ src/
│  │     ├─ main.rs
│  │     ├─ theme.rs          # reads the Omarchy theme
│  │     ├─ persist.rs        # XDG save/load
│  │     ├─ input.rs          # keyboard/mouse mapping -> PlayerInput
│  │     ├─ render/
│  │     │  ├─ mod.rs
│  │     │  ├─ arena.rs
│  │     │  ├─ hud.rs
│  │     │  └─ menu.rs
│  │     └─ screens.rs        # menu, game, pause, results
├─ assets/
│  ├─ levels/original.ron
│  ├─ levels/enhanced.ron
│  └─ fonts/
├─ packaging/
│  ├─ PKGBUILD
│  ├─ omarchy-jezzball.desktop
│  └─ install.sh
└─ docs/
```

Hard rule: `jezzball-core` MUST NOT depend on `macroquad`, `std::fs`,
`std::time`, or anything OS-specific. Only `serde` (opt-in) and arithmetic.
Time enters as `dt: f32`. Randomness enters as a `u64` seed fed to our own
deterministic PRNG (xorshift64*), never `rand` with OS entropy.

## 2. Arena model (hybrid grid + continuous)

The original JezzBall is grid-based. We use:

- **Logical grid** `Grid { w, h, cells: Vec<Cell> }` where
  `Cell = Open | Filled | Solid` (Solid = world-3 obstacle).
  This is the source of truth for "what is sealed" and for the area
  percentage.
- **Continuous balls** in cell coordinates using `f32` (pos, vel, radius),
  for smooth 60 FPS motion and correct diagonal bounces.

Cell size in pixels is decided by the render layer, NOT the core.
The core works in "cell units". Default arena: 64x40 cells.

### Ball↔grid collision
Semi-implicit integration with separate per-axis resolution:
1. move along X; if the destination cell is not `Open` → revert X and flip
   `vel.x`.
2. move along Y, identical.
This reproduces the classic 45° bounce without tunnelling as long as
`|vel| * dt < 0.5` cells. If `|vel| * dt >= 0.5`, the core performs internal
substeps (max. 8).

## 3. Walls

```rust
pub enum WallAxis { Horizontal, Vertical }

pub struct WallBuilder {
    pub axis: WallAxis,
    pub origin: (u16, u16),   // cell the player clicked
    pub lo: f32,              // front growing towards -X/-Y (in cells)
    pub hi: f32,              // front growing towards +X/+Y
    pub lo_done: bool,        // reached an edge/obstacle
    pub hi_done: bool,
    pub speed: f32,           // cells per second, per front
    pub shielded: bool,       // Shield power-up: absorbs 1 hit
}
```

Behaviour (identical to the original):
- Only one `WallBuilder` active at a time (unless the Double Wall power-up is
  in play → 2).
- It grows in BOTH directions simultaneously from `origin`.
- A front stops when it reaches `Filled`/`Solid`/an edge → `*_done = true`.
- Once `lo_done && hi_done` the wall **consolidates**: the cells along the
  segment become `Filled` and partitioning is triggered (section 4).
- If a ball touches ANY cell of a front that is still under construction:
  - if `shielded` → the shield is consumed and the wall survives.
  - otherwise → the wall is destroyed, `lives -= 1`, combo reset to 0.

## 4. Partitioning and area closing (the heart of it)

After a wall consolidates:
1. 4-connected flood fill over `Open` cells, labelling regions.
2. For each region, check whether it contains any ball (by the ball's cell).
3. Regions **with no ball** → all their cells become `Filled`.
4. Recompute `filled_ratio = filled_cells / fillable_cells`.
5. Score every sealed region (section 5).
6. `filled_ratio >= level.target_ratio` (0.75 by default) → level cleared.

Complexity: O(cells) per wall. 64x40 = 2560 cells → trivial at 60 FPS.

## 5. Risk/reward scoring

For each sealed region of `n` cells, in an arena with `open` open cells
before the closing:

```
frac      = n / open                      // 0..1, how large it was
size_mult = lerp(3.0, 0.4, frac)          // small = risky = pays more
risk      = 1.0 + 0.35 * balls_near       // balls <4 cells from the wall on consolidation
speed_b   = 1.0 + 0.10 * max_ball_speed   // fast arena = more risk
base      = 100.0 * n.sqrt()
points    = base * size_mult * risk * speed_b * combo_mult
```

Combos: every wall consolidated without losing a life inside
`COMBO_WINDOW = 4.0 s` raises the multiplier: x1 → x2 → x3 → x4 (capped at
x8). It resets when a life is lost or when the window expires.

## 6. Player input (the only mutation surface)

```rust
pub enum PlayerInput {
    None,
    StartWall { cell: (u16, u16), axis: WallAxis },
    ToggleAxis,                 // key: toggles H/V for the next wall
    UsePowerUp(PowerUpKind),
    Pause, Resume, Restart,
}
```

The single pure, central function that EVERY worker must respect:

```rust
pub fn step(state: &GameState, input: PlayerInput, dt: f32) -> (GameState, Vec<GameEvent>)
```

- It does not mutate `state`; it returns a new one (or uses `&mut self`
  internally on a clone).
- `GameEvent` describes what happened so render/audio can react:
  `WallStarted, WallBlocked, WallCompleted{points, cells}, BallLost,
   LifeLost, ComboUp(u8), ComboReset, PowerUpSpawned, LevelCleared{stars}, GameOver`.
- Determinism: same `state` + same `input` + same `dt` → same result.
  That is what makes the tests possible. There is no `Instant::now()` in the
  core.

## 7. Modes

```rust
pub enum Mode { Original, Enhanced }
```

- `Original`: 10 levels. Roughly `balls = 2 + (level-1)/2`, no power-ups,
  no obstacles, no combos (the combo exists but is pinned to x1),
  no secondary objectives. The `level.purist = true` flag disables everything
  modern.
- `Enhanced`: 60 levels to start with (6 worlds × 10), a structure that can
  be extended without recompiling (levels live in
  `assets/levels/enhanced.ron`).

Worlds: 1 Classic, 2 Speed, 3 Obstacles, 4 Special balls,
5 Power-ups, 6 Advanced challenges. Levels 5 and 10 of every world are
special: `Boss` (large slow ball), `SpeedRun` (against the clock), `Chaos`
(small fast balls + obstacles + 1 life).

## 8. Special balls

| Kind | Behaviour |
|---|---|
| `Normal` | radius 0.45, speed 8 cells/s |
| `Fast` | speed ×1.8 |
| `Erratic` | every 1.5 s it rotates its velocity by ±30° (deterministic PRNG) |
| `Splitter` | when its region is sealed it splits into 2 `Normal` balls if there is room |
| `Heavy` | radius 1.2, speed ×0.6, ALWAYS destroys walls under construction (ignores shields) |

## 9. Power-ups (Enhanced only, worlds ≥5)

`SlowMotion` (dt×0.5, 5 s), `Freeze` (balls frozen, 3 s),
`DoubleWall` (2 simultaneous builders, single use), `Shield` (absorbs 1 hit),
`RemoveBall` (removes the slowest ball, single use).
They spawn as a collectable cell every `30±10 s`, max 2 in the arena.

## 10. Objectives and stars

```rust
pub enum Objective {
    ClearRatio(f32),      // reduce the area to X%
    NoLivesLost,
    UnderTime(f32),
    MinScore(u32),
    KeepCombo(u8),
    NoPowerUps,
}
```
1 star per objective met, max 3 per level. Replayable: the all-time best star
count and score are saved.

## 11. Persistence (app layer, never the core)

Path: `$XDG_DATA_HOME/omarchy-jezzball/save.ron`
(fallback `~/.local/share/omarchy-jezzball/save.ron`).
Config: `$XDG_CONFIG_HOME/omarchy-jezzball/config.ron`.
Atomic writes: write to `save.ron.tmp` + `rename`. No network, no accounts.

```rust
struct SaveData {
    version: u32,
    original: BTreeMap<u16, LevelRecord>,   // level -> record
    enhanced: BTreeMap<u16, LevelRecord>,
    original_completed: bool,
}
struct LevelRecord { best_score: u32, stars: u8, completed: bool, best_time: f32 }
```

## 12. Omarchy theming

CORRECTION (verified on a real machine): Omarchy does NOT store the active
theme under `~/.config/omarchy/current`. The real path is
`~/.local/state/omarchy/current/theme/` (a symlink to the active theme) and
the theme name lives in `~/.local/state/omarchy/current/theme.name`.
System themes live in `/usr/share/omarchy/themes/<name>/`.

Preferred source of truth: the theme's `colors.toml`, which is a FLAT TOML
file (no sections) with direct keys. Real example (the `retro-82` theme):

```toml
mode = "dark"
accent = "#faa968"
selection = "#134e5a"
muted = "#2a6b78"
background = "#05182e"
dark_background = "#031222"
darker_background = "#020c17"
lighter_background = "#0a2540"
foreground = "#f6dcac"
dark_foreground = "#3f8f8a"
light_foreground = "#a7c9c6"
bright_foreground = "#f6dcac"
red = "#f85525"
yellow = "#e97b3c"
orange = "#faa968"
green = "#028391"
cyan = "#8cbfb8"
blue = "#3f8f8a"
magenta = "#3f8f8a"
brown = "#743d1e"
bright_red = "…" bright_yellow = "…" bright_green = "…"
bright_cyan = "…" bright_blue = "…" bright_magenta = "…"
```

Resolution order (first hit wins):
1. `$XDG_CONFIG_HOME/omarchy-jezzball/theme.ron` (user override).
2. `$XDG_STATE_HOME/omarchy/current/theme/colors.toml`
   (fallback `~/.local/state/omarchy/current/theme/colors.toml`).
3. Read the name from `~/.local/state/omarchy/current/theme.name` and look up
   `/usr/share/omarchy/themes/<name>/colors.toml`.
4. `~/.config/omarchy/themes/<name>/colors.toml` (user themes).
5. As the last source before the fallback: the theme's `alacritty.toml`, from
   which `colors.primary.background/foreground` and `colors.normal.*` are
   extracted.
6. Hardcoded fallback: bg `#0f0f14`, fg `#c9d1d9`, accent `#7aa2f7`.

Mapping from game roles → `colors.toml` keys:

| Role | Key |
|---|---|
| `bg` | `background` |
| `bg_panel` | `lighter_background` (HUD, panels) |
| `fg` | `foreground` |
| `fg_dim` | `dark_foreground` (secondary text) |
| `wall` | `muted` (consolidated wall) |
| `wall_building` | `yellow` (wall AT RISK, must stand out) |
| `ball` | `cyan` |
| `ball_special` | `magenta` |
| `danger` | `red` (lives, failure) |
| `ok` | `green` (objective met) |
| `accent` | `accent` (menu selection, combo) |

Never hardcode a colour in the render layer: everything comes from `Theme`.
Re-read the theme when the window regains focus (it is cheap) so hot theme
switches are picked up.
Real fixtures for tests live in `crates/jezzball-app/tests/fixtures/`
(`retro-82-colors.toml`, `tokyo-night-colors.toml`, `retro-82-alacritty.toml`).

## 13. Render / input (app layer)

- `macroquad` 0.4 (`miniquad` backend, supports native Wayland via GLFW/EGL;
  forcing Wayland through `SDL_VIDEODRIVER`/`WINIT_UNIX_BACKEND` does not
  apply — miniquad defaults to X11: document the `--x11` fallback and enable
  the Wayland feature when available; failing that, XWayland works, but
  `ggez`/`raylib-rs` is the preferred plan B).
- Loop: `next_frame().await` gives us the compositor's vsync.
  `dt = get_frame_time()`, clamped to 1/30 to avoid jumps after a stall.
- The render layer decides nothing about the game: it reads `&GameState` and
  draws.

Keyboard (keyboard-first):
`Space` pause · `R` restart level · `Esc` menu/back · `Tab` toggle wall axis
· `1..5` use power-up · `Enter` confirm · `↑↓←→`/`hjkl` navigate menus
· `F` toggle compact HUD · `Q` quit (with confirmation).
Mouse: left click = wall along the current axis; right click = opposite axis.

## 14. Packaging

- `PKGBUILD` (`pkgname=omarchy-jezzball`), `makedepends=(cargo)`,
  `depends=(wayland libxkbcommon)`, built with
  `cargo build --release --locked`, installs the binary into `/usr/bin`,
  assets into `/usr/share/omarchy-jezzball` and the `.desktop` file into
  `/usr/share/applications`.
- `.desktop`: `Categories=Game;ArcadeGame;`, `Terminal=false`,
  `StartupWMClass=omarchy-jezzball` (for Hyprland window rules).
- Document the suggested Hyprland rule:
  `windowrulev2 = float, class:^(omarchy-jezzball)$`

## 15. Quality

- `cargo test -p jezzball-core` must pass. At minimum:
  wall bounce, a consolidated wall seals an empty region, a region containing
  a ball is NOT sealed, a ball hitting a wall under construction → life lost,
  small-region score < large-region score inverted, combo rises and expires,
  determinism (same seed → same trace over 600 steps).
- `cargo clippy -- -D warnings` clean in the core.
- No `unwrap()` on the app's I/O paths.
