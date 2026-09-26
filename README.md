# Omarchy-Jezzball

[![CI](https://github.com/LuisGuevaraGtz/omarchy-jezzball/actions/workflows/ci.yml/badge.svg)](https://github.com/LuisGuevaraGtz/omarchy-jezzball/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A native JezzBall-style game for Omarchy (Arch Linux + Hyprland, Wayland).
Seal off regions of the arena by drawing walls before the balls smash them.
No Electron, no heavyweight engine, no accounts: one binary, keyboard-first,
and a terminal aesthetic that follows your Omarchy theme.

70 levels across two modes, written in Rust on `macroquad`.

![Enhanced Mode, level 25: obstacles in the arena, combo and objectives in the
HUD](docs/screenshots/gameplay-enhanced.png)

## Quick start

```bash
git clone https://github.com/LuisGuevaraGtz/omarchy-jezzball.git
cd omarchy-jezzball
./packaging/install.sh     # builds release, installs into ~/.local
omarchy-jezzball           # play
```

Needs stable Rust and a Wayland session. To remove it again:

```bash
./packaging/install.sh --uninstall   # keeps your saved progress
./packaging/install.sh --purge       # removes the save too
```

Arch users can build a real package instead — see
[Installation](#installation) below.

## Philosophy and modes

Two modes with two very different intentions:

- **Original Mode** — faithful preservation. 10 levels, rectangular arena,
  plain balls, no power-ups, no obstacles, no secondary objectives and no
  combos (multiplier pinned at x1). The difficulty curve is the classic one:
  level N has `1 + N` balls, each wave slightly faster. Flag `purist = true`.
- **Enhanced Mode** — evolution. 60 levels across 6 worlds × 10:
  1 Classic, 2 Speed, 3 Obstacles, 4 Special balls,
  5 Power-ups, 6 Advanced challenges. With combos (up to x8), 5 kinds of
  special ball, 5 power-ups, obstacles (`Block`, `Mover`, `NoSplit`) and up
  to 3 objectives per level (stars). Levels 5 and 10 of every world are
  special: `SpeedRun`, `Boss`, `Chaos`. Levels live in
  `assets/levels/*.ron`, so they can be extended without recompiling.

## Requirements

- Arch Linux (or a derivative), a Wayland compositor (Hyprland recommended).
- Stable Rust (`rustup`, `stable` toolchain).
- System libraries: `wayland`, `libxkbcommon`, `libglvnd`, `alsa-lib`.
- `git`, `cargo`. For the launcher: any menu that reads `.desktop` files.

## Installation

### Option A — Arch package (recommended)

```bash
cd packaging
makepkg -si
```

This builds with `cargo build --frozen --release --all-features`, runs the
core test suite and installs:

- the binary at `/usr/bin/omarchy-jezzball`
- levels at `/usr/share/omarchy-jezzball/`
- the launcher at `/usr/share/applications/omarchy-jezzball.desktop`
- the license at `/usr/share/licenses/omarchy-jezzball/LICENSE`

### Option B — Local install without pacman (development)

```bash
./packaging/install.sh
```

Builds in release mode and installs into `~/.local/bin`,
`~/.local/share/omarchy-jezzball` and `~/.local/share/applications`.
It warns you if `~/.local/bin` is not on your `PATH`. To uninstall:

```bash
./packaging/install.sh --uninstall   # keeps your saved progress
./packaging/install.sh --purge       # also removes save, config and crash log
```

`--uninstall` removes only what the installer put there. Your progress at
`$XDG_DATA_HOME/omarchy-jezzball/save.ron` survives, and the directory is only
removed if nothing of yours is left in it.

### Where does the game look for levels?

Levels (`original.ron` and `enhanced.ron`) are loaded from disk at runtime,
so they can be extended without recompiling. The game tries these paths, in
this order (the first one that exists wins):

1. `$OMARCHY_JEZZBALL_ASSETS/levels/` — development/test escape hatch;
   point it at the repo's `assets/` directory.
2. `<binary directory>/assets/levels/` — binary sitting next to the assets.
3. `<binary directory>/../share/omarchy-jezzball/levels/` — what
   `packaging/install.sh` produces (`~/.local/bin` + `~/.local/share`).
4. `$XDG_DATA_HOME/omarchy-jezzball/levels/`
   (if unset: `~/.local/share/omarchy-jezzball/levels/`).
5. `/usr/share/omarchy-jezzball/levels/` — what the `PKGBUILD` installs.
6. `./assets/levels/` — relative to the current directory (for `cargo run`
   from the repo root).

If none of those paths holds the levels, the error screen lists every path
that was tried, one per line, and the same list is printed to `stderr` at
startup.

### Hyprland (optional)

`packaging/hyprland.conf.example` ships suggested rules (a centred 1280×800
floating window) and a sample keybind (`SUPER+J`). Copy whatever you want
into your `hyprland.conf`.

## How to play

You clear a level by sealing at least 75 % of the fillable area
(`target_ratio`, which can vary in Enhanced). How:

1. Click an empty cell: a wall grows from there in both directions along the
   current axis (horizontal or vertical).
2. If the wall consolidates before a ball touches it, whichever isolated
   region has no balls in it gets filled and scores points.
3. If a ball hits the wall while it is still growing, the wall is destroyed
   and you lose a life (unless shielded; the `Heavy` ball always breaks
   through).

Scoring well means taking risks: sealing small regions close to fast balls
pays more (size × risk × speed × combo multiplier). Chaining walls without
losing a life within 4 s pushes the combo up to x8.

Command-line flags:

```bash
omarchy-jezzball                 # Original Mode, first unfinished level
omarchy-jezzball --mode original # start straight into Original Mode
omarchy-jezzball --mode enhanced # start straight into Enhanced Mode
omarchy-jezzball --level 25      # start at level 25 of the chosen mode
```

With no arguments the game goes straight into Original Mode. Once you have
completed a mode, launching it opens the level list instead, so you can replay
any level. `M` goes back to the menu from inside a game.

## Keyboard shortcuts

Source: contract §13. The mouse only draws walls; everything else is
keyboard.

| Key                | Action                              |
|--------------------|-------------------------------------|
| `Space`            | pause / resume                      |
| `R`                | restart level                       |
| `Esc`              | menu / back                         |
| `Tab`              | toggle wall axis (H/V)              |
| `1`–`5`            | use power-up (slots 1 to 5)         |
| `Enter`            | confirm                             |
| `↑` `↓` `←` `→` / `hjkl` | navigate menus                |
| `F`                | toggle compact HUD                  |
| `Q`                | quit (asks for confirmation)        |
| left click         | draw a wall along the current axis  |
| right click        | draw a wall along the opposite axis |

## Where the save file lives

Contract §11. No network, no accounts, atomic writes
(`save.ron.tmp` + `rename`):

- Save: `$XDG_DATA_HOME/omarchy-jezzball/save.ron`
  (if `XDG_DATA_HOME` is unset: `~/.local/share/omarchy-jezzball/save.ron`).
  It stores, per mode and level: best score, stars (0–3), completion flag
  and best time.
- Config: `$XDG_CONFIG_HOME/omarchy-jezzball/config.ron`
  (usually `~/.config/omarchy-jezzball/config.ron`).
- Theme override: `~/.config/omarchy-jezzball/theme.ron` (see theming).

## Progression

ENHANCED MODE unlocks once you complete all 10 levels of ORIGINAL MODE.
Enhanced is an evolution of the classic mechanic (obstacles, special balls,
power-ups), so you learn the fundamentals first.

Inside Enhanced, worlds 2 through 6 open up as you progress through
Original. The HOW TO PLAY screen in the menu explains the rules, the scoring
and every obstacle, special ball and power-up.

## Language

The game detects the system language (`LANG` / `LC_ALL`). Spanish and English
are included. To force one:

```bash
OMARCHY_JEZZBALL_LANG=en omarchy-jezzball
```

Every string lives in `assets/i18n/<code>.ron`, outside the code.
To add a language: copy `es.ron`, translate the VALUES (not the keys) and
register the code in `CATALOGS` (`crates/jezzball-app/src/i18n.rs`).
One test checks that every language has exactly the same keys, and another
checks that no user-facing string is hardcoded inside the code.

## If the game won't start or exits unexpectedly

Every internal failure is written to disk as well as printed to stderr:

```
~/.local/state/omarchy-jezzball/crash.log
```

That file is all anyone needs to diagnose a problem: it contains the exact
line that failed. If you open an issue, attach it.

**The window never opens.** The game uses miniquad's X11 backend by default
(under Hyprland that goes through XWayland, which is the normal setup).
Miniquad's native Wayland backend is marked unstable by its own authors, so
it is only used when X11 is unavailable. You can force the choice:

```bash
OMARCHY_JEZZBALL_BACKEND=wayland  omarchy-jezzball   # Wayland only
OMARCHY_JEZZBALL_BACKEND=x11      omarchy-jezzball   # X11 only
OMARCHY_JEZZBALL_BACKEND=wayland-first omarchy-jezzball
```

**"No levels available".** The game could not find the `.ron` files. Every
path it tried is listed on stderr at startup. You can also point at one
directly:

```bash
OMARCHY_JEZZBALL_ASSETS=/path/that/contains/levels omarchy-jezzball
```

**Text too small or too large.** The interface scales itself with the window
size (1280x720 reference, minimum x1.15). To override it by hand:

```bash
OMARCHY_JEZZBALL_UI_SCALE=1.5 omarchy-jezzball   # 1.5x
OMARCHY_JEZZBALL_UI_SCALE=1.0 omarchy-jezzball   # base size
```

**Debugging a crash with extra checks.** The `release-checked` profile builds
optimized but keeps overflow detection and debug symbols:

```bash
cargo build --profile release-checked
./target/release-checked/omarchy-jezzball
```

## Theming

The game ships no palette of its own: it derives its 9 colour roles from the
active Omarchy theme (`bg`, `fg`, `wall`, `wall_building`, `ball`,
`ball_special`, `danger`, `ok`, `accent`). Order: your manual `theme.ron` →
`~/.config/omarchy/current/theme/alacritty.toml` → theme by name → dark
fallback. Switching themes is picked up when the window regains focus, no
restart needed. Full detail and a `theme.ron` example in `docs/THEMING.md`.

## Repository layout

```text
Omarchy-Jezzball/
├─ Cargo.toml                    # virtual workspace (members: core, app)
├─ crates/
│  ├─ jezzball-core/             # pure logic: no render, no OS, no I/O
│  └─ jezzball-app/              # macroquad: render, input, audio, saving
├─ assets/
│  ├─ levels/original.ron        # 10 Original Mode levels
│  ├─ levels/enhanced.ron        # 60 Enhanced Mode levels
│  ├─ i18n/{en,es}.ron           # UI strings, compiled into the binary
│  └─ icons/omarchy-jezzball.svg
├─ packaging/
│  ├─ PKGBUILD
│  ├─ omarchy-jezzball.desktop
│  ├─ install.sh
│  └─ hyprland.conf.example
├─ docs/
│  ├─ ARCHITECTURE.md            # normative contract
│  ├─ LEVEL_SCHEMA.md            # level schema (normative)
│  ├─ THEMING.md
│  └─ screenshots/               # images used by this README
└─ README.md
```

## Documentation

- `docs/ARCHITECTURE.md` — the normative architecture contract.
- `docs/LEVEL_SCHEMA.md` — the level schema (normative).
- `docs/THEMING.md` — colour roles and a `theme.ron` example.

## Building and testing from source

```bash
cargo build --locked                    # debug
cargo build --locked --release          # release
cargo test -p jezzball-core             # mandatory core tests
cargo clippy -p jezzball-core -- -D warnings   # clippy must be clean in core
```

The core is deterministic and pure: the same state + the same input + the
same `dt` always produces the same result. Local packaging needs no `cargo`
work beyond what is described here; `packaging/install.sh` covers the
development flow.
