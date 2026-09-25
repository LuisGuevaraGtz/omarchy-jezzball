//! `GameState` + `step()`: el corazón de la lógica pura (ARCHITECTURE.md §6).
//!
//! `step` es PURA: no muta el estado recibido, clona y devuelve uno nuevo.
//! Es determinista: mismo `state` + mismo `input` + mismo `dt` => mismo
//! resultado (no hay `Instant::now()` ni fuentes externas de aleatoriedad).
//!
//! Orden de orquestación impuesto por el contrato (comentado en cada paso):
//!   a) aplicar input del jugador
//!   b) power-ups temporizados sobre dt (SlowMotion scale, Freeze anula)
//!   c) integrar bolas con substeps (rebote por ejes separados)
//!   d) avanzar los frentes de los builders
//!   e) impacto bola <-> muro en construcción (escudo o vida)
//!   f) consolidar muros completados
//!   g) flood fill 4-conexo + cierre de regiones + puntuación (sección 5)
//!   h) ventana de combo (4 s, tope x8, reset al perder vida)
//!   i) victoria / derrota / tiempo / objetivos y estrellas
//!   j) emitir `Vec<GameEvent>`

use std::collections::VecDeque;

use crate::arena::Arena;
use crate::ball::Ball;
use crate::geom::Vec2;
use crate::grid::{Cell, Grid};
use crate::level::{BallKind, LevelSpec, Objective};
use crate::powerup::{all_kinds, ActivePowerUp, PowerUp, PowerUpKind, INVENTORY_MAX, PICKUP_MAX};
use crate::rng::Rng64;
use crate::score::{region_points, Combo};
use crate::wall::{Wall, WallAxis, WallBuilder};

/// Fase de ejecución de la partida.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GamePhase {
    Running,
    Paused,
    Won,
    Lost,
}

/// Entrada del jugador: única superficie de mutación (ARCHITECTURE.md §6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerInput {
    None,
    StartWall {
        cell: (u16, u16),
        axis: WallAxis,
    },
    /// Alterna el eje por defecto del próximo muro (lo lee la capa de app).
    ToggleAxis,
    UsePowerUp(PowerUpKind),
    Pause,
    Resume,
    Restart,
}

/// Qué ha pasado, para que el render/audio reaccione (ARCHITECTURE.md §6).
#[derive(Clone, Debug, PartialEq)]
pub enum GameEvent {
    WallStarted,
    WallBlocked,
    WallCompleted { points: u32, cells: u32 },
    BallLost,
    LifeLost,
    ComboUp(u8),
    ComboReset,
    PowerUpSpawned,
    LevelCleared { stars: u8 },
    GameOver,
}

/// Estado de un objetivo secundario (estrellas).
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectiveProgress {
    pub objective: Objective,
    pub done: bool,
}

/// Estado completo de una partida. Deriva `Clone, Debug, PartialEq`
/// (obligatorio por contrato) y todos sus campos son públicos.
#[derive(Clone, Debug, PartialEq)]
pub struct GameState {
    pub level: LevelSpec,
    pub arena: Arena,
    pub balls: Vec<Ball>,
    pub builders: Vec<WallBuilder>,
    pub phase: GamePhase,
    /// Segundos transcurridos con la partida en marcha (Running).
    pub elapsed: f32,
    pub score: u32,
    pub combo: Combo,
    /// Máximo multiplicador alcanzado (objetivo `KeepCombo`).
    pub max_combo_reached: u8,
    pub lives: u8,
    pub powerups_active: Vec<ActivePowerUp>,
    pub pickups: Vec<PowerUp>,
    pub inventory: Vec<PowerUpKind>,
    pub pending_shield: bool,
    pub pending_double_wall: bool,
    /// Nº de power-ups usados (objetivo `NoPowerUps`).
    pub powerups_used: u32,
    /// Eje por defecto (lo alterna `ToggleAxis`).
    pub current_axis: WallAxis,
    pub objectives: Vec<ObjectiveProgress>,
    pub stars: u8,
    /// Cuenta atrás hasta el próximo spawn de power-up.
    pub next_powerup_in: f32,
    pub rng: Rng64,
}

impl GameState {
    /// Construye una partida nueva a partir de un `LevelSpec`.
    pub fn new(level: LevelSpec) -> Self {
        let arena = Arena::from_spec(&level.arena, level.seed);
        let balls = spawn_balls(&level.balls, &arena.grid);
        let lives = level.lives;
        let objectives = level
            .objectives
            .iter()
            .map(|o| ObjectiveProgress {
                objective: *o,
                done: false,
            })
            .collect();
        let mut rng = Rng64::new(level.seed);
        // Primer spawn de power-up: 30 ± 10 s, determinista.
        let next_powerup_in = 20.0 + rng.range_f32(0.0, 20.0);
        GameState {
            level,
            arena,
            balls,
            builders: Vec::new(),
            phase: GamePhase::Running,
            elapsed: 0.0,
            score: 0,
            combo: Combo::new(),
            max_combo_reached: 1,
            lives,
            powerups_active: Vec::new(),
            pickups: Vec::new(),
            inventory: Vec::new(),
            pending_shield: false,
            pending_double_wall: false,
            powerups_used: 0,
            current_axis: WallAxis::Horizontal,
            objectives,
            stars: 0,
            next_powerup_in,
            rng,
        }
    }
}

/// La función central del proyecto (ARCHITECTURE.md §6).
pub fn step(state: &GameState, input: PlayerInput, dt: f32) -> (GameState, Vec<GameEvent>) {
    // Clonamos para no mutar el estado recibido (firma pura).
    let mut s = state.clone();
    let mut events = Vec::new();

    // Partida terminada: solo se puede reiniciar.
    if s.phase == GamePhase::Won || s.phase == GamePhase::Lost {
        if input == PlayerInput::Restart {
            return (GameState::new(s.level.clone()), events);
        }
        return (s, events);
    }

    // Pausa: el reloj se detiene por completo.
    if s.phase == GamePhase::Paused {
        match input {
            PlayerInput::Resume => s.phase = GamePhase::Running,
            PlayerInput::Restart => return (GameState::new(s.level.clone()), events),
            _ => {}
        }
        return (s, events);
    }

    // Corriendo:
    match input {
        PlayerInput::Pause => {
            s.phase = GamePhase::Paused;
            return (s, events);
        }
        PlayerInput::Restart => return (GameState::new(s.level.clone()), events),
        other => apply_input(&mut s, other, &mut events),
    }

    // Movers (obstáculos móviles): actualizamos su ocupación ANTES de
    // integrar bolas para que los rebotes vean la rejilla nueva. Destruyen
    // muros en construcción SIN coste de vidas (decisión documentada; las
    // vidas solo se pierden por impacto de bola, ARCHITECTURE.md §3).
    let mover_cells = s.arena.update_movers(dt);
    destroy_builders_from_cells(&mut s, &mover_cells, &mut events);

    // b) Power-ups temporizados: aquí decidimos el `dt` efectivo de las bolas.
    //    SlowMotion lo scale a ×0.5; Freeze lo anula. Los frentes de muro
    //    usan siempre el `dt` real.
    let ball_dt = advance_timed_powerups(&mut s, dt);

    // c) Integrar bolas con substeps si |vel| * dt >= 0.5 (rebote por ejes
    //    separados contra la rejilla => rebote de 45° clásico).
    for ball in &mut s.balls {
        ball.step(&s.arena.grid, &mut s.rng, ball_dt);
    }

    // Recoger power-ups del suelo si una bola pisa su celda.
    collect_pickups(&mut s);

    // d) Avanzar los frentes de los builders y spawnear power-ups.
    for builder in &mut s.builders {
        builder.advance(&s.arena.grid, dt);
    }
    // e) Impacto bola <-> muro en construcción: escudo absorbe 1 impacto,
    //    Heavy lo ignora; si no hay escudo => muro destruido + vida perdida.
    //    Va ANTES del sellado: una bola que está tocando el frente debe
    //    romperlo, no quedar encerrada por él.
    wall_impacts(&mut s, &mut events);

    // Sellado por mitades (regla del JezzBall original): en cuanto un frente
    // alcanza su límite, ESA mitad se vuelca a la rejilla como `Filled` y
    // pasa a ser inmune. La otra mitad sigue creciendo y sigue siendo la
    // única que puede costar una vida.
    seal_finished_halves(&mut s);
    maybe_spawn_pickup(&mut s, dt, &mut events);

    // f+g+h) Consolidar muros, partición (flood fill), puntuación y combo.
    consolidate_walls(&mut s, &mut events);

    // h) Ventana de combo: expira y vuelve a x1.
    if !s.level.purist && s.combo.tick(dt) {
        events.push(GameEvent::ComboReset);
    }

    s.elapsed += dt;

    // i) Victoría / derrota / tiempo agotado / objetivos y estrellas.
    check_end(&mut s, &mut events);

    (s, events)
}

/// Crea las bolas de una partida a partir de sus `BallSpawn`, reubicando
/// cualquier spawn que no caiga en una celda `Open` de la arena ya
/// materializada.
fn spawn_balls(spawns: &[crate::level::BallSpawn], grid: &Grid) -> Vec<Ball> {
    spawns
        .iter()
        .enumerate()
        .filter_map(|(i, spawn)| new_or_relocated_ball(i as u32, spawn, grid))
        .collect()
}

/// Crea una bola desde su `BallSpawn` y garantiza el invariante "toda bola
/// nace en una celda `Open`".
///
/// Por qué hace falta una defensa aquí: `tools/gen_levels.py` genera los
/// niveles `ArenaShape::Maze` con un laberinto recursive-backtracker sembrado
/// con el `random` de Python y coloca los spawns de bola en las celdas
/// abiertas DE SU laberinto. En el motor, `Arena::from_spec` ->
/// `materialize_maze` reproduce el algoritmo pero con OTRO generador de
/// números aleatorios (`Rng64`, xorshift). Dos RNG distintos => dos
/// laberintos distintos => un spawn que el generador creía abierto puede
/// caer inside de un muro `Solid` (o fuera de la arena). En vez de replicar
/// el RNG de Python (que acoplaría el motor a un script externo y seguiría
/// rompiéndose al cambiar cualquiera de los dos), el motor defiende el
/// invariante: si el spawn no cae en una celda `Open`, se busca en anchura
/// desde la celda del spawn la primera celda `Open` que no sea `NoSplit` y
/// ahí se reubica la bola, conservando su velocidad, su `kind` y su radio.
/// Si no existe ninguna celda abierta en toda la arena (nivel degenerado),
/// la bola se descarta en vez de provocar un estado inválido.
fn new_or_relocated_ball(id: u32, spawn: &crate::level::BallSpawn, grid: &Grid) -> Option<Ball> {
    let mut ball = Ball::new(
        id,
        spawn.x,
        spawn.y,
        spawn.vx,
        spawn.vy,
        spawn.kind,
        spawn.radius_mul,
    );
    let sx = spawn.x.floor() as i64;
    let sy = spawn.y.floor() as i64;
    if grid.in_bounds(sx, sy) && grid.is_open(sx as u16, sy as u16) {
        return Some(ball);
    }
    let target = nearest_open_cell(grid, sx, sy)?;
    ball.pos = Vec2::new(target.0 as f32 + 0.5, target.1 as f32 + 0.5);
    ball.prev = ball.pos;
    Some(ball)
}

/// Búsqueda en anchura desde la celda `(sx, sy)`: devuelve la primera celda
/// `Open` que no sea `NoSplit`. `None` si no hay ninguna (arena degenerada).
fn nearest_open_cell(grid: &Grid, sx: i64, sy: i64) -> Option<(u16, u16)> {
    let start = (
        sx.clamp(0, grid.w as i64 - 1) as u16,
        sy.clamp(0, grid.h as i64 - 1) as u16,
    );
    let mut visited = vec![false; grid.w as usize * grid.h as usize];
    let mut queue = VecDeque::from([start]);
    visited[grid.idx(start.0, start.1)] = true;
    while let Some((cx, cy)) = queue.pop_front() {
        if grid.is_open(cx, cy) && !grid.is_no_split(cx, cy) {
            return Some((cx, cy));
        }
        for (dx, dy) in [(1i64, 0), (0, 1), (-1, 0), (0, -1)] {
            let (nx, ny) = (cx as i64 + dx, cy as i64 + dy);
            if grid.in_bounds(nx, ny) {
                let (ux, uy) = (nx as u16, ny as u16);
                let i = grid.idx(ux, uy);
                if !visited[i] {
                    visited[i] = true;
                    queue.push_back((ux, uy));
                }
            }
        }
    }
    None
}

/// a) Aplicar la entrada del jugador (sin Pause/Resume/Restart, ya tratados).
fn apply_input(s: &mut GameState, input: PlayerInput, events: &mut Vec<GameEvent>) {
    match input {
        PlayerInput::None | PlayerInput::Pause | PlayerInput::Resume | PlayerInput::Restart => {}
        PlayerInput::ToggleAxis => {
            s.current_axis = match s.current_axis {
                WallAxis::Horizontal => WallAxis::Vertical,
                WallAxis::Vertical => WallAxis::Horizontal,
            };
        }
        PlayerInput::StartWall { cell, axis } => start_wall(s, cell, axis, events),
        PlayerInput::UsePowerUp(kind) => use_powerup(s, kind, events),
    }
}

/// Inicia un muro (`PlayerInput::StartWall`).
fn start_wall(s: &mut GameState, cell: (u16, u16), axis: WallAxis, events: &mut Vec<GameEvent>) {
    // Fuera de la arena o sobre una celda no abierta -> bloqueado.
    if cell.0 >= s.arena.grid.w || cell.1 >= s.arena.grid.h {
        events.push(GameEvent::WallBlocked);
        return;
    }
    if !s.arena.grid.is_open(cell.0, cell.1) || s.arena.grid.is_no_split(cell.0, cell.1) {
        events.push(GameEvent::WallBlocked);
        return;
    }
    // Un solo builder a la vez, salvo con el power-up Muro Doble (2).
    let max_builders = if s.pending_double_wall { 2 } else { 1 };
    if s.builders.len() >= max_builders {
        // Se ignora silenciosamente (decisión documentada: no se emite evento
        // porque no es un fallo del muro en sí, es un input no válido).
        return;
    }
    let shielded = s.pending_shield;
    s.pending_shield = false;
    if s.pending_double_wall {
        s.pending_double_wall = false;
    }
    s.builders
        .push(WallBuilder::new(axis, cell, s.level.wall_speed, shielded));
    events.push(GameEvent::WallStarted);
}

/// Usa un power-up del inventario si está available (ARCHITECTURE.md §9).
fn use_powerup(s: &mut GameState, kind: PowerUpKind, events: &mut Vec<GameEvent>) {
    // Modo Original: sin power-ups (un solo camino de código con guardas).
    if s.level.purist {
        return;
    }
    let Some(pos) = s.inventory.iter().position(|k| *k == kind) else {
        return;
    };
    s.inventory.remove(pos);

    let used = match kind {
        PowerUpKind::SlowMotion | PowerUpKind::Freeze => {
            s.powerups_active.push(ActivePowerUp {
                kind,
                remaining: ActivePowerUp::duration(kind),
            });
            true
        }
        PowerUpKind::DoubleWall => {
            if s.pending_double_wall {
                s.inventory.push(kind); // ya estaba pendiente: no se consume
                false
            } else {
                s.pending_double_wall = true;
                true
            }
        }
        PowerUpKind::Shield => {
            if s.pending_shield {
                s.inventory.push(kind);
                false
            } else {
                s.pending_shield = true;
                true
            }
        }
        PowerUpKind::RemoveBall => {
            if s.balls.len() > 1 {
                let idx = slowest_ball_index(&s.balls);
                s.balls.remove(idx);
                events.push(GameEvent::BallLost);
                true
            } else {
                s.inventory.push(kind); // no puede quedar la arena sin bolas
                false
            }
        }
    };
    if used {
        s.powerups_used += 1;
    }
}

/// Índice de la bola más lenta (para `RemoveBall`).
fn slowest_ball_index(balls: &[Ball]) -> usize {
    let mut best = 0;
    let mut best_speed = f32::MAX;
    for (i, ball) in balls.iter().enumerate() {
        let sp = ball.speed();
        if sp < best_speed {
            best = i;
            best_speed = sp;
        }
    }
    best
}

/// b) Avanza los power-ups temporizados y devuelve el `dt` efectivo de bolas.
fn advance_timed_powerups(s: &mut GameState, dt: f32) -> f32 {
    let mut ball_dt = dt;
    for active in &mut s.powerups_active {
        active.remaining -= dt;
        match active.kind {
            PowerUpKind::SlowMotion => ball_dt *= 0.5,
            PowerUpKind::Freeze => ball_dt *= 0.0,
            _ => {}
        }
    }
    s.powerups_active.retain(|p| p.remaining > 0.0);
    ball_dt
}

/// Celda en la que está el centro de la bola.
fn ball_cell(ball: &Ball, grid: &Grid) -> Option<(u16, u16)> {
    let x = ball.pos.x.floor() as i64;
    let y = ball.pos.y.floor() as i64;
    if grid.in_bounds(x, y) {
        Some((x as u16, y as u16))
    } else {
        None
    }
}

/// Recoge power-ups del suelo cuando una bola pisa su celda (llenando el
/// inventario hasta `INVENTORY_MAX`).
fn collect_pickups(s: &mut GameState) {
    if s.pickups.is_empty() {
        return;
    }
    let mut remove: Vec<usize> = Vec::new();
    for (pi, pickup) in s.pickups.iter().enumerate() {
        let hit = s
            .balls
            .iter()
            .any(|b| ball_cell(b, &s.arena.grid) == Some(pickup.cell));
        if hit && s.inventory.len() < INVENTORY_MAX {
            remove.push(pi);
            s.inventory.push(pickup.kind);
        }
    }
    for &pi in remove.iter().rev() {
        s.pickups.remove(pi);
    }
}

/// d) Spawnea un power-up recogible cada 30 ± 10 s (máx `PICKUP_MAX` en arena).
fn maybe_spawn_pickup(s: &mut GameState, dt: f32, events: &mut Vec<GameEvent>) {
    s.next_powerup_in -= dt;
    if !s.level.powerups_enabled || s.level.purist {
        return;
    }
    if s.next_powerup_in > 0.0 || s.pickups.len() >= PICKUP_MAX {
        return;
    }
    let Some(cell) = random_open_cell(s) else {
        s.next_powerup_in = 20.0 + s.rng.range_f32(0.0, 20.0);
        return;
    };
    let kinds = all_kinds();
    let kind = kinds[s.rng.range_i64(0, kinds.len() as i64 - 1) as usize];
    s.pickups.push(PowerUp { kind, cell });
    events.push(GameEvent::PowerUpSpawned);
    s.next_powerup_in = 20.0 + s.rng.range_f32(0.0, 20.0);
}

/// Celda aleatoria abierta (y no `NoSplit`) para un spawn de power-up.
fn random_open_cell(s: &mut GameState) -> Option<(u16, u16)> {
    for _ in 0..128 {
        let x = s.rng.range_i64(0, s.arena.grid.w as i64 - 1) as u16;
        let y = s.rng.range_i64(0, s.arena.grid.h as i64 - 1) as u16;
        if s.arena.grid.is_open(x, y) && !s.arena.grid.is_no_split(x, y) {
            return Some((x, y));
        }
    }
    None
}

/// Destruye builders rozados por celdas ocupadas (aquí: por `Mover`).
/// No cuesta vidas.
fn destroy_builders_from_cells(
    s: &mut GameState,
    occupied: &[(u16, u16)],
    events: &mut Vec<GameEvent>,
) {
    if occupied.is_empty() || s.builders.is_empty() {
        return;
    }
    s.builders.retain(|builder| {
        // Igual que con las bolas: un Mover sólo puede romper las mitades que
        // siguen creciendo, no la que ya se selló contra la pared.
        let vulnerable = builder.vulnerable_cells(&s.arena.grid);
        let hit = vulnerable.iter().any(|c| occupied.contains(c));
        if hit {
            events.push(GameEvent::WallBlocked);
        }
        !hit
    });
}

/// Sella las mitades de muro que ya alcanzaron su límite.
///
/// Regla del JezzBall original: cuando un frente toca una pared (o un muro
/// prev), esa mitad "se convierte" y deja de estar en riesgo. Aquí se
/// vuelca a la rejilla como `Filled` y se marca `*_sealed`, de modo que
/// `vulnerable_cells` deja de incluirla y una bola que la toque no cuesta
/// nada — igual que cualquier other muro ya consolidado.
fn seal_finished_halves(s: &mut GameState) {
    // Una mitad no se sella sobre la celda que ocupa una bola: la encerraría
    // inside del muro. En esa situación se espera al frame siguiente (la bola
    // se habrá movido, o habrá roto el frente en `wall_impacts`).
    let ocupadas: Vec<(u16, u16)> = s
        .balls
        .iter()
        .filter_map(|b| ball_cell(b, &s.arena.grid))
        .collect();

    for builder in &mut s.builders {
        if builder.lo_done && !builder.lo_sealed {
            let celdas = builder.lo_cells();
            if !celdas.iter().any(|c| ocupadas.contains(c)) {
                for (cx, cy) in celdas {
                    if s.arena.grid.is_open(cx, cy) {
                        s.arena.grid.set(cx, cy, Cell::Filled);
                    }
                }
                builder.lo_sealed = true;
            }
        }
        if builder.hi_done && !builder.hi_sealed {
            let celdas = builder.hi_cells();
            if !celdas.iter().any(|c| ocupadas.contains(c)) {
                for (cx, cy) in celdas {
                    if s.arena.grid.is_open(cx, cy) {
                        s.arena.grid.set(cx, cy, Cell::Filled);
                    }
                }
                builder.hi_sealed = true;
            }
        }
    }
}

/// e) Impacto bola <-> muro en construcción.
fn wall_impacts(s: &mut GameState, events: &mut Vec<GameEvent>) {
    if s.builders.is_empty() {
        return;
    }
    let n = s.builders.len();
    let mut destroyed = vec![false; n];
    for (bi, d) in destroyed.iter_mut().enumerate() {
        if *d {
            continue;
        }
        // Sólo las mitades VIVAS están en riesgo: la que ya tocó pared se
        // sellló y se comporta como muro normal (regla del original).
        let cells = s.builders[bi].vulnerable_cells(&s.arena.grid);
        if cells.is_empty() {
            continue;
        }
        let mut shielded = s.builders[bi].shielded;
        for ball in &s.balls {
            // AABB barrido entre posiciones previa y current (anti-túnel).
            let min_x = ball.prev.x.min(ball.pos.x) - ball.radius;
            let max_x = ball.prev.x.max(ball.pos.x) + ball.radius;
            let min_y = ball.prev.y.min(ball.pos.y) - ball.radius;
            let max_y = ball.prev.y.max(ball.pos.y) + ball.radius;
            let overlap = cells.iter().any(|&(cx, cy)| {
                let (x0, x1) = (cx as f32, cx as f32 + 1.0);
                let (y0, y1) = (cy as f32, cy as f32 + 1.0);
                min_x < x1 && x0 < max_x && min_y < y1 && y0 < max_y
            });
            if overlap {
                if shielded && !ball.kind.ignores_shield() {
                    // El escudo absorbe exactamente este impacto.
                    shielded = false;
                } else {
                    *d = true;
                    break;
                }
            }
        }
        s.builders[bi].shielded = shielded;
    }

    // Aplicar destrucciones. La mitad ya sellada NO se borra: sus celdas
    // quedaron `Filled` en la rejilla y siguen ahí, igual que en el original.
    let mut keep: Vec<WallBuilder> = Vec::new();
    for (bi, builder) in s.builders.drain(..).enumerate() {
        if destroyed[bi] {
            // Sólo se retiran las celdas de las mitades que seguían vivas.
            if !builder.lo_sealed {
                for (cx, cy) in builder.lo_cells() {
                    if s.arena.grid.get(cx, cy) == Cell::Filled {
                        s.arena.grid.set(cx, cy, Cell::Open);
                    }
                }
            }
            if !builder.hi_sealed {
                for (cx, cy) in builder.hi_cells() {
                    if s.arena.grid.get(cx, cy) == Cell::Filled {
                        s.arena.grid.set(cx, cy, Cell::Open);
                    }
                }
            }
            if s.lives > 0 {
                s.lives -= 1;
            }
            events.push(GameEvent::LifeLost);
            if !s.level.purist {
                s.combo.reset();
                events.push(GameEvent::ComboReset);
            }
        } else {
            keep.push(builder);
        }
    }
    s.builders = keep;
}

/// f+g+h) Consolidar muros hechos, cerrar regiones, puntuar y subir combo.
fn consolidate_walls(s: &mut GameState, events: &mut Vec<GameEvent>) {
    let mut ready = Vec::new();
    let mut active = Vec::new();
    for builder in s.builders.drain(..) {
        if builder.is_done() {
            ready.push(builder);
        } else {
            active.push(builder);
        }
    }
    s.builders = active;

    for builder in ready {
        // f) El muro se consolida: sus celdas pasan a `Filled`.
        let cells = builder.cells();
        for (cx, cy) in &cells {
            if s.arena.grid.is_open(*cx, *cy) {
                s.arena.grid.set(*cx, *cy, Cell::Filled);
            }
        }

        // Riesgo y velocidad para la fórmula de puntuación (§5).
        let balls_near = count_balls_near(s, &cells);
        let max_speed = max_ball_speed(s);
        let open_before = s.arena.grid.open_count() as u32;

        // g) Partición: flood fill 4-conexo, cerrar regiones sin bola,
        //    puntuar cada una y dividir splitters.
        let (points, cells_closed) = partition_and_score(s, open_before, balls_near, max_speed);
        s.score += points;

        let wall = Wall {
            axis: builder.axis,
            origin: builder.origin,
            cells: cells_closed,
            points,
        };
        events.push(GameEvent::WallCompleted {
            points: wall.points,
            cells: wall.cells,
        });

        // h) Cada muro consolidado sin perder vida sube el combo (x8 tope).
        if !s.level.purist {
            s.combo.bump();
            s.max_combo_reached = s.max_combo_reached.max(s.combo.multiplier);
            events.push(GameEvent::ComboUp(s.combo.multiplier));
        }
    }
}

/// Bolas a menos de 4 celdas del segmento del muro al consolidar (§5).
fn count_balls_near(s: &GameState, cells: &[(u16, u16)]) -> u32 {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for &(cx, cy) in cells {
        min_x = min_x.min(cx as f32);
        min_y = min_y.min(cy as f32);
        max_x = max_x.max(cx as f32);
        max_y = max_y.max(cy as f32);
    }
    let mut near = 0;
    for ball in &s.balls {
        let px = ball.pos.x.clamp(min_x, max_x + 1.0);
        let py = ball.pos.y.clamp(min_y, max_y + 1.0);
        let dx = ball.pos.x - px;
        let dy = ball.pos.y - py;
        if (dx * dx + dy * dy).sqrt() < 4.0 {
            near += 1;
        }
    }
    near
}

/// Velocidad máxima entre todas las bolas (§5 formula `speed_b`).
fn max_ball_speed(s: &GameState) -> f32 {
    let mut max: f32 = 0.0;
    for ball in &s.balls {
        max = max.max(ball.speed());
    }
    max
}

/// g) Flood fill 4-conexo y cierre de regiones sin bola.
///
/// Devuelve `(puntos, celdas_cerradas)` para el evento `WallCompleted`.
fn partition_and_score(
    s: &mut GameState,
    open_before: u32,
    balls_near: u32,
    max_speed: f32,
) -> (u32, u32) {
    let regions = s.arena.grid.open_regions();
    let mut points = 0u32;
    let mut cells_closed = 0u32;

    for region in regions {
        let any_ball = region.iter().any(|&cell| {
            s.balls
                .iter()
                .any(|b| ball_cell(b, &s.arena.grid) == Some(cell))
        });
        if any_ball {
            // Región con bola(s): no se cierra. Si solo hay Splitters, se
            // dividen en 2 Normal (región encerrada con espacio, §8).
            split_splitters_in_region(s, &region);
            continue;
        }
        // Región vacía -> cerrada: todas sus celdas a `Filled` y se puntúa.
        let n = region.len() as u32;
        for &(cx, cy) in &region {
            s.arena.grid.set(cx, cy, Cell::Filled);
        }
        points += region_points(n, open_before, balls_near, max_speed, s.combo.multiplier);
        cells_closed += n;
    }

    (points, cells_closed)
}

/// `BallKind::Splitter`: al quedar "encerrada" su región (solo splitters en
/// ella), se divide en 2 bolas `Normal` si hay sitio (>= 2 celdas abiertas).
fn split_splitters_in_region(s: &mut GameState, region: &[(u16, u16)]) {
    let in_region: Vec<usize> = s
        .balls
        .iter()
        .enumerate()
        .filter(|(_, b)| ball_cell(b, &s.arena.grid).is_some_and(|cell| region.contains(&cell)))
        .map(|(i, _)| i)
        .collect();
    if in_region.is_empty() {
        return;
    }
    let only_splitters = in_region
        .iter()
        .all(|&i| s.balls[i].kind == BallKind::Splitter);
    if !only_splitters || region.len() < 2 {
        return;
    }

    let mut next_id = s.balls.iter().map(|b| b.id).max().unwrap_or(0);
    for &i in &in_region {
        let base = s.balls[i].pos;
        let speed = s.balls[i].speed();
        let dir = s.balls[i].vel.normalized();
        if speed <= 0.0 {
            continue;
        }
        // Las dos nuevas Normal nacen muy juntas (se separan por la velocidad)
        // y van ligeramente rotadas para no viajar en paralelo exacto.
        let pos_a = base + Vec2::new(-0.05, 0.0);
        let dir_a = dir.rotate(-0.35);
        let dir_b = dir.rotate(0.35);
        next_id += 1;
        let a = Ball::new(
            next_id,
            pos_a.x,
            pos_a.y,
            dir_a.x * speed,
            dir_a.y * speed,
            BallKind::Normal,
            1.0,
        );
        next_id += 1;
        let b = Ball::new(
            next_id,
            base.x,
            base.y,
            dir_b.x * speed,
            dir_b.y * speed,
            BallKind::Normal,
            1.0,
        );
        s.balls[i] = a;
        s.balls.push(b);
    }
}

/// i) Evaluar fin de partida, tiempo, objetivos y estrellas.
fn check_end(s: &mut GameState, events: &mut Vec<GameEvent>) {
    if s.phase != GamePhase::Running {
        return;
    }

    // Derrota por vidas agotadas o arena sin bolas.
    if s.lives == 0 || s.balls.is_empty() {
        s.phase = GamePhase::Lost;
        events.push(GameEvent::GameOver);
        return;
    }

    // Tiempo agotado.
    if let Some(limit) = s.level.time_limit {
        if s.elapsed >= limit {
            s.phase = GamePhase::Lost;
            events.push(GameEvent::GameOver);
            return;
        }
    }

    // Victoría por área rellenada.
    let ratio = s.arena.grid.filled_ratio();
    if ratio >= s.level.target_ratio {
        let stars = compute_stars(s);
        s.stars = stars;
        s.phase = GamePhase::Won;
        events.push(GameEvent::LevelCleared { stars });
    }
}

/// Estrellas: 1 por objetivo cumplido (máx 3). En modo Original (purist) no
/// hay objetivos y el número de estrellas es 0.
fn compute_stars(s: &mut GameState) -> u8 {
    if s.level.purist {
        return 0;
    }
    let ratio = s.arena.grid.filled_ratio();
    let mut stars = 0u8;
    for progress in s.objectives.iter_mut() {
        let done = match progress.objective {
            Objective::ClearRatio(r) => ratio >= r,
            Objective::NoLivesLost => s.lives == s.level.lives,
            Objective::UnderTime(t) => s.elapsed <= t,
            Objective::MinScore(sc) => s.score >= sc,
            Objective::KeepCombo(k) => s.max_combo_reached >= k,
            Objective::NoPowerUps => s.powerups_used == 0,
        };
        progress.done = done;
        if done {
            stars += 1;
        }
    }
    stars
}
