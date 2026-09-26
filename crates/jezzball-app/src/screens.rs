//! Screen state machine and APP logic (ARCHITECTURE.md §6 and §13).
//!
//! Rules: the game logic lives in `jezzball-core`; here we only orchestrate
//! `step()` with the player's input, update records/persistence and
//! decide the transitions between screens. No `unsafe`, no panics.

use std::fs;
use std::path::{Path, PathBuf};

use macroquad::prelude::*;
use macroquad::window::screen_height;
use macroquad::window::screen_width;

use jezzball_core::level::{LevelSpec, Mode};
use jezzball_core::state::{step, GameEvent, GamePhase, GameState, PlayerInput};
use jezzball_core::wall::WallAxis;

use crate::input::{NavRepeat, UiKey};
use crate::persist::{load_save, save_save, SaveData};
use crate::render::{mode_name, Layout};
use crate::theme::{resolve_theme, OmarchyDirs, Theme};

/// Minimum seconds the results screen is shown before accepting
/// input (it prevents advancing because of a stray click from the previous level).
const RESULTS_WAIT: f32 = 0.4;

/// Screens of the state machine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Screen {
    Menu,
    ModeMenu,
    Selector,
    Playing,
    Results,
    ModeComplete,
    Help,
    Error,
}

/// One menu option of the active list (`menu_items`).
#[derive(Clone, Debug)]
pub struct MenuItem {
    pub label: String,
    pub enabled: bool,
}

/// Kind of floating text (it cuts the event after the fact).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FloaterKind {
    Good,
    Bad,
    Combo,
    Neutral,
}

/// Animated floating text (points, combos, floaters).
#[derive(Clone, Debug)]
pub struct Floater {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub ttl: f32,
    pub kind: FloaterKind,
}

/// Brief floater at the bottom centre.
#[derive(Clone, Debug)]
pub struct Toast {
    pub text: String,
    pub ttl: f32,
}

/// Data for the results screen.
#[derive(Clone, Debug)]
pub struct ResultsData {
    pub won: bool,
    pub stars: u8,
    pub prev_stars: u8,
    pub new_best: bool,
    pub score: u32,
    pub prev_best: u32,
    pub time: f32,
    pub no_next: bool,
    pub next_world: Option<u8>,
    pub level: u16,
}

/// Levels loaded per mode, sorted by `id`.
#[derive(Debug, Clone)]
pub struct Levels {
    pub original: Vec<LevelSpec>,
    pub enhanced: Vec<LevelSpec>,
}

impl Levels {
    pub fn list(&self, mode: Mode) -> &[LevelSpec] {
        match mode {
            Mode::Original => &self.original,
            Mode::Enhanced => &self.enhanced,
        }
    }

    pub fn len(&self, mode: Mode) -> u16 {
        self.list(mode).len() as u16
    }

    pub fn get(&self, mode: Mode, idx: u16) -> Option<&LevelSpec> {
        self.list(mode).get(idx as usize)
    }
}

/// Global state of the app layer.
pub struct App {
    pub theme: Theme,
    pub levels: Levels,
    pub save: SaveData,
    pub screen: Screen,
    pub state: GameState,
    pub level_number: u16,
    pub mode: Mode,
    pub frame: crate::input::Frame,
    pub mouse_pos: (f32, f32),
    pub select: usize,
    pub nav: NavRepeat,
    pub floaters: Vec<Floater>,
    pub toast: Option<Toast>,
    pub results: ResultsData,
    pub menu_items: Vec<MenuItem>,
    pub selector_mode: Mode,
    pub error_msg: String,
    /// Concrete paths tried when loading the levels (diagnostics: they are
    /// shown on the error screen if none are found).
    pub candidates: Vec<PathBuf>,
    pub hud_compact: bool,
    pub quit_confirm: bool,
    pub quit_requested: bool,
    /// Protection countdown for the results screen.
    pub results_wait: f32,
    /// Visible page of the help screen (0..HELP_PAGES).
    pub help_page: usize,
    /// First visible item of the selector list: without this, with 61
    /// levels the list ran off the screen and the selection was not visible.
    pub list_top: usize,
    /// List rows that fit on the screen. The render loop refreshes it
    /// (the only place that knows the real height of the window) and the
    /// scrolling logic consumes it, which is how it avoids depending on macroquad.
    pub visible_rows: usize,
}

impl App {
    pub fn new() -> Self {
        let theme = resolve_theme(&OmarchyDirs::from_env());
        let (levels, candidates) = load_levels();
        if levels.original.is_empty() && levels.enhanced.is_empty() {
            eprintln!("omarchy-jezzball: no se encontraron niveles. Rutas probadas:");
            for c in &candidates {
                eprintln!("  {}", c.display());
            }
            eprintln!(
                "Set OMARCHY_JEZZBALL_ASSETS to the directory containing 'levels/' \
                 to point at it by hand."
            );
        }
        let save = load_save();
        let mut app = App {
            theme,
            levels,
            candidates,
            save,
            screen: Screen::Menu,
            state: GameState::new(fallback_level()),
            level_number: 0,
            mode: Mode::Original,
            frame: crate::input::Frame::default(),
            mouse_pos: (0.0, 0.0),
            select: 0,
            nav: NavRepeat::default(),
            floaters: Vec::new(),
            toast: None,
            results: ResultsData {
                won: false,
                stars: 0,
                prev_stars: 0,
                new_best: false,
                score: 0,
                prev_best: 0,
                time: 0.0,
                no_next: false,
                next_world: None,
                level: 1,
            },
            menu_items: Vec::new(),
            selector_mode: Mode::Original,
            error_msg: String::new(),
            hud_compact: false,
            quit_confirm: false,
            quit_requested: false,
            results_wait: RESULTS_WAIT,
            help_page: 0,
            list_top: 0,
            visible_rows: 12,
        };
        open_menu(&mut app);
        app
    }
}

/// A minimal emergency `LevelSpec` to initialise `App` (it is never
/// played: every screen loads the levels from disk before playing).
fn fallback_level() -> LevelSpec {
    LevelSpec {
        id: 1,
        name: "fallback".into(),
        world: 0,
        kind: jezzball_core::level::LevelKind::Standard,
        arena: jezzball_core::level::ArenaSpec {
            w: 16,
            h: 12,
            shape: jezzball_core::level::ArenaShape::Rect,
            obstacles: vec![],
        },
        balls: vec![jezzball_core::level::BallSpawn {
            x: 8.0,
            y: 6.0,
            vx: 18.0,
            vy: 18.0,
            kind: jezzball_core::level::BallKind::Normal,
            radius_mul: 1.0,
        }],
        target_ratio: 0.95,
        lives: 3,
        wall_speed: 22.0,
        time_limit: None,
        powerups_enabled: false,
        objectives: vec![],
        purist: true,
        seed: 1,
    }
}

// --- Level loading ---

/// Environment variable override for development and tests: it points to the repo's
/// `assets/` folder, which contains `levels/`. See `level_candidates`.
const ENV_ASSETS: &str = "OMARCHY_JEZZBALL_ASSETS";

/// Loaded levels + candidate paths tried (for diagnostics).
fn load_levels() -> (Levels, Vec<PathBuf>) {
    let dirs = level_candidates(
        current_exe_dir().as_deref(),
        &crate::persist::data_home_dir(),
        std::env::var_os(ENV_ASSETS).map(PathBuf::from).as_deref(),
    );
    let levels = load_levels_in(&dirs);
    (levels, dirs)
}

/// Directory of the active binary (`std::env::current_exe()`), or `None` if the
/// OS cannot resolve it (never panics).
fn current_exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

/// Candidate directories where `original.ron` / `enhanced.ron` may live,
/// in priority order. A PURE function (it does not touch the OS) so the
/// order can be tested without environment side effects. Order documented in the README:
/// 1. `$OMARCHY_JEZZBALL_ASSETS/levels`         (development/test override)
/// 2. `<exe dir>/assets/levels`                 (binary next to assets)
/// 3. `<exe dir>/../share/omarchy-jezzball/levels` (install.sh: ~/.local)
/// 4. `$XDG_DATA_HOME/omarchy-jezzball/levels`  (fallback ~/.local/share)
/// 5. `/usr/share/omarchy-jezzball/levels`      (PKGBUILD / pacman)
/// 6. `./assets/levels`                         (CWD: `cargo run` in the repo)
fn level_candidates(
    exe_dir: Option<&Path>,
    data_home: &Path,
    env_override: Option<&Path>,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(assets) = env_override {
        out.push(normalize_path(&assets.join("levels")));
    }
    if let Some(exe) = exe_dir {
        out.push(normalize_path(&exe.join("assets").join("levels")));
        out.push(normalize_path(
            &exe.join("..")
                .join("share")
                .join("omarchy-jezzball")
                .join("levels"),
        ));
    }
    out.push(normalize_path(
        &data_home.join("omarchy-jezzball").join("levels"),
    ));
    out.push(PathBuf::from("/usr/share/omarchy-jezzball/levels"));
    out.push(PathBuf::from("assets/levels"));
    // With ~/.local (install.sh) candidates 3 and 4 converge on the same
    // path: it is collapsed so the diagnostics do not repeat directories.
    out.dedup();
    out
}

/// Collapses redundant `.` and `..` from a path (pure, it never does I/O).
fn normalize_path(p: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(c.as_os_str());
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn load_levels_in(dirs: &[PathBuf]) -> Levels {
    Levels {
        original: load_level_list(dirs, "original.ron").unwrap_or_default(),
        enhanced: load_level_list(dirs, "enhanced.ron").unwrap_or_default(),
    }
}

fn load_level_list(dirs: &[PathBuf], file: &str) -> Result<Vec<LevelSpec>, String> {
    let mut last_err = String::new();
    for dir in dirs {
        match read_level_list(&dir.join(file)) {
            Ok(mut list) => {
                list.sort_by_key(|l| l.id);
                return Ok(list);
            }
            Err(e) => last_err = e,
        }
    }
    Err(format!("no se encontraron niveles {file}: {last_err}"))
}

fn read_level_list(path: &Path) -> Result<Vec<LevelSpec>, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse_levels(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn parse_levels(text: &str) -> Result<Vec<LevelSpec>, String> {
    let list: Vec<LevelSpec> = ron::from_str(text).map_err(|e| format!("invalid RON: {e}"))?;
    Ok(list.into_iter().filter(|l| l.id >= 1).collect())
}

// --- Navigation / menus ---

fn open_menu(app: &mut App) {
    app.screen = Screen::Menu;
    // Enhanced is earned: until the 10 Original levels are completed, the option appears
    // locked and the label says why (a bare "(BLOQUEADO)" would leave the
    // player with no idea what to do).
    let enh_ok = original_mode_completed(app);
    let enh_label = if enh_ok {
        crate::i18n::t("menu.modo_enhanced").to_string()
    } else {
        crate::i18n::t("menu.enhanced_bloqueado")
            .replace("{}", &app.levels.len(Mode::Original).to_string())
    };
    app.menu_items = vec![
        MenuItem {
            label: crate::i18n::t("menu.modo_original").into(),
            enabled: !app.levels.original.is_empty(),
        },
        MenuItem {
            label: enh_label,
            enabled: !app.levels.enhanced.is_empty() && enh_ok,
        },
        MenuItem {
            label: crate::i18n::t("menu.elegir_nivel").into(),
            enabled: true,
        },
        MenuItem {
            label: crate::i18n::t("menu.como_se_juega").into(),
            enabled: true,
        },
    ];
    app.select = 0;
    app.nav = NavRepeat::default();
}

fn open_mode_menu(app: &mut App) {
    app.screen = Screen::ModeMenu;
    app.menu_items = vec![
        MenuItem {
            label: crate::i18n::t("menu.modo_original").into(),
            enabled: !app.levels.original.is_empty(),
        },
        MenuItem {
            label: crate::i18n::t("menu.modo_enhanced").into(),
            enabled: !app.levels.enhanced.is_empty() && original_mode_completed(app),
        },
    ];
    app.select = 0;
    app.nav = NavRepeat::default();
}

fn open_selector(app: &mut App, mode: Mode) {
    app.selector_mode = mode;
    app.screen = Screen::Selector;
    let mut items = vec![MenuItem {
        label: crate::i18n::t("menu.empieza_nivel_1").into(),
        enabled: true,
    }];
    let list = app.levels.list(mode);
    for (i, spec) in list.iter().enumerate() {
        items.push(MenuItem {
            label: crate::i18n::t("menu.nivel_item")
                .replacen("{}", &i.saturating_add(1).to_string(), 1)
                .replacen("{}", &spec.name, 1),
            enabled: is_unlocked(app, mode, i as u16),
        });
    }
    app.menu_items = items;
    // Start on the first playable level, not on level 1 if it is already beaten:
    // with 61 levels, always opening at the top forces dozens of downward moves.
    let first = app.menu_items.iter().position(|i| i.enabled).unwrap_or(0);
    app.select = first;
    app.list_top = 0;
    adjust_scroll_window(app);
    app.nav = NavRepeat::default();
}

/// Typical vertical navigation over `menu_items` (it skips options disabled
/// by double keys).
fn nav_menu(app: &mut App, dt: f32) {
    if let Some(d) = app.nav.input(
        &app.frame,
        &crate::input::NAV_UP,
        &crate::input::NAV_DOWN,
        dt,
    ) {
        let n = app.menu_items.len();
        if n > 0 {
            let mut sel = app.select as i64 + d as i64;
            let mut guard = 0;
            while guard < n && (sel < 0 || sel >= n as i64 || !app.menu_items[sel as usize].enabled)
            {
                sel = (sel + d as i64).rem_euclid(n as i64);
                guard += 1;
            }
            app.select = sel.clamp(0, n as i64 - 1) as usize;
            adjust_scroll_window(app);
        }
    }
}

/// Visible list rows, for the scrolling logic.
///
/// It cannot ask the render layer: macroquad's `screen_height()` panics if there
/// is no window, and that made testing the scroll impossible. The render layer uses this
/// same function, so both count the same.
pub fn visible_rows_for(height: f32, scale: f32) -> usize {
    let row = 30.0 * scale;
    let available = (height - 170.0 * scale - 20.0 * scale - 24.0 * scale).max(row);
    ((available / row).floor() as usize).max(1)
}

/// Keeps `list_top` such that `select` always stays inside the
/// visible window of the list. Without this, with 61 levels the player moved the
/// selection off the screen and kept seeing the first levels.
fn adjust_scroll_window(app: &mut App) {
    let visible = app.visible_rows;
    let n = app.menu_items.len();
    if n <= visible {
        app.list_top = 0;
        return;
    }
    if app.select < app.list_top {
        app.list_top = app.select;
    } else if app.select >= app.list_top + visible {
        app.list_top = app.select + 1 - visible;
    }
    app.list_top = app.list_top.min(n - visible);
}

/// Persisted progress per mode, for the menu.
pub fn mode_progress(app: &App, mode: Mode) -> (usize, u32) {
    let total = app.levels.len(mode);
    let done = app.save.completed_count(mode).min(total as usize);
    let stars = app.save.total_stars(mode);
    (done, stars)
}

/// Is this level unlocked for playing? (partial progress in Original →
/// unlocks Enhancement as a mode to replay whole ones angelically)
fn is_unlocked(app: &App, mode: Mode, idx: u16) -> bool {
    // Entry gate to Enhanced: it is checked BEFORE anything else, including the
    // first level. Enhanced is the evolution of the classic mechanic, so it
    // requires the 10 Original levels completed; otherwise, a new player
    // would start on levels with obstacles and special balls without having
    // learned the basics.
    if mode == Mode::Enhanced && !original_mode_completed(app) {
        return false;
    }
    if idx == 0 {
        return true;
    }
    if app.levels.list(mode).is_empty() {
        return false;
    }
    // In Original (purist, 10 levels) the progression is strict: the previous level
    // completed, unless the mode has already been finished (free replay).
    if mode == Mode::Original {
        if original_mode_completed(app) {
            return true;
        }
        return prev_completed(app, mode, idx);
    }
    // Inside Enhanced there is a thematic gate: worlds 2..=6 require a
    // minimum progress in Original (§7: original level n => enhanced world w,
    // with n >= 2(w-1)). The whole world is unlocked at once (any level of
    // that world can be played, with no internal sequence).
    let spec = match app.levels.get(mode, idx) {
        Some(s) => s,
        None => return false,
    };
    if spec.world > 1 {
        let required = (spec.world as u16 - 1) * 2;
        let orig_ok = app
            .levels
            .get(Mode::Original, required.saturating_sub(1))
            .is_some_and(|l| {
                app.save
                    .record(Mode::Original, l.id)
                    .is_some_and(|r| r.completed)
            });
        if !orig_ok {
            return false;
        }
    }
    true
}

fn prev_completed(app: &App, mode: Mode, idx: u16) -> bool {
    // `idx` is 0-based: level 0 has no previous one. Subtracting without checking
    // overflowed the `u16` and in debug builds that PANICS, closing the
    // game when querying the unlocking of the first level.
    let Some(prev_idx) = idx.checked_sub(1) else {
        return false;
    };
    let Some(prev_spec) = app.levels.get(mode, prev_idx) else {
        return false;
    };
    app.save
        .record(mode, prev_spec.id)
        .is_some_and(|r| r.completed)
}

fn original_mode_completed(app: &App) -> bool {
    app.save.original_completed
        || app.save.completed_count(Mode::Original) >= app.levels.len(Mode::Original) as usize
}

// --- Starting games ---

/// Error message when a mode has no levels: besides the warning, it lists
/// the concrete paths that were tried, one per line, so the user
/// knows where to place `original.ron` / `enhanced.ron`.
fn no_levels_message(mode: Mode, candidates: &[PathBuf]) -> String {
    let paths: Vec<String> = candidates
        .iter()
        .map(|c| format!("  {}", c.display()))
        .collect();
    let mut msg = crate::i18n::t("error.sin_niveles").replace("{}", mode_name(mode));
    if !paths.is_empty() {
        msg.push_str(crate::i18n::t("error.rutas_probadas"));
        msg.push_str(&paths.join("\n"));
        msg.push_str(crate::i18n::t("error.coloca_niveles"));
    }
    msg
}

fn start_mode(app: &mut App, mode: Mode) {
    if app.levels.list(mode).is_empty() {
        app.error_msg = no_levels_message(mode, &app.candidates);
        app.screen = Screen::Error;
        app.nav = NavRepeat::default();
        return;
    }
    // Modes already completed: they open the selector to replay levels.
    let completed = if mode == Mode::Original {
        original_mode_completed(app)
    } else {
        app.save.completed_count(Mode::Enhanced) >= app.levels.len(Mode::Enhanced) as usize
    };
    if completed {
        open_selector(app, mode);
        return;
    }
    start_level(app, mode, 0);
}

fn start_level(app: &mut App, mode: Mode, idx: u16) {
    let Some(spec) = app.levels.get(mode, idx).cloned() else {
        // `idx + 1` goes from 0-based to 1-based for the message, but with
        // `idx == u16::MAX` it would overflow and in debug that panics. `saturating_add`
        // keeps the message readable without ever being able to break.
        app.error_msg = crate::i18n::t("error.nivel_no_existe")
            .replace("{}", &idx.saturating_add(1).to_string());
        app.screen = Screen::Error;
        app.nav = NavRepeat::default();
        return;
    };
    app.mode = mode;
    app.level_number = idx;
    app.state = GameState::new(spec);
    app.screen = Screen::Playing;
    app.floaters.clear();
    app.toast = None;
    app.hud_compact = false;
    app.nav = NavRepeat::default();
}

/// Initial startup from the CLI: `--mode` and/or `--level` (1-based).
pub fn launch(app: &mut App, mode: Option<Mode>, level: Option<u16>) {
    let mode = mode.unwrap_or(Mode::Original);
    if let Some(n) = level {
        if n >= 1 {
            let idx = n - 1;
            if app.levels.get(mode, idx).is_some() {
                start_level(app, mode, idx);
                return;
            }
            app.error_msg = crate::i18n::t("error.nivel_no_existe_en")
                .replacen("{}", &n.to_string(), 1)
                .replacen("{}", mode_name(mode), 1);
            app.screen = Screen::Error;
            app.nav = NavRepeat::default();
            return;
        }
    }
    start_mode(app, mode);
}

// --- Update loop ---

pub fn update(app: &mut App, dt: f32) {
    app.frame = crate::input::sample_frame();
    app.mouse_pos = (app.frame.mouse.x, app.frame.mouse.y);
    // The window height can change (resizing, full screen),
    // so the visible row count is refreshed every frame here, where
    // there is a window, and the scroll logic consumes it without touching macroquad.
    app.visible_rows = crate::render::menu::visible_rows();

    if app.quit_confirm {
        update_quit_confirm(app);
        return;
    }

    match app.screen {
        Screen::Menu => update_menu(app, dt),
        Screen::ModeMenu => update_mode_menu(app, dt),
        Screen::Selector => update_selector(app, dt),
        Screen::Playing => update_playing(app, dt),
        Screen::Results => update_results(app, dt),
        Screen::ModeComplete => update_mode_complete(app, dt),
        Screen::Help => update_help(app, dt),
        Screen::Error => update_error(app),
    }

    // Animations independent of the screen.
    for f in &mut app.floaters {
        f.y -= 16.0 * dt;
        f.ttl -= dt;
    }
    app.floaters.retain(|f| f.ttl > 0.0);
    if let Some(t) = &mut app.toast {
        t.ttl -= dt;
        if t.ttl <= 0.0 {
            app.toast = None;
        }
    }
}

fn update_menu(app: &mut App, dt: f32) {
    nav_menu(app, dt);
    if app.frame.pressed(UiKey::Escape) {
        app.quit_confirm = true;
        return;
    }
    if !confirm_pressed(app) {
        return;
    }
    match app.select {
        0 => start_mode(app, Mode::Original),
        1 => start_mode(app, Mode::Enhanced),
        2 => open_mode_menu(app),
        _ => open_help(app),
    }
}

/// Help pages. The contents live in the language catalogs
/// (`assets/i18n/*.ron`), not here: adding a language does not touch this code.
pub const HELP_PAGES: usize = 4;

/// Title and body of a help page, translated.
/// Lines starting with "# " are subheadings and the render layer highlights them.
pub fn help_page(n: usize) -> (&'static str, Vec<&'static str>) {
    let (kt, kc) = match n {
        0 => ("ayuda.p1.title", "ayuda.p1.body"),
        1 => ("ayuda.p2.title", "ayuda.p2.body"),
        2 => ("ayuda.p3.title", "ayuda.p3.body"),
        _ => ("ayuda.p4.title", "ayuda.p4.body"),
    };
    (crate::i18n::t(kt), crate::i18n::t(kc).lines().collect())
}

/// Help screen: rules, controls and the catalog of Enhanced obstacles, balls and
/// power-ups. Reachable from the menu and from the pause screen.
fn open_help(app: &mut App) {
    app.screen = Screen::Help;
    app.help_page = 0;
    app.nav = NavRepeat::default();
}

fn update_help(app: &mut App, _dt: f32) {
    if app.frame.pressed(UiKey::Escape) {
        open_menu(app);
        return;
    }
    // Pagination: down/right advances, up/left goes back.
    if app.frame.pressed_any(&[UiKey::Down, UiKey::J, UiKey::L]) {
        app.help_page = (app.help_page + 1).min(HELP_PAGES.saturating_sub(1));
    }
    if app.frame.pressed_any(&[UiKey::Up, UiKey::K, UiKey::H]) {
        app.help_page = app.help_page.saturating_sub(1);
    }
    if confirm_pressed(app) {
        if app.help_page + 1 >= HELP_PAGES {
            open_menu(app);
        } else {
            app.help_page += 1;
        }
    }
}

fn update_mode_menu(app: &mut App, dt: f32) {
    nav_menu(app, dt);
    if app.frame.pressed(UiKey::Escape) {
        open_menu(app);
        return;
    }
    if !confirm_pressed(app) {
        return;
    }
    match app.select {
        0 => open_selector(app, Mode::Original),
        _ => open_selector(app, Mode::Enhanced),
    }
}

fn update_selector(app: &mut App, dt: f32) {
    nav_menu(app, dt);
    if app.frame.pressed(UiKey::Escape) {
        open_menu(app);
        return;
    }
    if !confirm_pressed(app) {
        return;
    }
    let mode = app.selector_mode;
    if app.select == 0 {
        start_level(app, mode, 0);
    } else if let Some(spec) = app.levels.get(mode, app.select as u16 - 1) {
        // Level ids are 1-based by contract (LEVEL_SCHEMA.md), but a
        // hand-edited .ron could carry `id: 0`; without `saturating_sub` that
        // would overflow the u16 and in debug would close the game.
        let idx = spec.id.saturating_sub(1);
        start_level(app, mode, idx);
    }
}

/// Enter, Space or the controller key (in keyboard menus).
fn confirm_pressed(app: &App) -> bool {
    app.frame
        .pressed_any(&[UiKey::Enter, UiKey::Space, UiKey::M])
}

// --- Game screen ---

fn update_playing(app: &mut App, dt: f32) {
    match app.state.phase {
        GamePhase::Paused => {
            if app.frame.pressed(UiKey::Space) || app.frame.pressed(UiKey::Escape) {
                let (ns, _) = step(&app.state, PlayerInput::Resume, 0.0);
                app.state = ns;
            } else if app.frame.pressed(UiKey::R) {
                let (ns, _) = step(&app.state, PlayerInput::Restart, 0.0);
                app.state = ns;
            } else if app.frame.pressed(UiKey::H) {
                open_help(app);
            } else if app.frame.pressed(UiKey::M) {
                // Leave THE LEVEL back to the menu, without closing the game.
                open_menu(app);
            } else if app.frame.pressed(UiKey::Q) {
                app.quit_confirm = true;
            }
            return;
        }
        GamePhase::Won | GamePhase::Lost => {
            finish_level(app);
            return;
        }
        GamePhase::Running => {}
    }

    if app.frame.pressed(UiKey::Q) {
        app.quit_confirm = true;
        return;
    }
    if app.frame.pressed(UiKey::F) {
        app.hud_compact = !app.hud_compact;
    }
    if app.frame.pressed(UiKey::Escape) || app.frame.pressed(UiKey::Space) {
        let (ns, _) = step(&app.state, PlayerInput::Pause, 0.0);
        app.state = ns;
        return;
    }
    if app.frame.pressed(UiKey::R) {
        let (ns, _) = step(&app.state, PlayerInput::Restart, 0.0);
        app.state = ns;
        let _ = dt;
        return;
    }

    let mut input = PlayerInput::None;

    if app.frame.pressed(UiKey::Tab) {
        input = PlayerInput::ToggleAxis;
    } else if let Some(d) = app.frame.digit() {
        if let Some(kind) = app.state.inventory.get(d).copied() {
            input = PlayerInput::UsePowerUp(kind);
        }
    } else {
        let left = app.frame.mouse.left;
        let right = app.frame.mouse.right;
        if (left || right) && !app.state.builders.is_empty() {
            // With a wall under construction the mouse cannot fire another one.
        } else if left || right {
            let max_builders = if app.state.pending_double_wall { 2 } else { 1 };
            if app.state.builders.len() < max_builders {
                let layout = Layout::compute(
                    screen_width(),
                    screen_height(),
                    app.state.arena.grid.w,
                    app.state.arena.grid.h,
                    hud_rows_for(app),
                );
                if let Some(cell) = layout.hovered_cell(app.mouse_pos.0, app.mouse_pos.1) {
                    let axis = if right {
                        opposite_axis(app.state.current_axis)
                    } else {
                        app.state.current_axis
                    };
                    input = PlayerInput::StartWall { cell, axis };
                }
            }
        }
    }

    let (ns, events) = step(&app.state, input, dt);
    app.state = ns;
    handle_events(app, events);

    if app.state.phase == GamePhase::Won || app.state.phase == GamePhase::Lost {
        finish_level(app);
    }
}

fn opposite_axis(a: WallAxis) -> WallAxis {
    match a {
        WallAxis::Horizontal => WallAxis::Vertical,
        WallAxis::Vertical => WallAxis::Horizontal,
    }
}

fn handle_events(app: &mut App, events: Vec<GameEvent>) {
    for ev in events {
        match ev {
            GameEvent::WallCompleted { points, .. } => {
                add_floater(app, FloaterKind::Good, format!("+{}", points));
            }
            GameEvent::WallBlocked => {
                add_floater(
                    app,
                    FloaterKind::Bad,
                    crate::i18n::t("aviso.bloqueado").into(),
                );
            }
            GameEvent::LifeLost => add_floater(
                app,
                FloaterKind::Bad,
                crate::i18n::t("aviso.vida_perdida").into(),
            ),
            GameEvent::BallLost => add_floater(
                app,
                FloaterKind::Neutral,
                crate::i18n::t("aviso.bola_perdida").into(),
            ),
            GameEvent::ComboUp(m) if m > 1 => add_floater(
                app,
                FloaterKind::Combo,
                crate::i18n::t("hud.combo_mayus").replace("{}", &m.to_string()),
            ),
            GameEvent::ComboReset | GameEvent::ComboUp(_) => {}
            GameEvent::PowerUpSpawned => {
                app.toast = Some(Toast {
                    text: crate::i18n::t("aviso.powerup").into(),
                    ttl: 2.0,
                });
            }
            GameEvent::WallStarted => {}
            GameEvent::LevelCleared { .. } => {}
            GameEvent::GameOver => {}
        }
    }
}

fn add_floater(app: &mut App, kind: FloaterKind, text: String) {
    app.floaters.push(Floater {
        text,
        x: app.mouse_pos.0,
        y: app.mouse_pos.1 - 8.0,
        ttl: 1.4,
        kind,
    });
}

// --- End of level / results ---

fn finish_level(app: &mut App) {
    let mode = app.mode;
    let won = app.state.phase == GamePhase::Won;
    let spec = app.state.level.clone();
    let id = spec.id;
    let score = app.state.score;
    let stars = app.state.stars;
    let time = app.state.elapsed;

    let prev = app.save.record(mode, id).copied().unwrap_or_default();
    let rec = app.save.record_for(mode, id);
    let new_best = score > prev.best_score;
    rec.best_score = rec.best_score.max(score);
    rec.stars = rec.stars.max(stars);
    if won {
        rec.completed = true;
        rec.best_time = if prev.completed {
            prev.best_time.min(time)
        } else {
            time
        };
    }

    if mode == Mode::Original && won && id == app.levels.len(Mode::Original) {
        app.save.original_completed = true;
    }

    let total = app.levels.len(mode);
    let no_next = id >= total;
    let mut next_world = None;
    if let Some(next) = app.levels.get(mode, id) {
        if next.world > 0 && next.world != spec.world {
            next_world = Some(next.world);
        }
    }

    app.results = ResultsData {
        won,
        stars,
        prev_stars: prev.stars,
        new_best,
        score,
        prev_best: prev.best_score,
        time,
        no_next,
        next_world,
        level: id,
    };
    app.results_wait = RESULTS_WAIT;
    app.screen = Screen::Results;

    // Immediate persistence (silent degradation if the disk fails).
    if !save_save(&app.save) {
        eprintln!("omarchy-jezzball: no se pudo guardar la partida");
    }
}

fn update_results(app: &mut App, dt: f32) {
    if app.results_wait > 0.0 {
        app.results_wait -= dt;
        return;
    }
    let next = if app.results.no_next {
        None
    } else {
        // `saturating_add` as a defence: `no_next` should already cover it, but
        // a `level_number` at the maximum would overflow and with overflow-checks that
        // closes the game. We do not make stability depend on a flag.
        Some(app.level_number.saturating_add(1))
    };
    if app.frame.pressed(UiKey::R) {
        start_level(app, app.mode, app.level_number);
    } else if app.frame.pressed(UiKey::M) || app.frame.pressed(UiKey::Escape) {
        open_menu(app);
    } else if confirm_pressed(app) {
        if app.results.no_next {
            app.screen = Screen::ModeComplete;
            app.nav = NavRepeat::default();
        } else if let Some(n) = next {
            start_level(app, app.mode, n);
        }
    }
}

fn update_mode_complete(app: &mut App, dt: f32) {
    let _ = dt;
    if confirm_pressed(app) || app.frame.pressed(UiKey::Escape) {
        open_menu(app);
    }
}

fn update_error(app: &mut App) {
    if confirm_pressed(app) || app.frame.pressed(UiKey::Escape) {
        open_menu(app);
    }
}

// --- Exit dialog ---

fn update_quit_confirm(app: &mut App) {
    if app.frame.pressed(UiKey::Enter) {
        app.quit_requested = true;
    }
    if app.frame.pressed(UiKey::Escape) || app.frame.pressed(UiKey::Q) {
        app.quit_confirm = false;
    }
}

/// HUD rows according to mode and compaction.
pub fn hud_rows_for(app: &App) -> u32 {
    if app.hud_compact {
        return 1;
    }
    match app.mode {
        Mode::Original => 1,
        Mode::Enhanced => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jezzball_core::level::{BallKind, Objective};

    fn sample_level(id: u16, purist: bool) -> LevelSpec {
        LevelSpec {
            id,
            name: "t".into(),
            world: 0,
            kind: jezzball_core::level::LevelKind::Standard,
            arena: jezzball_core::level::ArenaSpec {
                w: 16,
                h: 12,
                shape: jezzball_core::level::ArenaShape::Rect,
                obstacles: vec![],
            },
            balls: vec![jezzball_core::level::BallSpawn {
                x: 8.0,
                y: 6.0,
                vx: 18.0,
                vy: 18.0,
                kind: BallKind::Normal,
                radius_mul: 1.0,
            }],
            target_ratio: 0.9,
            lives: 3,
            wall_speed: 22.0,
            time_limit: None,
            powerups_enabled: !purist,
            objectives: if purist {
                vec![]
            } else {
                vec![Objective::ClearRatio(0.5)]
            },
            purist,
            seed: 1,
        }
    }

    #[test]
    fn parse_levels_ron_basico() {
        let ron = r#"
            // comentario legal
            [
                (
                    id: 1,
                    name: "first",
                    world: 0,
                    kind: Standard,
                    arena: (w: 16, h: 16, shape: Rect, obstacles: []),
                    balls: [(x: 1.0, y: 1.0, vx: 4.0, vy: -4.0, kind: Normal, radius_mul: 1.0)],
                    target_ratio: 0.75,
                    lives: 3,
                    wall_speed: 22.0,
                    time_limit: Some(60.0),
                    powerups_enabled: false,
                    objectives: [ClearRatio(0.75)],
                    purist: true,
                    seed: 42,
                ),
            ]
        "#;
        let lv = parse_levels(ron).unwrap();
        assert_eq!(lv.len(), 1);
        assert_eq!(lv[0].id, 1);
        assert!(lv[0].purist);
        assert_eq!(lv[0].time_limit, Some(60.0));
    }

    #[test]
    fn the_real_assets_parse() {
        // Path relative to the workspace (the assets live outside the crate).
        let base = |name: &str| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets/levels")
                .join(name)
        };
        let original = read_level_list(&base("original.ron")).unwrap();
        assert!(!original.is_empty());
        assert!(original.iter().all(|l| l.purist));
        assert!(original.iter().all(|l| l.world == 0));
        let ids: Vec<u16> = original.iter().map(|l| l.id).collect();
        assert_eq!(ids, (1..=original.len() as u16).collect::<Vec<_>>());

        let enhanced = read_level_list(&base("enhanced.ron")).unwrap();
        // 60 levels from the 6 worlds + the bonus level with the Omarchy logo.
        assert_eq!(enhanced.len(), 61);
        assert!(enhanced.iter().all(|l| !l.purist));
        assert!(enhanced.iter().all(|l| (1..=6).contains(&l.world)));
    }

    /// Sweep over the 70 real assets: no ball spawn must land on
    /// a cell that is not `Open` after building the game.
    #[test]
    fn no_spawn_in_the_real_assets_lands_on_a_non_open_cell() {
        use jezzball_core::grid::Cell;
        use jezzball_core::state::GameState;

        let base = |name: &str| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets/levels")
                .join(name)
        };
        let original = read_level_list(&base("original.ron")).unwrap();
        let enhanced = read_level_list(&base("enhanced.ron")).unwrap();

        let mut checked = 0u32;
        let mut violated: Vec<(u16, u8, String, i64, i64)> = Vec::new();
        for list in [&original, &enhanced] {
            for level in list {
                let s = GameState::new(level.clone());
                for b in &s.balls {
                    checked += 1;
                    let x = b.pos.x.floor() as i64;
                    let y = b.pos.y.floor() as i64;
                    if !s.arena.grid.in_bounds(x, y)
                        || s.arena.grid.get(x as u16, y as u16) != Cell::Open
                    {
                        violated.push((level.id, level.world, level.name.clone(), x, y));
                    }
                }
            }
        }
        assert!(
            violated.is_empty(),
            "{} of {} balls spawn on non-`Open` cells (level, world, name, cell): {violated:?}",
            violated.len(),
            checked
        );
        assert!(checked > 0, "the sweep must check at least one ball");
    }

    #[test]
    fn original_unlocking_is_strict() {
        let mut app = App::new();
        app.levels.original = (1..=10).map(|i| sample_level(i, true)).collect();
        app.levels.enhanced.clear();
        app.save = SaveData::default();
        assert!(is_unlocked(&app, Mode::Original, 0));
        assert!(!is_unlocked(&app, Mode::Original, 5));
        // I complete level 5 (id 5): it unlocks number 6.
        app.save.record_for(Mode::Original, 5).completed = true;
        assert!(is_unlocked(&app, Mode::Original, 5));
        assert!(!is_unlocked(&app, Mode::Original, 6));
    }

    #[test]
    fn enhanced_honours_the_original_gate() {
        let mut app = App::new();
        app.levels.original = (1..=10).map(|i| sample_level(i, true)).collect();
        app.levels.enhanced = (1..=2).map(|i| sample_level(i, false)).collect();
        app.levels.enhanced[0].world = 2;
        app.levels.enhanced[1].world = 2;
        app.save = SaveData::default();

        // Entry gate: without Original completed, NOTHING of Enhanced can be
        // played, not even the first level.
        assert!(
            !is_unlocked(&app, Mode::Enhanced, 0),
            "Enhanced must not open before Original is completed"
        );

        // Completing the whole of Original opens Enhanced.
        for i in 1..=10 {
            app.save.record_for(Mode::Original, i).completed = true;
        }
        app.save.original_completed = true;
        assert!(is_unlocked(&app, Mode::Enhanced, 0));
        assert!(is_unlocked(&app, Mode::Enhanced, 1));
    }

    #[test]
    fn enhanced_is_locked_in_the_menu_until_original_is_finished() {
        let mut app = App::new();
        app.levels.original = (1..=10).map(|i| sample_level(i, true)).collect();
        app.levels.enhanced = (1..=5).map(|i| sample_level(i, false)).collect();
        app.save = SaveData::default();

        open_menu(&mut app);
        let enh = &app.menu_items[1];
        assert!(!enh.enabled, "the Enhanced entry must be locked");
        assert!(
            enh.label.contains("ORIGINAL"),
            "the label must explain how to unlock it, not just say LOCKED: {:?}",
            enh.label
        );

        app.save.original_completed = true;
        open_menu(&mut app);
        assert!(
            app.menu_items[1].enabled,
            "after Original, Enhanced opens up"
        );
    }

    #[test]
    fn the_list_window_follows_the_selection() {
        // Regression: with 61 levels the whole list was drawn from the
        // first one, so when selecting a low one, the player kept seeing
        // the beginning of the list and did not know what was chosen.
        let mut app = App::new();
        app.menu_items = (0..61)
            .map(|i| MenuItem {
                label: format!("LEVEL {i}"),
                enabled: true,
            })
            .collect();
        app.list_top = 0;

        app.visible_rows = visible_rows_for(1000.0, 1.4);
        let visible = app.visible_rows;

        // Selection inside the first window: no need to scroll.
        app.select = 0;
        adjust_scroll_window(&mut app);
        assert_eq!(app.list_top, 0);

        // Selection far below: the window must reach it.
        app.select = 55;
        adjust_scroll_window(&mut app);
        assert!(
            app.select >= app.list_top && app.select < app.list_top + visible,
            "selection {} fell outside the window [{}, {})",
            app.select,
            app.list_top,
            app.list_top + visible
        );

        // And when going back up, inside again.
        app.select = 2;
        adjust_scroll_window(&mut app);
        assert!(
            app.select >= app.list_top && app.select < app.list_top + visible,
            "when scrolling up, the selection fell outside the window again"
        );
    }

    #[test]
    fn every_help_page_has_content() {
        for n in 0..HELP_PAGES {
            let (title, lines) = help_page(n);
            assert!(!title.is_empty(), "la pagina {n} no tiene title");
            assert!(
                lines.iter().any(|l| !l.trim().is_empty()),
                "la pagina {n} no tiene contents"
            );
        }
    }

    #[test]
    fn candidate_order_matches_the_six_documented_paths() {
        // The resolution order must be exactly the documented one: first
        // the environment override, then the ones relative to the binary, then XDG,
        // then /usr/share, and lastly the CWD.
        let dirs = level_candidates(
            Some(Path::new("/opt/jezzball/bin")),
            Path::new("/home/user/.local/share"),
            Some(Path::new("/tmp/mis-assets")),
        );
        let paths: Vec<String> = dirs
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert_eq!(dirs.len(), 6, "seis candidatos: {paths:?}");
        assert_eq!(paths[0], "/tmp/mis-assets/levels");
        assert_eq!(paths[1], "/opt/jezzball/bin/assets/levels");
        assert_eq!(paths[2], "/opt/jezzball/share/omarchy-jezzball/levels");
        assert_eq!(paths[3], "/home/user/.local/share/omarchy-jezzball/levels");
        assert_eq!(paths[4], "/usr/share/omarchy-jezzball/levels");
        assert_eq!(paths[5], "assets/levels");
    }

    #[test]
    fn local_install_sh_layout_converges_and_collapses() {
        // install.sh leaves the binary in ~/.local/bin and the levels in
        // ~/.local/share/omarchy-jezzball/levels. Candidate 3 (relative to the
        // exe, `bin/../share`) and candidate 4 (XDG_DATA_HOME) coincide after collapsing
        // `..`, so they are deduplicated: the diagnostics do not repeat the path.
        let dirs = level_candidates(
            Some(Path::new("/home/user/.local/bin")),
            Path::new("/home/user/.local/share"),
            None,
        );
        let paths: Vec<String> = dirs
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            paths,
            vec![
                "/home/user/.local/bin/assets/levels",
                "/home/user/.local/share/omarchy-jezzball/levels",
                "/usr/share/omarchy-jezzball/levels",
                "assets/levels",
            ]
        );
    }

    #[test]
    fn no_candidate_contains_the_wrong_assets_segment() {
        // Regression for bug 2: the app must NEVER look in
        // `<...>/omarchy-jezzball/assets/levels`. Sweep of configurations
        // (with and without override, exe in binary dirs, pacman, etc.).
        let cases: [(&Path, &Path, Option<&Path>); 4] = [
            (
                Path::new("/opt/jezzball/bin"),
                Path::new("/home/u/.local/share"),
                Some(Path::new("/tmp/mis-assets")),
            ),
            (
                Path::new("/home/u/.local/bin"),
                Path::new("/home/u/.local/share"),
                None,
            ),
            (
                Path::new("/usr/bin"),
                Path::new("/var/empty/x"),
                Some(Path::new("/opt/assets")),
            ),
            (Path::new("/bin"), Path::new("/srv/share"), None),
        ];
        let mut checked = 0;
        for (exe, data, env) in cases {
            for d in level_candidates(Some(exe), data, env) {
                let s = d.to_string_lossy();
                assert!(
                    !s.contains("omarchy-jezzball/assets/levels"),
                    "candidate with the wrong path from bug 2: {s}"
                );
                checked += 1;
            }
        }
        assert!(checked >= 5, "el barrido debe revisar varios candidatos");
    }

    #[test]
    fn real_assets_via_simulated_exe_dir_pointing_at_the_repo() {
        // Candidate 2 (`<exe dir>/assets/levels`) with the exe "living" at the
        // root of the repo: it must load the 70 real levels without touching the OS.
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dirs = level_candidates(Some(&repo), Path::new("/var/empty/nonexistent"), None);
        let levels = load_levels_in(&dirs);
        assert_eq!(levels.original.len(), 10);
        assert_eq!(levels.enhanced.len(), 61);
    }

    #[test]
    fn real_assets_via_simulated_env_var() {
        // `$OMARCHY_JEZZBALL_ASSETS` = the repo's `assets/` folder: the
        // candidate 1 (development/test override) must find it.
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let dirs = level_candidates(
            Some(Path::new("/bin")),
            Path::new("/var/empty/x"),
            Some(&assets),
        );
        assert_eq!(
            dirs[0],
            normalize_path(&assets.join("levels")),
            "the override opens the list"
        );
        let levels = load_levels_in(&dirs);
        assert_eq!(levels.original.len(), 10);
        assert_eq!(levels.enhanced.len(), 61);
    }

    #[test]
    fn with_no_path_at_all_loading_returns_empty_without_panicking() {
        // With exe and data_home in non-existent places and with no override, NO
        // candidate hits: the load returns empty lists (which the UI shows
        // as "no levels available"), never a panic.
        let dirs = level_candidates(
            Some(Path::new("/srv/game/bin")),
            Path::new("/srv/no-existe"),
            None,
        );
        let levels = load_levels_in(&dirs);
        assert!(levels.original.is_empty());
        assert!(levels.enhanced.is_empty());
    }

    /// Fuzz of the unlocking logic and level startup with hostile
    /// indices. In debug, a subtraction on a `u16` that goes below zero PANICS, so
    /// this test covers the class of failure that would close the game when navigating
    /// the selector (which is how it runs with `cargo run`).
    #[test]
    fn fuzz_hostile_indices_does_not_panic() {
        let mut app = App::new();
        app.levels.original = (1..=10).map(|i| sample_level(i, true)).collect();
        app.levels.enhanced = (1..=60).map(|i| sample_level(i, false)).collect();

        // Indices inside, at the edges and far out of range.
        let hostile = [
            0u16,
            1,
            2,
            9,
            10,
            11,
            59,
            60,
            61,
            255,
            256,
            1000,
            u16::MAX - 1,
            u16::MAX,
        ];
        for mode in [Mode::Original, Mode::Enhanced] {
            for &idx in &hostile {
                // None of these must panic, whatever happens.
                let _ = is_unlocked(&app, mode, idx);
                let _ = prev_completed(&app, mode, idx);
                let _ = app.levels.get(mode, idx);
                start_level(&mut app, mode, idx);
            }
        }

        // Also with the empty lists (the "there are no levels" case).
        let mut vacio = App::new();
        for mode in [Mode::Original, Mode::Enhanced] {
            for &idx in &hostile {
                let _ = is_unlocked(&vacio, mode, idx);
                let _ = prev_completed(&vacio, mode, idx);
                start_level(&mut vacio, mode, idx);
            }
            start_mode(&mut vacio, mode);
        }
    }

    /// `launch` from the CLI with an arbitrary `--level`: it must never panic,
    /// even if the user asks for a level that does not exist or for 0.
    #[test]
    fn fuzz_cli_launch_does_not_panic() {
        for lvl in [
            None,
            Some(0u16),
            Some(1),
            Some(10),
            Some(11),
            Some(60),
            Some(61),
            Some(u16::MAX),
        ] {
            for mode in [None, Some(Mode::Original), Some(Mode::Enhanced)] {
                let mut app = App::new();
                app.levels.original = (1..=10).map(|i| sample_level(i, true)).collect();
                app.levels.enhanced = (1..=60).map(|i| sample_level(i, false)).collect();
                launch(&mut app, mode, lvl);
            }
        }
    }

    /// Fuzz of the PROGRESSION: completing levels one after another, which is the path
    /// actually walked by whoever plays. It covers `finish_level` (records,
    /// stars, unlocking of the next world, end of mode) with extreme
    /// game states, looking for overflows and invalid indices.
    #[test]
    fn fuzz_progression_completing_levels_does_not_panic() {
        for mode in [Mode::Original, Mode::Enhanced] {
            let mut app = App::new();
            app.levels.original = (1..=10).map(|i| sample_level(i, true)).collect();
            app.levels.enhanced = (1..=60).map(|i| sample_level(i, false)).collect();
            app.mode = mode;

            let total = app.levels.len(mode);
            // We go through ALL the levels of the mode, winning and losing
            // alternately, including the last one (end of mode).
            for idx in 0..total {
                start_level(&mut app, mode, idx);
                app.level_number = idx;
                // Hostile game state: score and stars at maximum.
                app.state.score = u32::MAX;
                app.state.stars = 3;
                app.state.elapsed = f32::MAX;
                app.state.phase = if idx % 2 == 0 {
                    GamePhase::Won
                } else {
                    GamePhase::Lost
                };
                finish_level(&mut app);
            }

            // And once more past the end: `level_number` out of range.
            app.level_number = total;
            app.state.phase = GamePhase::Won;
            finish_level(&mut app);

            app.level_number = u16::MAX;
            app.state.phase = GamePhase::Won;
            finish_level(&mut app);
        }
    }
}
