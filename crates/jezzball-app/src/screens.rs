//! Máquina de estados de pantallas y lógica de APP (ARCHITECTURE.md §6 y §13).
//!
//! Reglas: la lógica de partida vive en `jezzball-core`; aquí solo se orquesta
//! `step()` con la entrada del jugador, se actualizan récords/persistencia y se
//! deciden las transiciones entre pantallas. Sin `unsafe`, sin panics.

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

/// Segundos mínimos que se muestra la pantalla de resultados antes de aceptar
/// entrada (evita avanzar por un clic suelto del nivel anterior).
const RESULTS_WAIT: f32 = 0.4;

/// Pantallas de la máquina de estados.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Screen {
    Menu,
    ModeMenu,
    Selector,
    Playing,
    Results,
    ModeComplete,
    Error,
}

/// Una opción de menú de la lista activa (`menu_items`).
#[derive(Clone, Debug)]
pub struct MenuItem {
    pub label: String,
    pub enabled: bool,
}

/// Tipo de texto flotante (corta el evento después de los hechos).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FloaterKind {
    Good,
    Bad,
    Combo,
    Neutral,
}

/// Texto flotante animado (puntos, combos, avisos).
#[derive(Clone, Debug)]
pub struct Floater {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub ttl: f32,
    pub kind: FloaterKind,
}

/// Aviso breve en el centro inferior.
#[derive(Clone, Debug)]
pub struct Toast {
    pub text: String,
    pub ttl: f32,
}

/// Datos de la pantalla de resultados.
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

/// Niveles cargados por modo, ordenados por `id`.
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

/// Estado global de la capa de app.
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
    /// Rutas concretas probadas al cargar los niveles (diagnóstico: se
    /// muestran en la pantalla de error si no se encuentran).
    pub candidates: Vec<PathBuf>,
    pub hud_compact: bool,
    pub quit_confirm: bool,
    pub quit_requested: bool,
    /// Cuenta atrás de protección de la pantalla de resultados.
    pub results_wait: f32,
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
        };
        open_menu(&mut app);
        app
    }
}

/// Un `LevelSpec` mínimo de emergencia para inicializar `App` (nunca se
/// juega: todas las pantallas cargan los niveles de disco antes de jugar).
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

// --- Carga de niveles ---

/// Variable de entorno de escape para desarrollo y tests: apunta a la carpeta
/// `assets/` del repo, que contiene `levels/`. Ver `level_candidates`.
const ENV_ASSETS: &str = "OMARCHY_JEZZBALL_ASSETS";

/// Niveles cargados + rutas candidatas probadas (para el diagnóstico).
fn load_levels() -> (Levels, Vec<PathBuf>) {
    let dirs = level_candidates(
        current_exe_dir().as_deref(),
        &crate::persist::data_home_dir(),
        std::env::var_os(ENV_ASSETS).map(PathBuf::from).as_deref(),
    );
    let levels = load_levels_in(&dirs);
    (levels, dirs)
}

/// Directorio del binario activo (`std::env::current_exe()`), o `None` si el
/// SO no lo puede resolver (nunca panic).
fn current_exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

/// Directorios candidatos donde puede haber `original.ron` / `enhanced.ron`,
/// en orden de prioridad. Función PURA (no toca el SO) para poder testear el
/// orden sin efectos de entorno. Orden documentado en README:
/// 1. `$OMARCHY_JEZZBALL_ASSETS/levels`         (escape desarrollo/tests)
/// 2. `<dir exe>/assets/levels`                 (binario junto a assets)
/// 3. `<dir exe>/../share/omarchy-jezzball/levels` (install.sh: ~/.local)
/// 4. `$XDG_DATA_HOME/omarchy-jezzball/levels`  (fallback ~/.local/share)
/// 5. `/usr/share/omarchy-jezzball/levels`      (PKGBUILD / pacman)
/// 6. `./assets/levels`                         (CWD: `cargo run` en el repo)
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
    // Con ~/.local (install.sh) los candidatos 3 y 4 convergen en la misma
    // ruta: se coleapsa para que el diagnóstico no repita directorios.
    out.dedup();
    out
}

/// Colapsa `.` y `..` redundantes de una ruta (puro, nunca hace I/O).
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
    let list: Vec<LevelSpec> = ron::from_str(text).map_err(|e| format!("RON invalido: {e}"))?;
    Ok(list.into_iter().filter(|l| l.id >= 1).collect())
}

// --- Navegación / menús ---

fn open_menu(app: &mut App) {
    app.screen = Screen::Menu;
    app.menu_items = vec![
        MenuItem {
            label: "MODO ORIGINAL".into(),
            enabled: !app.levels.original.is_empty(),
        },
        MenuItem {
            label: "MODO ENHANCED".into(),
            enabled: !app.levels.enhanced.is_empty(),
        },
        MenuItem {
            label: "ELEGIR NIVEL".into(),
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
            label: "MODO ORIGINAL".into(),
            enabled: !app.levels.original.is_empty(),
        },
        MenuItem {
            label: "MODO ENHANCED".into(),
            enabled: !app.levels.enhanced.is_empty(),
        },
    ];
    app.select = 0;
    app.nav = NavRepeat::default();
}

fn open_selector(app: &mut App, mode: Mode) {
    app.selector_mode = mode;
    app.screen = Screen::Selector;
    let mut items = vec![MenuItem {
        label: "EMPIEZA EN NIVEL 1".into(),
        enabled: true,
    }];
    let list = app.levels.list(mode);
    for (i, spec) in list.iter().enumerate() {
        items.push(MenuItem {
            label: format!("NIVEL {}  -  {}", i + 1, spec.name),
            enabled: is_unlocked(app, mode, i as u16),
        });
    }
    app.menu_items = items;
    app.select = 0;
    app.nav = NavRepeat::default();
}

/// Navegación vertical típica sobre `menu_items` (salta opciones deshabilitadas
/// por teclas dobles).
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
        }
    }
}

/// Progreso persistido por modo, para el menú.
pub fn mode_progress(app: &App, mode: Mode) -> (usize, u32) {
    let total = app.levels.len(mode);
    let done = app.save.completed_count(mode).min(total as usize);
    let stars = app.save.total_stars(mode);
    (done, stars)
}

/// ¿Este nivel está desbloqueado para jugar? (progreso parcial en Original →
/// desbloquea Enhancement como modalidad de rejugar enteras a angelical)
fn is_unlocked(app: &App, mode: Mode, idx: u16) -> bool {
    if idx == 0 {
        return true;
    }
    if app.levels.list(mode).is_empty() {
        return false;
    }
    // En Original (purist, 10 niveles) la progresión es estricta: nivel previo
    // completado, salvo que el modo ya se haya terminado (rejugar libre).
    if mode == Mode::Original {
        if original_mode_completed(app) {
            return true;
        }
        return prev_completed(app, mode, idx);
    }
    // Enhanced: además hay una puerta temática: los mundos 2..=6 exigen un
    // avance mínimo en Original (§7: original nivel n => enhanced mundo w,
    // con n >= 2(w-1)). El mundo entero se desbloquea a la vez (se puede
    // jugar cualquier nivel de ese mundo, sin secuencia interna).
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
    let Some(prev_spec) = app.levels.get(mode, idx - 1) else {
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

// --- Inicio de partidas ---

/// Mensaje de error cuando un modo no tiene niveles: además del aviso, lista
/// las rutas concretas que se probaron, una por línea, para que el usuario
/// sepa dónde colocar `original.ron` / `enhanced.ron`.
fn no_levels_message(mode: Mode, candidates: &[PathBuf]) -> String {
    let paths: Vec<String> = candidates
        .iter()
        .map(|c| format!("  {}", c.display()))
        .collect();
    let mut msg = format!("{} no tiene niveles disponibles.", mode_name(mode));
    if !paths.is_empty() {
        msg.push_str("\nSe buscaron en:\n");
        msg.push_str(&paths.join("\n"));
        msg.push_str("\n\nColoca original.ron y enhanced.ron en una de esas rutas.");
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
    // Modos ya completados: abren el selector para rejugar niveles.
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
        app.error_msg = format!("El nivel {} no existe", idx + 1);
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

/// Arranque inicial desde la CLI: `--mode` y/o `--level` (1-based).
pub fn launch(app: &mut App, mode: Option<Mode>, level: Option<u16>) {
    let mode = mode.unwrap_or(Mode::Original);
    if let Some(n) = level {
        if n >= 1 {
            let idx = n - 1;
            if app.levels.get(mode, idx).is_some() {
                start_level(app, mode, idx);
                return;
            }
            app.error_msg = format!("El nivel {} no existe en {}", n, mode_name(mode));
            app.screen = Screen::Error;
            app.nav = NavRepeat::default();
            return;
        }
    }
    start_mode(app, mode);
}

// --- Bucle de actualización ---

pub fn update(app: &mut App, dt: f32) {
    app.frame = crate::input::sample_frame();
    app.mouse_pos = (app.frame.mouse.x, app.frame.mouse.y);

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
        Screen::Error => update_error(app),
    }

    // Animaciones independientes de la pantalla.
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
        _ => open_mode_menu(app),
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
        let idx = spec.id - 1;
        start_level(app, mode, idx);
    }
}

/// Enter, Espacio o la tecla del controlador (en menús de teclado).
fn confirm_pressed(app: &App) -> bool {
    app.frame
        .pressed_any(&[UiKey::Enter, UiKey::Space, UiKey::M])
}

// --- Pantalla de juego ---

fn update_playing(app: &mut App, dt: f32) {
    match app.state.phase {
        GamePhase::Paused => {
            if app.frame.pressed(UiKey::Space) || app.frame.pressed(UiKey::Escape) {
                let (ns, _) = step(&app.state, PlayerInput::Resume, 0.0);
                app.state = ns;
            } else if app.frame.pressed(UiKey::R) {
                let (ns, _) = step(&app.state, PlayerInput::Restart, 0.0);
                app.state = ns;
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
            // Con el muro en construcción el ratón no puede disparar otro.
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
                add_floater(app, FloaterKind::Bad, "BLOQUEADO".into());
            }
            GameEvent::LifeLost => add_floater(app, FloaterKind::Bad, "VIDA PERDIDA".into()),
            GameEvent::BallLost => add_floater(app, FloaterKind::Neutral, "BOLA PERDIDA".into()),
            GameEvent::ComboUp(m) if m > 1 => {
                add_floater(app, FloaterKind::Combo, format!("COMBO x{}", m))
            }
            GameEvent::ComboReset | GameEvent::ComboUp(_) => {}
            GameEvent::PowerUpSpawned => {
                app.toast = Some(Toast {
                    text: "Power-up disponible".into(),
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

// --- Fin de nivel / resultados ---

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

    // Persistencia inmediata (degradación silenciosa si el disco falla).
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
        Some(app.level_number + 1)
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

// --- Diálogo de salida ---

fn update_quit_confirm(app: &mut App) {
    if app.frame.pressed(UiKey::Enter) {
        app.quit_requested = true;
    }
    if app.frame.pressed(UiKey::Escape) || app.frame.pressed(UiKey::Q) {
        app.quit_confirm = false;
    }
}

/// Filas del HUD según modo y compactación.
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
                    name: "primero",
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
    fn los_assets_reales_se_parsean() {
        // Ruta relativa al workspace (los assets viven fuera del crate).
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
        assert_eq!(enhanced.len(), 60);
        assert!(enhanced.iter().all(|l| !l.purist));
        assert!(enhanced.iter().all(|l| (1..=6).contains(&l.world)));
    }

    /// Barrido sobre los 70 assets reales: ningún spawn de bola debe caer en
    /// una celda que no sea `Open` tras construir la partida.
    #[test]
    fn ningun_spawn_de_los_assets_reales_cae_en_celda_no_abierta() {
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
            "{} bolas de {} nacen en celdas no `Open` (nivel, mundo, nombre, celda): {violated:?}",
            violated.len(),
            checked
        );
        assert!(checked > 0, "el barrido debe revisar al menos una bola");
    }

    #[test]
    fn desbloqueo_original_estricto() {
        let mut app = App::new();
        app.levels.original = (1..=10).map(|i| sample_level(i, true)).collect();
        app.levels.enhanced.clear();
        app.save = SaveData::default();
        assert!(is_unlocked(&app, Mode::Original, 0));
        assert!(!is_unlocked(&app, Mode::Original, 5));
        // Completo el nivel 5 (id 5): desbloquea el 6.
        app.save.record_for(Mode::Original, 5).completed = true;
        assert!(is_unlocked(&app, Mode::Original, 5));
        assert!(!is_unlocked(&app, Mode::Original, 6));
    }

    #[test]
    fn enhanced_reconoce_puerta_original() {
        let mut app = App::new();
        app.levels.original = (1..=10).map(|i| sample_level(i, true)).collect();
        app.levels.enhanced = (1..=2).map(|i| sample_level(i, false)).collect();
        app.levels.enhanced[0].world = 2;
        app.levels.enhanced[1].world = 2;
        app.save = SaveData::default();
        // Nivel 1 del mundo 2: siempre desbloqueado (inicio).
        assert!(is_unlocked(&app, Mode::Enhanced, 0));
        // El resto del mundo 2 pide que el original 2 esté completado.
        assert!(!is_unlocked(&app, Mode::Enhanced, 1));
        app.save.record_for(Mode::Original, 2).completed = true;
        assert!(is_unlocked(&app, Mode::Enhanced, 1));
    }

    #[test]
    fn orden_de_candidatos_seis_rutas_documentadas() {
        // El orden de resolución debe ser exactamente el documentado: primero
        // la escotilla de entorno, luego las relativas al binario, luego XDG,
        // luego /usr/share, y por último el CWD.
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
    fn instalacion_local_de_install_sh_converge_y_colapsa() {
        // install.sh deja el binario en ~/.local/bin y los niveles en
        // ~/.local/share/omarchy-jezzball/levels. El candidato 3 (relativo al
        // exe, `bin/../share`) y el 4 (XDG_DATA_HOME) coinciden tras colapsar
        // `..`, así que se deduplican: el diagnóstico no repite la ruta.
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
    fn ningun_candidato_contiene_el_segmento_assets_equivocado() {
        // Regresión del bug 2: la app NUNCA debe buscar en
        // `<...>/omarchy-jezzball/assets/levels`. Barrido de configuraciones
        // (con y sin override, exe en binarios, pacman, etc.).
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
                    "candidato con la ruta equivocada del bug 2: {s}"
                );
                checked += 1;
            }
        }
        assert!(checked >= 5, "el barrido debe revisar varios candidatos");
    }

    #[test]
    fn assets_reales_via_exe_dir_simulado_apuntando_al_repo() {
        // Candidato 2 (`<dir exe>/assets/levels`) con el exe "vivido" en la
        // raíz del repo: debe cargar los 70 niveles reales sin tocar el SO.
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dirs = level_candidates(Some(&repo), Path::new("/var/empty/nonexistent"), None);
        let levels = load_levels_in(&dirs);
        assert_eq!(levels.original.len(), 10);
        assert_eq!(levels.enhanced.len(), 60);
    }

    #[test]
    fn assets_reales_via_variable_de_entorno_simulada() {
        // `$OMARCHY_JEZZBALL_ASSETS` = carpeta `assets/` del repo: el
        // candidato 1 (escotilla de desarrollo/tests) debe encontrarla.
        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let dirs = level_candidates(
            Some(Path::new("/bin")),
            Path::new("/var/empty/x"),
            Some(&assets),
        );
        assert_eq!(
            dirs[0],
            normalize_path(&assets.join("levels")),
            "el override abre la lista"
        );
        let levels = load_levels_in(&dirs);
        assert_eq!(levels.original.len(), 10);
        assert_eq!(levels.enhanced.len(), 60);
    }

    #[test]
    fn sin_ninguna_ruta_la_carga_devuelve_vacio_sin_panic() {
        // Con exe y data_home en sitios inexistentes y sin override, NINGÚN
        // candidato acierta: la carga devuelve listas vacías (que la UI muestra
        // como "no tiene niveles disponibles"), nunca panic.
        let dirs = level_candidates(
            Some(Path::new("/srv/juego/bin")),
            Path::new("/srv/no-existe"),
            None,
        );
        let levels = load_levels_in(&dirs);
        assert!(levels.original.is_empty());
        assert!(levels.enhanced.is_empty());
    }
}
