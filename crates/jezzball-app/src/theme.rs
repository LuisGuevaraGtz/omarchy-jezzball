//! Resolution of the Omarchy theme (ARCHITECTURE.md §12).
//!
//! Resolution order (the first hit wins):
//!   1. `$XDG_CONFIG_HOME/omarchy-jezzball/theme.ron` (the user's override).
//!   2. `$XDG_STATE_HOME/omarchy/current/theme/colors.toml`.
//!   3. Theme name in `.../current/theme.name` + `colors.toml` in
//!      `/usr/share/omarchy/themes/<name>/`.
//!   4. `~/.config/omarchy/themes/<name>/colors.toml`.
//!   5. The theme's `alacritty.toml` (`colors.primary.*` / `colors.normal.*`).
//!   6. Hardcoded fallback.
//!
//! It never panics because of I/O paths: each step can fail and degrades
//! gracefully down to the final fallback. The `resolve_theme` function receives the
//! base directories as parameters so it can be tested without touching the system.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// RGB colour (8 bits per channel). Independent of macroquad so that all the
/// theme mapping is pure and testable without a window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Rgb { r, g, b }
    }

    /// Converts to `macroquad::color::Color` with the given transparency.
    pub fn to_mq(self, a: f32) -> macroquad::color::Color {
        macroquad::color::Color::new(
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            a,
        )
    }
}

/// The game's palette, derived from Omarchy's active theme. No render hardcodes
/// a colour: everything comes from here (ARCHITECTURE.md §12 "Role mapping").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub bg: Rgb,
    pub bg_panel: Rgb,
    pub fg: Rgb,
    pub fg_dim: Rgb,
    pub wall: Rgb,
    pub wall_building: Rgb,
    pub ball: Rgb,
    pub ball_special: Rgb,
    pub danger: Rgb,
    pub ok: Rgb,
    pub accent: Rgb,
}

/// Format of the user's override file (`theme.ron`, step 1). It is a DTO
/// deliberately smaller than `Theme`: it exposes the 9 roles documented in
/// docs/THEMING.md §3 as `#rrggbb` strings, without the 3 fields the app derives
/// (`name`, `bg_panel`, `fg_dim`). RON parsing is strict: if a role is missing or
/// a hex does not parse, the whole override is discarded and we move on to step 2.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThemeOverrideFile {
    pub bg: String,
    pub fg: String,
    pub wall: String,
    pub wall_building: String,
    pub ball: String,
    pub ball_special: String,
    pub danger: String,
    pub ok: String,
    pub accent: String,
}

impl ThemeOverrideFile {
    /// Converts the DTO into a `Theme`. Returns `None` if any of the 9 hex values is
    /// invalid: a half-built theme is never constructed. The fields the user
    /// does not write are derived: `name` is fixed and `bg_panel`/`fg_dim` are computed
    /// with a soft luminosity adjustment over `bg` and `fg` (≈12 %, see
    /// docs/THEMING.md §3).
    fn into_theme(self) -> Option<Theme> {
        let bg = parse_hex(&self.bg)?;
        let fg = parse_hex(&self.fg)?;
        Some(Theme {
            name: "personalizado".to_string(),
            bg,
            bg_panel: mix(bg, Rgb::new(255, 255, 255), DERIVE_SHIFT),
            fg,
            fg_dim: mix(fg, Rgb::new(0, 0, 0), DERIVE_SHIFT),
            wall: parse_hex(&self.wall)?,
            wall_building: parse_hex(&self.wall_building)?,
            ball: parse_hex(&self.ball)?,
            ball_special: parse_hex(&self.ball_special)?,
            danger: parse_hex(&self.danger)?,
            ok: parse_hex(&self.ok)?,
            accent: parse_hex(&self.accent)?,
        })
    }
}

/// Colours of the hardcoded fallback (ARCHITECTURE.md §12, step 6).
pub const FB_BG: Rgb = Rgb {
    r: 0x0f,
    g: 0x0f,
    b: 0x14,
};
pub const FB_BG_PANEL: Rgb = Rgb {
    r: 0x16,
    g: 0x16,
    b: 0x1e,
};
pub const FB_FG: Rgb = Rgb {
    r: 0xc9,
    g: 0xd1,
    b: 0xd9,
};
pub const FB_FG_DIM: Rgb = Rgb {
    r: 0x6e,
    g: 0x6e,
    b: 0x79,
};
pub const FB_WALL: Rgb = Rgb {
    r: 0x56,
    g: 0x5f,
    b: 0x89,
};
pub const FB_WALL_BUILDING: Rgb = Rgb {
    r: 0xe0,
    g: 0xaf,
    b: 0x68,
};
pub const FB_BALL: Rgb = Rgb {
    r: 0x44,
    g: 0x9d,
    b: 0xab,
};
pub const FB_BALL_SPECIAL: Rgb = Rgb {
    r: 0xbb,
    g: 0x9a,
    b: 0xf7,
};
pub const FB_DANGER: Rgb = Rgb {
    r: 0xf7,
    g: 0x76,
    b: 0x8e,
};
pub const FB_OK: Rgb = Rgb {
    r: 0x9e,
    g: 0xce,
    b: 0x6a,
};
pub const FB_ACCENT: Rgb = Rgb {
    r: 0x7a,
    g: 0xa2,
    b: 0xf7,
};

/// Constructor of the fallback set by contract.
pub fn fallback() -> Theme {
    Theme {
        name: "fallback".to_string(),
        bg: FB_BG,
        bg_panel: FB_BG_PANEL,
        fg: FB_FG,
        fg_dim: FB_FG_DIM,
        wall: FB_WALL,
        wall_building: FB_WALL_BUILDING,
        ball: FB_BALL,
        ball_special: FB_BALL_SPECIAL,
        danger: FB_DANGER,
        ok: FB_OK,
        accent: FB_ACCENT,
    }
}

impl Default for Theme {
    fn default() -> Self {
        fallback()
    }
}

/// Base directories that take part in the theme resolution. They are injected as
/// parameters so the tests do not depend on the environment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OmarchyDirs {
    pub config_home: PathBuf,
    pub state_home: PathBuf,
    pub usr_share_themes: PathBuf,
}

impl OmarchyDirs {
    /// Computes the directories from the real XDG variables.
    pub fn from_env() -> Self {
        OmarchyDirs {
            config_home: crate::persist::config_home_dir(),
            state_home: crate::persist::state_home_dir(),
            usr_share_themes: PathBuf::from("/usr/share/omarchy/themes"),
        }
    }
}

/// Parses `#rrggbb` (with or without the hash). Returns `None` for odd strings.
pub fn parse_hex(s: &str) -> Option<Rgb> {
    let t = s.trim().strip_prefix('#').unwrap_or(s.trim());
    if t.len() != 6 {
        return None;
    }
    let comp = |i: usize| u8::from_str_radix(&t[i..i + 2], 16).ok();
    Some(Rgb::new(comp(0)?, comp(2)?, comp(4)?))
}

/// Fraction of the soft luminosity adjustment for the roles derived from the
/// override (docs/THEMING.md §3): `bg_panel` is lightened towards white and
/// `fg_dim` is darkened towards black, both ≈12 %.
const DERIVE_SHIFT: f32 = 0.12;

/// Interpolates colour `a` towards colour `b` with factor `t` (0..=1), per channel.
fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    Rgb::new(
        (a.r as f32 + (b.r as f32 - a.r as f32) * t).round() as u8,
        (a.g as f32 + (b.g as f32 - a.g as f32) * t).round() as u8,
        (a.b as f32 + (b.b as f32 - a.b as f32) * t).round() as u8,
    )
}

/// Reads a `get` value from a `toml::Value`, expecting a colour string.
fn table_get<'a>(v: &'a toml::Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(|x| x.as_str())
}

/// Optional colour from a table, looked up by a set of alternative keys.
fn color_or(v: &toml::Value, keys: &[&str]) -> Option<Rgb> {
    keys.iter()
        .find_map(|k| table_get(v, k))
        .and_then(parse_hex)
}

/// Builds a `Theme` from a flat `colors.toml` (direct keys).
/// It requires `background` and `foreground`; the rest falls back to alternatives from the
/// theme itself or to the per-role fallback.
fn theme_from_flat(v: &toml::Value) -> Option<Theme> {
    let bg = color_or(v, &["background", "darker_background"])?;
    let fg = color_or(v, &["foreground", "bright_foreground"])?;
    Some(Theme {
        name: String::new(),
        bg,
        bg_panel: color_or(v, &["lighter_background", "background", "selection"]).unwrap_or(bg),
        fg,
        // `fg_dim` is the secondary text (footers, progress
        // summaries, previous stars). `light_foreground` is preferred over
        // `dark_foreground`: measured on the retro-82 theme, `dark_foreground`
        // (#3f8f8a) composited with the 0.7-0.8 alpha the render uses drops to
        // 2.94:1 contrast against the background, far below the readable
        // minimum (4.5:1). `light_foreground` gives 5.49:1 under the same
        // conditions.
        fg_dim: color_or(v, &["light_foreground", "bright_foreground", "cyan"])
            .unwrap_or_else(|| mix(fg, bg, 0.35)),
        wall: color_or(v, &["muted", "blue", "cyan"]).unwrap_or(FB_WALL),
        wall_building: color_or(v, &["yellow", "orange"]).unwrap_or(FB_WALL_BUILDING),
        ball: color_or(v, &["cyan", "blue"]).unwrap_or(fg),
        ball_special: color_or(v, &["magenta", "red"]).unwrap_or(FB_BALL_SPECIAL),
        danger: color_or(v, &["red"]).unwrap_or(FB_DANGER),
        ok: color_or(v, &["green"]).unwrap_or(FB_OK),
        accent: color_or(v, &["accent", "orange", "magenta"]).unwrap_or(FB_ACCENT),
    })
}

/// Builds a `Theme` from an `alacritty.toml` (`colors.primary.*` and
/// `colors.normal.*`). It requires `primary.background` and `primary.foreground`.
fn theme_from_alacritty(v: &toml::Value) -> Option<Theme> {
    let colors = v.get("colors")?;
    let primary = colors.get("primary")?;
    let bg = table_get(primary, "background").and_then(parse_hex)?;
    let fg = table_get(primary, "foreground").and_then(parse_hex)?;
    let normal = colors.get("normal")?;
    Some(Theme {
        name: String::new(),
        bg,
        bg_panel: color_or(normal, &["black"]).unwrap_or(bg),
        fg,
        fg_dim: color_or(normal, &["black"]).unwrap_or(fg),
        wall: color_or(normal, &["blue", "cyan"]).unwrap_or(FB_WALL),
        wall_building: color_or(normal, &["yellow", "white"]).unwrap_or(FB_WALL_BUILDING),
        ball: color_or(normal, &["cyan", "blue"]).unwrap_or(fg),
        ball_special: color_or(normal, &["magenta"]).unwrap_or(FB_BALL_SPECIAL),
        danger: color_or(normal, &["red"]).unwrap_or(FB_DANGER),
        ok: color_or(normal, &["green"]).unwrap_or(FB_OK),
        accent: color_or(normal, &["magenta", "blue"]).unwrap_or(FB_ACCENT),
    })
}

/// Tries to build a theme from a TOML, regardless of whether it is flat or
/// alacritty style.
fn theme_from_value(v: &toml::Value) -> Option<Theme> {
    theme_from_flat(v).or_else(|| theme_from_alacritty(v))
}

/// Reads a `colors.toml` or `alacritty.toml` from a file. The source name is
/// used as the theme's label. It never returns an error: only `None`.
fn load_colors_file(path: &Path) -> Option<Theme> {
    let content = fs::read_to_string(path).ok()?;
    let v: toml::Value = toml::from_str(&content).ok()?;
    theme_from_value(&v)
}

/// Reads the user's RON override (step 1). It ignores parsing errors: a
/// file with one role missing or an invalid hex is discarded entirely and the
/// resolution falls through to step 2 (docs/THEMING.md §3).
fn load_ron_override(path: &Path) -> Option<Theme> {
    let content = fs::read_to_string(path).ok()?;
    let file: ThemeOverrideFile = ron::from_str(&content).ok()?;
    file.into_theme()
}

/// Reads the name of the active theme (`theme.name`). It rejects names with
/// path separators to avoid traversal.
fn read_theme_name(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let name = raw.trim();
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        None
    } else {
        Some(name.to_string())
    }
}

/// Full theme resolution according to ARCHITECTURE.md §12. The first source that
/// succeeds wins; if none shows up, it returns the hardcoded fallback.
pub fn resolve_theme(dirs: &OmarchyDirs) -> Theme {
    // 1. The user's override.
    let override_path = dirs.config_home.join("omarchy-jezzball").join("theme.ron");
    if let Some(t) = load_ron_override(&override_path) {
        return t;
    }

    let current = dirs
        .state_home
        .join("omarchy")
        .join("current")
        .join("theme");

    let name = read_theme_name(&dirs.state_home.join("omarchy/current/theme.name"));

    // 2. Active symlinked theme (flat colors.toml). The name comes from
    //    `theme.name` (e.g. "retro-82"); previously the literal
    //    "corriente" was written —a word-for-word translation of "current"— which, on top
    //    of meaning nothing in Spanish, discarded the real name, which is
    //    available right next to it.
    if let Some(mut t) = load_colors_file(&current.join("colors.toml")) {
        t.name = name.clone().unwrap_or_else(|| "omarchy".to_string());
        return t;
    }

    if let Some(n) = name.as_deref() {
        // 3. System theme.
        let sys = dirs.usr_share_themes.join(n);
        if let Some(mut t) = load_colors_file(&sys.join("colors.toml")) {
            t.name = n.to_string();
            return t;
        }
        // 4. User theme.
        let user = dirs.config_home.join("omarchy/themes").join(n);
        if let Some(mut t) = load_colors_file(&user.join("colors.toml")) {
            t.name = n.to_string();
            return t;
        }
        // 5. Last real source: the theme's alacritty.toml.
        if let Some(t) = load_colors_file(&sys.join("alacritty.toml")) {
            return t;
        }
        if let Some(t) = load_colors_file(&user.join("alacritty.toml")) {
            return t;
        }
    }

    // 5b. Alacritty of the "current" theme (if there was no name).
    if let Some(t) = load_colors_file(&current.join("alacritty.toml")) {
        return t;
    }

    // 6. Fallback.
    Theme::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    fn read_fixture(name: &str) -> String {
        fs::read_to_string(fixture(name)).expect("fixture presente")
    }

    /// Temporary directory for tests that touch disk, inside the repo
    /// (the sandbox blocks /tmp and ~/.local, but the repo is writable).
    fn tmp_root(name: &str) -> PathBuf {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/.tmp")
            .join(format!("{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("crear tmp");
        root
    }

    #[test]
    fn parse_hex_works() {
        assert_eq!(parse_hex("  #05182e  "), Some(Rgb::new(0x05, 0x18, 0x2e)));
        assert_eq!(parse_hex("f6dcac"), Some(Rgb::new(0xf6, 0xdc, 0xac)));
        assert_eq!(parse_hex("F6DCAC"), Some(Rgb::new(0xf6, 0xdc, 0xac)));
        assert_eq!(parse_hex("#zzzzzz"), None);
        assert_eq!(parse_hex("#12345"), None);
        assert_eq!(parse_hex("abc"), None);
        assert_eq!(parse_hex(""), None);
    }

    #[test]
    fn retro82_flat_palette() {
        let v: toml::Value = toml::from_str(&read_fixture("retro-82-colors.toml")).unwrap();
        let t = theme_from_flat(&v).expect("theme parses");
        assert_eq!(t.bg, Rgb::new(0x05, 0x18, 0x2e));
        assert_eq!(t.fg, Rgb::new(0xf6, 0xdc, 0xac));
        assert_eq!(t.accent, Rgb::new(0xfa, 0xa9, 0x68));
        assert_eq!(t.wall, Rgb::new(0x2a, 0x6b, 0x78)); // muted
        assert_eq!(t.wall_building, Rgb::new(0xe9, 0x7b, 0x3c)); // yellow
        assert_eq!(t.ball, Rgb::new(0x8c, 0xbf, 0xb8)); // cyan
        assert_eq!(t.ball_special, Rgb::new(0x3f, 0x8f, 0x8a)); // magenta
        assert_eq!(t.danger, Rgb::new(0xf8, 0x55, 0x25)); // red
        assert_eq!(t.ok, Rgb::new(0x02, 0x83, 0x91)); // green
    }

    #[test]
    fn tokyo_night_flat_palette() {
        let v: toml::Value = toml::from_str(&read_fixture("tokyo-night-colors.toml")).unwrap();
        let t = theme_from_flat(&v).expect("theme parses");
        assert_eq!(t.bg, Rgb::new(0x1a, 0x1b, 0x26));
        assert_eq!(t.accent, Rgb::new(0x7a, 0xa2, 0xf7));
        assert_eq!(t.ok, Rgb::new(0x9e, 0xce, 0x6a));
    }

    #[test]
    fn alacritty_source() {
        let v: toml::Value = toml::from_str(&read_fixture("retro-82-alacritty.toml")).unwrap();
        let t = theme_from_alacritty(&v).expect("alacritty parseable");
        assert_eq!(t.bg, Rgb::new(0x05, 0x18, 0x2e));
        assert_eq!(t.fg, Rgb::new(0xf6, 0xdc, 0xac));
        assert_eq!(t.danger, Rgb::new(0xf8, 0x55, 0x25));
        assert_eq!(t.ok, Rgb::new(0x02, 0x83, 0x91));
        // accent falls back to magenta/blue from normal.
        assert_ne!(t.accent, FB_ACCENT);
    }

    #[test]
    fn with_no_files_it_returns_the_fallback_without_panicking() {
        let root = tmp_root("nc");
        let dirs = OmarchyDirs {
            config_home: root.join("config"),
            state_home: root.join("state"),
            usr_share_themes: root.join("system"),
        };
        let t = resolve_theme(&dirs);
        assert_eq!(t.name, "fallback");
        assert_eq!(t.bg, FB_BG);
        assert_eq!(t.fg, FB_FG);
        assert_eq!(t.accent, FB_ACCENT);
    }

    #[test]
    fn user_override_wins() {
        let root = tmp_root("override");
        let cfg = root.join("omarchy-jezzball");
        fs::create_dir_all(&cfg).unwrap();
        fs::write(
            cfg.join("theme.ron"),
            r##"(
                bg: "#010203",
                fg: "#070809",
                wall: "#0d0e0f",
                wall_building: "#101112",
                ball: "#131415",
                ball_special: "#161718",
                danger: "#191a1b",
                ok: "#1c1d1e",
                accent: "#1f2021",
            )"##,
        )
        .unwrap();
        let dirs = OmarchyDirs {
            config_home: root.clone(),
            state_home: root.join("state"),
            usr_share_themes: PathBuf::from("/nonexistent"),
        };
        let t = resolve_theme(&dirs);
        assert_eq!(t.name, "personalizado");
        assert_eq!(t.bg, Rgb::new(0x01, 0x02, 0x03));
        assert_eq!(t.accent, Rgb::new(0x1f, 0x20, 0x21));
        fs::remove_dir_all(&root).unwrap();
    }

    /// The exact RON block from docs/THEMING.md (copied literally, with its
    /// indentation) must parse and produce the expected colours. It is the contract
    /// that keeps document and code telling the same story: if someone changes
    /// the format in one of the two places, this test fails.
    #[test]
    fn exact_block_from_theming_md_parses() {
        const DOCUMENTADO: &str = r##"(
    bg: "#0f0f14",
    fg: "#c9d1d9",
    wall: "#e6e6e6",
    wall_building: "#e5c07b",
    ball: "#56b6c2",
    ball_special: "#c678dd",
    danger: "#e06c75",
    ok: "#98c379",
    accent: "#7aa2f7",
)"##;
        let f: ThemeOverrideFile = ron::from_str(DOCUMENTADO).expect("RON de docs parsea");
        let t = f.into_theme().expect("valid hex");
        assert_eq!(t.name, "personalizado");
        assert_eq!(t.bg, Rgb::new(0x0f, 0x0f, 0x14));
        assert_eq!(t.accent, Rgb::new(0x7a, 0xa2, 0xf7));
        assert_eq!(t.fg, Rgb::new(0xc9, 0xd1, 0xd9));
        assert_eq!(t.wall, Rgb::new(0xe6, 0xe6, 0xe6));
        assert_eq!(t.wall_building, Rgb::new(0xe5, 0xc0, 0x7b));
        assert_eq!(t.ball, Rgb::new(0x56, 0xb6, 0xc2));
        assert_eq!(t.ball_special, Rgb::new(0xc6, 0x78, 0xdd));
        assert_eq!(t.danger, Rgb::new(0xe0, 0x6c, 0x75));
        assert_eq!(t.ok, Rgb::new(0x98, 0xc3, 0x79));
        // Derived fields: bg_panel lightens bg by 12 % towards white,
        // fg_dim darkens fg by 12 % towards black.
        assert_eq!(t.bg_panel, Rgb::new(0x2c, 0x2c, 0x30));
        assert_eq!(t.fg_dim, Rgb::new(0xb1, 0xb8, 0xbf));
    }

    /// If a role is missing, the whole override is ignored and the resolution falls
    /// through to step 2 (docs/THEMING.md §3: "sources are not mixed halfway").
    #[test]
    fn override_with_a_missing_role_falls_through_to_step_2() {
        let root = tmp_root("faltante");
        let cfg = root.join("omarchy-jezzball");
        fs::create_dir_all(&cfg).unwrap();
        // `ok` is missing: the whole file must be discarded.
        fs::write(
            cfg.join("theme.ron"),
            r##"(
                bg: "#0f0f14",
                fg: "#c9d1d9",
                wall: "#e6e6e6",
                wall_building: "#e5c07b",
                ball: "#56b6c2",
                ball_special: "#c678dd",
                danger: "#e06c75",
                accent: "#7aa2f7",
            )"##,
        )
        .unwrap();
        // Step 2 available so that the fallthrough is observable.
        let theme_dir = root.join("state/omarchy/current/theme");
        fs::create_dir_all(&theme_dir).unwrap();
        fs::copy(
            fixture("retro-82-colors.toml"),
            theme_dir.join("colors.toml"),
        )
        .unwrap();
        let dirs = OmarchyDirs {
            config_home: root.clone(),
            state_home: root.join("state"),
            usr_share_themes: PathBuf::from("/nonexistent"),
        };
        let t = resolve_theme(&dirs);
        // With no theme.name in the fixture, step 2 uses the generic name.
        assert_eq!(t.name, "omarchy"); // it reached step 2, override discarded
        assert_eq!(t.bg, Rgb::new(0x05, 0x18, 0x2e));
        fs::remove_dir_all(&root).unwrap();
    }

    /// An invalid hex discards the whole override and falls through to step 2.
    #[test]
    fn override_with_invalid_hex_falls_through_to_step_2() {
        for (name, bad) in [("zz", "#zzzzzz"), ("corto", "#12345")] {
            let root = tmp_root(&format!("hexinv-{name}"));
            let cfg = root.join("omarchy-jezzball");
            fs::create_dir_all(&cfg).unwrap();
            let ron = format!(
                r##"(
                    bg: "#0f0f14",
                    fg: "#c9d1d9",
                    wall: "{bad}",
                    wall_building: "#e5c07b",
                    ball: "#56b6c2",
                    ball_special: "#c678dd",
                    danger: "#e06c75",
                    ok: "#98c379",
                    accent: "#7aa2f7",
                )"##
            );
            fs::write(cfg.join("theme.ron"), ron).unwrap();
            let theme_dir = root.join("state/omarchy/current/theme");
            fs::create_dir_all(&theme_dir).unwrap();
            fs::copy(
                fixture("retro-82-colors.toml"),
                theme_dir.join("colors.toml"),
            )
            .unwrap();
            let dirs = OmarchyDirs {
                config_home: root.clone(),
                state_home: root.join("state"),
                usr_share_themes: PathBuf::from("/nonexistent"),
            };
            let t = resolve_theme(&dirs);
            assert_eq!(t.name, "omarchy", "hex {bad} debe descartar el override");
            assert_eq!(t.bg, Rgb::new(0x05, 0x18, 0x2e));
            fs::remove_dir_all(&root).unwrap();
        }
    }

    /// Hex without `#` and in uppercase works (docs/THEMING.md accepts both).
    #[test]
    fn override_accepts_hex_without_hash_and_uppercase() {
        let root = tmp_root("mayus");
        let cfg = root.join("omarchy-jezzball");
        fs::create_dir_all(&cfg).unwrap();
        fs::write(
            cfg.join("theme.ron"),
            r##"(
                bg: "0F0F14",
                fg: "C9D1D9",
                wall: "E6E6E6",
                wall_building: "E5C07B",
                ball: "56B6C2",
                ball_special: "C678DD",
                danger: "E06C75",
                ok: "98C379",
                accent: "7AA2F7",
            )"##,
        )
        .unwrap();
        let dirs = OmarchyDirs {
            config_home: root.clone(),
            state_home: root.join("state"),
            usr_share_themes: PathBuf::from("/nonexistent"),
        };
        let t = resolve_theme(&dirs);
        assert_eq!(t.name, "personalizado");
        assert_eq!(t.bg, Rgb::new(0x0f, 0x0f, 0x14));
        assert_eq!(t.accent, Rgb::new(0x7a, 0xa2, 0xf7));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn current_colors_toml_from_state() {
        let root = tmp_root("current");
        let theme_dir = root.join("omarchy/current/theme");
        fs::create_dir_all(&theme_dir).unwrap();
        fs::copy(
            fixture("retro-82-colors.toml"),
            theme_dir.join("colors.toml"),
        )
        .unwrap();
        let dirs = OmarchyDirs {
            config_home: root.join("config"),
            state_home: root.clone(),
            usr_share_themes: PathBuf::from("/nonexistent"),
        };
        let t = resolve_theme(&dirs);
        assert_eq!(t.name, "omarchy");
        assert_eq!(t.bg, Rgb::new(0x05, 0x18, 0x2e));
    }

    #[test]
    fn theme_name_is_looked_up_in_system_and_user_dirs() {
        let root = tmp_root("name");
        let state = root.join("omarchy/current");
        fs::create_dir_all(&state).unwrap();
        fs::write(state.join("theme.name"), "aether\n").unwrap();
        let sys = root.join("system/aether");
        fs::create_dir_all(&sys).unwrap();
        fs::copy(fixture("tokyo-night-colors.toml"), sys.join("colors.toml")).unwrap();

        let dirs = OmarchyDirs {
            config_home: root.join("config"),
            state_home: root.clone(),
            usr_share_themes: root.join("system"),
        };
        let t = resolve_theme(&dirs);
        assert_eq!(t.name, "aether");
        assert_eq!(t.accent, Rgb::new(0x7a, 0xa2, 0xf7));

        // User theme with no colors.toml in the system.
        let user = root.join("config/omarchy/themes/retro82");
        fs::create_dir_all(&user).unwrap();
        fs::copy(fixture("retro-82-colors.toml"), user.join("colors.toml")).unwrap();
        fs::write(state.join("theme.name"), "retro82\n").unwrap();
        let t = resolve_theme(&dirs);
        assert_eq!(t.name, "retro82");
        assert_eq!(t.accent, Rgb::new(0xfa, 0xa9, 0x68));
    }

    #[test]
    fn malicious_theme_name_cannot_traverse() {
        let root = tmp_root("evil");
        let state = root.join("omarchy/current");
        fs::create_dir_all(&state).unwrap();
        fs::write(state.join("theme.name"), "../somewhere").unwrap();
        let t = resolve_theme(&OmarchyDirs {
            config_home: root.join("config"),
            state_home: root.clone(),
            usr_share_themes: root.join("system"),
        });
        assert_eq!(t.name, "fallback");
    }

    #[test]
    fn alacritty_as_last_resort() {
        let root = tmp_root("alac");
        let sys = root.join("system/t");
        fs::create_dir_all(&sys).unwrap();
        fs::write(
            sys.join("alacritty.toml"),
            read_fixture("retro-82-alacritty.toml"),
        )
        .unwrap();
        let state = root.join("omarchy/current");
        fs::create_dir_all(&state).unwrap();
        fs::write(state.join("theme.name"), "t\n").unwrap();
        let t = resolve_theme(&OmarchyDirs {
            config_home: root.join("config"),
            state_home: root.clone(),
            usr_share_themes: root.join("system"),
        });
        assert_eq!(t.bg, Rgb::new(0x05, 0x18, 0x2e));
        assert_eq!(t.fg, Rgb::new(0xf6, 0xdc, 0xac));
    }
}
