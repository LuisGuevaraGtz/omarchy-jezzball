# docs/THEMING.md — How Omarchy-Jezzball gets its colours

The game ships no palette of its own. It derives every colour from the active
Omarchy theme. This page explains the resolution order, the role mapping and
how to pin colours by hand.

## 1. Resolution order

The first step that returns a valid theme wins:

1. **User override:** `$XDG_CONFIG_HOME/omarchy-jezzball/theme.ron`
   (usually `~/.config/omarchy-jezzball/theme.ron`). If it exists and parses
   cleanly, it is used as-is. It is the only source you control 100 %.
2. **Live Omarchy theme:** `~/.config/omarchy/current/theme/alacritty.toml`.
   `colors.primary.background`, `colors.primary.foreground` and the
   `colors.normal` table (`black, red, green, yellow, blue, magenta, cyan,
   white`) are read from it.
3. **Theme by name:** if Omarchy exposes the active theme under
   `~/.config/omarchy/themes/<active>/`, its `alacritty.toml` is read exactly
   as in step 2.
4. **Hardcoded fallback:** background `#0f0f14`, text `#c9d1d9`,
   accent `#7aa2f7`. This guarantees the game always starts, even with no
   theme installed at all.

The theme is re-read whenever the window regains focus, so switching themes
in Omarchy shows up in the game without a restart.

## 2. Role mapping

The game works with 9 semantic roles. The render layer never uses a literal
colour: it always asks `Theme` for one of these roles. The mapping from an
Omarchy theme (steps 2 and 3) is fixed:

| Game role       | Source in the Omarchy theme | Where it is used in the game                 |
|-----------------|-----------------------------|----------------------------------------------|
| `bg`            | `primary.background`        | arena and menu background                    |
| `fg`            | `primary.foreground`        | text, HUD, menu borders                      |
| `wall`          | `normal.white`              | already-consolidated walls                   |
| `wall_building` | `normal.yellow`             | wall under construction (growing fronts)     |
| `ball`          | `normal.cyan`               | normal balls                                 |
| `ball_special`  | `normal.magenta`            | special balls (Fast, Erratic, …)             |
| `danger`        | `normal.red`                | warnings: ball near the wall, last life      |
| `ok`            | `normal.green`              | confirmations, level cleared, stars          |
| `accent`        | `normal.blue`               | menu selection, highlights, power-ups        |

`normal.black` is ignored: against dark terminal backgrounds it does not give
enough contrast for any role.

## 3. Manual override: `theme.ron`

To pin colours by hand, create `~/.config/omarchy-jezzball/theme.ron` with
all 9 roles as `#rrggbb` hex values. All 9 are mandatory; if one is missing
or the file fails to parse, the whole file is ignored and resolution falls
through to step 2 (sources are never partially merged).

Full example:

```ron
(
    bg: "#0f0f14",
    fg: "#c9d1d9",
    wall: "#e6e6e6",
    wall_building: "#e5c07b",
    ball: "#56b6c2",
    ball_special: "#c678dd",
    danger: "#e06c75",
    ok: "#98c379",
    accent: "#7aa2f7",
)
```

Notes:

- Values are `String`s formatted as lowercase `#rrggbb` (uppercase is
  accepted, but the example uses lowercase by convention).
- The 3 fields that do not appear here (`name`, `bg_panel`, `fg_dim`) are
  derived: `name` becomes "personalizado", `bg_panel` lightens `bg` and
  `fg_dim` darkens `fg`, both by ~12 % (a gentle per-channel luminosity
  adjustment).
- To go back to the automatic Omarchy theme, delete or rename the file.
- IMPLEMENTED: this `(key: "value", …)` schema is implemented by the app in
  `crates/jezzball-app/src/theme.rs` through the `ThemeOverrideFile` DTO. The
  `exact_block_from_theming_md_parses` test copies this example verbatim: if
  the format changes here or in the struct, that test fails and both must be
  updated together.
