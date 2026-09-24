//! Resolución del tema de Omarchy (ARCHITECTURE.md §12).
//!
//! Orden de resolución (primer acierto gana):
//!   1. `$XDG_CONFIG_HOME/omarchy-jezzball/theme.ron` (override del usuario).
//!   2. `$XDG_STATE_HOME/omarchy/current/theme/colors.toml`.
//!   3. Nombre del tema en `.../current/theme.name` + `colors.toml` en
//!      `/usr/share/omarchy/themes/<nombre>/`.
//!   4. `~/.config/omarchy/themes/<nombre>/colors.toml`.
//!   5. `alacritty.toml` del tema (`colors.primary.*` / `colors.normal.*`).
//!   6. Fallback hardcodeado.
//!
//! Nunca se hace panic por rutas de I/O: cada paso falla y se degrada con
//! elegancia hasta el fallback final. La función `resolve_theme` recibe los
//! directorios base por parámetro para poder testearse sin tocar el sistema.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Color RGB (8 bits por canal). Independiente de macroquad para que todo el
/// mapeo de temas sea puro y testeable sin ventana.
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

    /// Convierte a `macroquad::color::Color` con la transpariencia indicada.
    pub fn to_mq(self, a: f32) -> macroquad::color::Color {
        macroquad::color::Color::new(
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            a,
        )
    }
}

/// Paleta del juego, derivada del tema activo de Omarchy. Ningún render hardcodea
/// un color: todo sale de aquí (ARCHITECTURE.md §12 "Mapeo de roles").
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

/// Colores del fallback hardcodeado (ARCHITECTURE.md §12, paso 6).
pub const FB_BG: Rgb = Rgb { r: 0x0f, g: 0x0f, b: 0x14 };
pub const FB_BG_PANEL: Rgb = Rgb { r: 0x16, g: 0x16, b: 0x1e };
pub const FB_FG: Rgb = Rgb { r: 0xc9, g: 0xd1, b: 0xd9 };
pub const FB_FG_DIM: Rgb = Rgb { r: 0x6e, g: 0x6e, b: 0x79 };
pub const FB_WALL: Rgb = Rgb { r: 0x56, g: 0x5f, b: 0x89 };
pub const FB_WALL_BUILDING: Rgb = Rgb { r: 0xe0, g: 0xaf, b: 0x68 };
pub const FB_BALL: Rgb = Rgb { r: 0x44, g: 0x9d, b: 0xab };
pub const FB_BALL_SPECIAL: Rgb = Rgb { r: 0xbb, g: 0x9a, b: 0xf7 };
pub const FB_DANGER: Rgb = Rgb { r: 0xf7, g: 0x76, b: 0x8e };
pub const FB_OK: Rgb = Rgb { r: 0x9e, g: 0xce, b: 0x6a };
pub const FB_ACCENT: Rgb = Rgb { r: 0x7a, g: 0xa2, b: 0xf7 };

/// Constructor del fallback marcado por contrato.
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

/// Directorios base que participan en la resolución del tema. Se inyectan por
/// parámetro para no depender del entorno en los tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OmarchyDirs {
    pub config_home: PathBuf,
    pub state_home: PathBuf,
    pub usr_share_themes: PathBuf,
}

impl OmarchyDirs {
    /// Calcula los directorios desde las variables XDG reales.
    pub fn from_env() -> Self {
        OmarchyDirs {
            config_home: crate::persist::config_home_dir(),
            state_home: crate::persist::state_home_dir(),
            usr_share_themes: PathBuf::from("/usr/share/omarchy/themes"),
        }
    }
}

/// Parsea `#rrggbb` (con o sin almohadilla). Devuelve `None` ante cadenas raras.
pub fn parse_hex(s: &str) -> Option<Rgb> {
    let t = s.trim().strip_prefix('#').unwrap_or(s.trim());
    if t.len() != 6 {
        return None;
    }
    let comp = |i: usize| u8::from_str_radix(&t[i..i + 2], 16).ok();
    Some(Rgb::new(comp(0)?, comp(2)?, comp(4)?))
}

/// Lee un valor `get` de un `toml::Value`, esperando una cadena de color.
fn table_get<'a>(v: &'a toml::Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(|x| x.as_str())
}

/// Color opcional de una tabla por un conjunto de claves alternativas.
fn color_or(v: &toml::Value, keys: &[&str]) -> Option<Rgb> {
    keys.iter().find_map(|k| table_get(v, k)).and_then(parse_hex)
}

/// Construye `Theme` a partir de un `colors.toml` plano (claves directas).
/// Exige `background` y `foreground`; el resto cae a alternativas del propio
/// tema o al fallback por rol.
fn theme_from_flat(v: &toml::Value) -> Option<Theme> {
    let bg = color_or(v, &["background", "darker_background"])?;
    let fg = color_or(v, &["foreground", "bright_foreground"])?;
    Some(Theme {
        name: String::new(),
        bg,
        bg_panel: color_or(v, &["lighter_background", "background", "selection"]).unwrap_or(bg),
        fg,
        fg_dim: color_or(v, &["dark_foreground", "brown", "muted"]).unwrap_or(fg),
        wall: color_or(v, &["muted", "blue", "cyan"]).unwrap_or(FB_WALL),
        wall_building: color_or(v, &["yellow", "orange"]).unwrap_or(FB_WALL_BUILDING),
        ball: color_or(v, &["cyan", "blue"]).unwrap_or(fg),
        ball_special: color_or(v, &["magenta", "red"]).unwrap_or(FB_BALL_SPECIAL),
        danger: color_or(v, &["red"]).unwrap_or(FB_DANGER),
        ok: color_or(v, &["green"]).unwrap_or(FB_OK),
        accent: color_or(v, &["accent", "orange", "magenta"]).unwrap_or(FB_ACCENT),
    })
}

/// Construye `Theme` a partir de un `alacritty.toml` (`colors.primary.*` y
/// `colors.normal.*`). Exige `primary.background` y `primary.foreground`.
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

/// Intenta construir un tema desde un TOML, indistintamente de si es plano o
/// estilo alacritty.
fn theme_from_value(v: &toml::Value) -> Option<Theme> {
    theme_from_flat(v).or_else(|| theme_from_alacritty(v))
}

/// Lee `colors.toml` o `alacritty.toml` de un fichero. El nombre de fuente se
/// usa como etiqueta del tema. Nunca devuelve error: solo `None`.
fn load_colors_file(path: &Path) -> Option<Theme> {
    let content = fs::read_to_string(path).ok()?;
    let v: toml::Value = toml::from_str(&content).ok()?;
    theme_from_value(&v)
}

/// Lee el override de usuario en RON (paso 1). Ignora errores de parseo.
fn load_ron_override(path: &Path) -> Option<Theme> {
    let content = fs::read_to_string(path).ok()?;
    ron::from_str(&content).ok()
}

/// Lee el nombre del tema activo (`theme.name`). Rechaza nombres con
/// separadores de ruta para evitar traversal.
fn read_theme_name(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    let name = raw.trim();
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        None
    } else {
        Some(name.to_string())
    }
}

/// Resolución completa del tema según ARCHITECTURE.md §12. Primera fuente con
/// éxito gana; si ninguna aparece, devuelve el fallback hardcodeado.
pub fn resolve_theme(dirs: &OmarchyDirs) -> Theme {
    // 1. Override del usuario.
    let override_path = dirs
        .config_home
        .join("omarchy-jezzball")
        .join("theme.ron");
    if let Some(t) = load_ron_override(&override_path) {
        return t;
    }

    let current = dirs
        .state_home
        .join("omarchy")
        .join("current")
        .join("theme");

    // 2. Tema activo symlinkeado (colors.toml plano).
    if let Some(mut t) = load_colors_file(&current.join("colors.toml")) {
        t.name = "corriente".to_string();
        return t;
    }

    let name = read_theme_name(&dirs.state_home.join("omarchy/current/theme.name"));

    if let Some(n) = name.as_deref() {
        // 3. Tema del sistema.
        let sys = dirs.usr_share_themes.join(n);
        if let Some(mut t) = load_colors_file(&sys.join("colors.toml")) {
            t.name = n.to_string();
            return t;
        }
        // 4. Tema de usuario.
        let user = dirs.config_home.join("omarchy/themes").join(n);
        if let Some(mut t) = load_colors_file(&user.join("colors.toml")) {
            t.name = n.to_string();
            return t;
        }
        // 5. Última fuente real: alacritty.toml del tema.
        if let Some(t) = load_colors_file(&sys.join("alacritty.toml")) {
            return t;
        }
        if let Some(t) = load_colors_file(&user.join("alacritty.toml")) {
            return t;
        }
    }

    // 5b. Alacritty del tema "current" (si no hubo nombre).
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

    /// Directorio temporal para tests que tocan disco, dentro del repo
    /// (el sandbox bloquea /tmp y ~/.local, pero el repo es de escritura).
    fn tmp_root(name: &str) -> PathBuf {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/.tmp")
            .join(format!("{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("crear tmp");
        root
    }

    #[test]
    fn parse_hex_funciona() {
        assert_eq!(parse_hex("  #05182e  "), Some(Rgb::new(0x05, 0x18, 0x2e)));
        assert_eq!(parse_hex("f6dcac"), Some(Rgb::new(0xf6, 0xdc, 0xac)));
        assert_eq!(parse_hex("#zzzzzz"), None);
        assert_eq!(parse_hex("abc"), None);
        assert_eq!(parse_hex(""), None);
    }

    #[test]
    fn retro82_plano() {
        let v: toml::Value = toml::from_str(&read_fixture("retro-82-colors.toml")).unwrap();
        let t = theme_from_flat(&v).expect("tema parseable");
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
    fn tokyo_night_plano() {
        let v: toml::Value = toml::from_str(&read_fixture("tokyo-night-colors.toml")).unwrap();
        let t = theme_from_flat(&v).expect("tema parseable");
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
        // accent cae a magenta/blue de normal.
        assert_ne!(t.accent, FB_ACCENT);
    }

    #[test]
    fn sin_ficheros_devuelve_fallback_sin_panic() {
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
    fn override_de_usuario_gana() {
        let root = tmp_root("override");
        let cfg = root.join("omarchy-jezzball");
        fs::create_dir_all(&cfg).unwrap();
        fs::write(
            cfg.join("theme.ron"),
            r#"(
                name: "custom",
                bg: (r: 1, g: 2, b: 3),
                bg_panel: (r: 4, g: 5, b: 6),
                fg: (r: 7, g: 8, b: 9),
                fg_dim: (r: 10, g: 11, b: 12),
                wall: (r: 13, g: 14, b: 15),
                wall_building: (r: 16, g: 17, b: 18),
                ball: (r: 19, g: 20, b: 21),
                ball_special: (r: 22, g: 23, b: 24),
                danger: (r: 25, g: 26, b: 27),
                ok: (r: 28, g: 29, b: 30),
                accent: (r: 31, g: 32, b: 33),
            )"#,
        )
        .unwrap();
        let dirs = OmarchyDirs {
            config_home: root.clone(),
            state_home: root.join("state"),
            usr_share_themes: PathBuf::from("/nonexistent"),
        };
        let t = resolve_theme(&dirs);
        assert_eq!(t.name, "custom");
        assert_eq!(t.bg, Rgb::new(1, 2, 3));
        assert_eq!(t.accent, Rgb::new(31, 32, 33));
    }

    #[test]
    fn current_colors_toml_del_estado() {
        let root = tmp_root("current");
        let theme_dir = root.join("omarchy/current/theme");
        fs::create_dir_all(&theme_dir).unwrap();
        fs::copy(fixture("retro-82-colors.toml"), theme_dir.join("colors.toml")).unwrap();
        let dirs = OmarchyDirs {
            config_home: root.join("config"),
            state_home: root.clone(),
            usr_share_themes: PathBuf::from("/nonexistent"),
        };
        let t = resolve_theme(&dirs);
        assert_eq!(t.name, "corriente");
        assert_eq!(t.bg, Rgb::new(0x05, 0x18, 0x2e));
    }

    #[test]
    fn nombre_de_tema_busca_en_sistema_y_usuario() {
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

        // Tema de usuario sin colors.toml en sistema.
        let user = root.join("config/omarchy/themes/retro82");
        fs::create_dir_all(&user).unwrap();
        fs::copy(fixture("retro-82-colors.toml"), user.join("colors.toml")).unwrap();
        fs::write(state.join("theme.name"), "retro82\n").unwrap();
        let t = resolve_theme(&dirs);
        assert_eq!(t.name, "retro82");
        assert_eq!(t.accent, Rgb::new(0xfa, 0xa9, 0x68));
    }

    #[test]
    fn nombre_malicioso_no_viaja() {
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
    fn alacritty_como_ultimo_recurso() {
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