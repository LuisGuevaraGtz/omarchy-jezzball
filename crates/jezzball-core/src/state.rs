//! `GameState` + `step()`: the heart of the pure logic (ARCHITECTURE.md §6).
//!
//! `step` is PURE: it does not mutate the state it receives, it clones and
//! returns a new one. It is deterministic: same `state` + same `input` +
//! same `dt` => same result (there is no `Instant::now()` nor any external
//! source of randomness).
//!
//! Orchestration order imposed by the contract (commented at each step):
//!   a) apply the player's input
//!   b) timed power-ups over dt (SlowMotion scales it, Freeze cancels it)
//!   c) integrate balls with substeps (bounce on separate axes)
//!   d) advance the fronts of the builders
//!   e) ball <-> wall-under-construction impact (shield or life)
//!   f) consolidate completed walls
//!   g) 4-connected flood fill + region closing + scoring (section 5)
//!   h) combo window (4 s, cap x8, reset on losing a life)
//!   i) win / loss / time / objectives and stars
//!   j) emit `Vec<GameEvent>`

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

/// Execution phase of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GamePhase {
    Running,
    Paused,
    Won,
    Lost,
}

/// Player input: the only mutation surface (ARCHITECTURE.md §6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerInput {
    None,
    StartWall {
        cell: (u16, u16),
        axis: WallAxis,
    },
    /// Toggles the default axis of the next wall (read by the app layer).
    ToggleAxis,
    UsePowerUp(PowerUpKind),
    Pause,
    Resume,
    Restart,
}

/// What has happened, so that render/audio can react (ARCHITECTURE.md §6).
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

/// State of a secondary objective (stars).
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectiveProgress {
    pub objective: Objective,
    pub done: bool,
}

/// Complete state of a game. It derives `Clone, Debug, PartialEq`
/// (mandatory by contract) and all of its fields are public.
#[derive(Clone, Debug, PartialEq)]
pub struct GameState {
    pub level: LevelSpec,
    pub arena: Arena,
    pub balls: Vec<Ball>,
    pub builders: Vec<WallBuilder>,
    pub phase: GamePhase,
    /// Seconds elapsed with the game running (Running).
    pub elapsed: f32,
    pub score: u32,
    pub combo: Combo,
    /// Highest multiplier reached (`KeepCombo` objective).
    pub max_combo_reached: u8,
    pub lives: u8,
    pub powerups_active: Vec<ActivePowerUp>,
    pub pickups: Vec<PowerUp>,
    pub inventory: Vec<PowerUpKind>,
    pub pending_shield: bool,
    pub pending_double_wall: bool,
    /// Number of power-ups used (`NoPowerUps` objective).
    pub powerups_used: u32,
    /// Default axis (toggled by `ToggleAxis`).
    pub current_axis: WallAxis,
    pub objectives: Vec<ObjectiveProgress>,
    pub stars: u8,
    /// Countdown until the next power-up spawn.
    pub next_powerup_in: f32,
    pub rng: Rng64,
}

impl GameState {
    /// Builds a new game from a `LevelSpec`.
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
        // First power-up spawn: 30 +/- 10 s, deterministic.
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

/// The central function of the project (ARCHITECTURE.md §6).
pub fn step(state: &GameState, input: PlayerInput, dt: f32) -> (GameState, Vec<GameEvent>) {
    // We clone so as not to mutate the state we received (pure signature).
    let mut s = state.clone();
    let mut events = Vec::new();

    // Game over: the only thing left is to restart.
    if s.phase == GamePhase::Won || s.phase == GamePhase::Lost {
        if input == PlayerInput::Restart {
            return (GameState::new(s.level.clone()), events);
        }
        return (s, events);
    }

    // Paused: the clock stops completely.
    if s.phase == GamePhase::Paused {
        match input {
            PlayerInput::Resume => s.phase = GamePhase::Running,
            PlayerInput::Restart => return (GameState::new(s.level.clone()), events),
            _ => {}
        }
        return (s, events);
    }

    // Running:
    match input {
        PlayerInput::Pause => {
            s.phase = GamePhase::Paused;
            return (s, events);
        }
        PlayerInput::Restart => return (GameState::new(s.level.clone()), events),
        other => apply_input(&mut s, other, &mut events),
    }

    // Movers (moving obstacles): we update their occupancy BEFORE
    // integrating the balls so that the bounces see the new grid. They
    // destroy walls under construction with NO cost in lives (documented
    // decision; lives are only lost to ball impacts, ARCHITECTURE.md §3).
    let mover_cells = s.arena.update_movers(dt);
    destroy_builders_from_cells(&mut s, &mover_cells, &mut events);

    // b) Timed power-ups: here we decide the effective `dt` of the balls.
    //    SlowMotion scales it to x0.5; Freeze cancels it. Wall fronts always
    //    use the real `dt`.
    let ball_dt = advance_timed_powerups(&mut s, dt);

    // c) Integrate balls with substeps if |vel| * dt >= 0.5 (bounce on
    //    separate axes against the grid => classic 45-degree bounce).
    for ball in &mut s.balls {
        ball.step(&s.arena.grid, &mut s.rng, ball_dt);
    }

    // Collect power-ups from the floor if a ball steps on their cell.
    collect_pickups(&mut s);

    // d) Advance the fronts of the builders and spawn power-ups.
    for builder in &mut s.builders {
        builder.advance(&s.arena.grid, dt);
    }
    // e) Ball <-> wall-under-construction impact: the shield absorbs 1
    //    impact, Heavy ignores it; with no shield => wall destroyed + life
    //    lost. This goes BEFORE the sealing: a ball that is touching the
    //    front must break it, not end up enclosed by it.
    wall_impacts(&mut s, &mut events);

    // Sealing by halves (rule from the original JezzBall): as soon as a front
    // reaches its limit, THAT half is committed to the grid as `Filled` and
    // becomes immune. The other half keeps growing and remains the only one
    // that can cost a life.
    seal_finished_halves(&mut s);
    maybe_spawn_pickup(&mut s, dt, &mut events);

    // f+g+h) Consolidate walls, partition (flood fill), scoring and combo.
    consolidate_walls(&mut s, &mut events);

    // h) Combo window: it expires and goes back to x1.
    if !s.level.purist && s.combo.tick(dt) {
        events.push(GameEvent::ComboReset);
    }

    s.elapsed += dt;

    // i) Win / loss / time out / objectives and stars.
    check_end(&mut s, &mut events);

    (s, events)
}

/// Creates the balls of a game from their `BallSpawn`s, relocating any spawn
/// that does not land on an `Open` cell of the already materialized arena.
fn spawn_balls(spawns: &[crate::level::BallSpawn], grid: &Grid) -> Vec<Ball> {
    spawns
        .iter()
        .enumerate()
        .filter_map(|(i, spawn)| new_or_relocated_ball(i as u32, spawn, grid))
        .collect()
}

/// Creates a ball from its `BallSpawn` and guarantees the invariant "every
/// ball is born on an `Open` cell".
///
/// Why a defence is needed here: `tools/gen_levels.py` generates the
/// `ArenaShape::Maze` levels with a recursive-backtracker maze seeded with
/// Python's `random` and places the ball spawns on the open cells OF ITS
/// maze. In the engine, `Arena::from_spec` -> `materialize_maze` reproduces
/// the algorithm but with ANOTHER random number generator (`Rng64`,
/// xorshift). Two different RNGs => two different mazes => a spawn the
/// generator believed to be open can land inside a `Solid` wall (or outside
/// the arena). Instead of replicating Python's RNG (which would couple the
/// engine to an external script and would keep breaking whenever either of
/// the two changed), the engine defends the invariant: if the spawn does not
/// land on an `Open` cell, a breadth-first search from the spawn's cell
/// finds the first `Open` cell that is not `NoSplit` and the ball is
/// relocated there, keeping its velocity, its `kind` and its radius. If
/// there is no open cell at all in the whole arena (degenerate level), the
/// ball is discarded instead of causing an invalid state.
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

/// Breadth-first search from cell `(sx, sy)`: returns the first `Open` cell
/// that is not `NoSplit`. `None` if there is none (degenerate arena).
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

/// a) Apply the player's input (without Pause/Resume/Restart, already
/// handled).
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

/// Starts a wall (`PlayerInput::StartWall`).
fn start_wall(s: &mut GameState, cell: (u16, u16), axis: WallAxis, events: &mut Vec<GameEvent>) {
    // Outside the arena or on a non-open cell -> blocked.
    if cell.0 >= s.arena.grid.w || cell.1 >= s.arena.grid.h {
        events.push(GameEvent::WallBlocked);
        return;
    }
    if !s.arena.grid.is_open(cell.0, cell.1) || s.arena.grid.is_no_split(cell.0, cell.1) {
        events.push(GameEvent::WallBlocked);
        return;
    }
    // A single builder at a time, except with the Double Wall power-up (2).
    let max_builders = if s.pending_double_wall { 2 } else { 1 };
    if s.builders.len() >= max_builders {
        // Silently ignored (documented decision: no event is emitted because
        // it is not a failure of the wall itself, it is an invalid input).
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

/// Uses a power-up from the inventory if it is available
/// (ARCHITECTURE.md §9).
fn use_powerup(s: &mut GameState, kind: PowerUpKind, events: &mut Vec<GameEvent>) {
    // Original mode: no power-ups (a single code path with guards).
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
                s.inventory.push(kind); // already pending: it is not consumed
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
                s.inventory.push(kind); // the arena cannot be left with no balls
                false
            }
        }
    };
    if used {
        s.powerups_used += 1;
    }
}

/// Index of the slowest ball (for `RemoveBall`).
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

/// b) Advances the timed power-ups and returns the effective `dt` for balls.
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

/// Cell the centre of the ball is in.
fn ball_cell(ball: &Ball, grid: &Grid) -> Option<(u16, u16)> {
    let x = ball.pos.x.floor() as i64;
    let y = ball.pos.y.floor() as i64;
    if grid.in_bounds(x, y) {
        Some((x as u16, y as u16))
    } else {
        None
    }
}

/// Collects power-ups from the floor when a ball steps on their cell
/// (filling the inventory up to `INVENTORY_MAX`).
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

/// d) Spawns a collectible power-up every 30 +/- 10 s (max `PICKUP_MAX` in
/// the arena).
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

/// Random open (and non-`NoSplit`) cell for a power-up spawn.
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

/// Destroys builders brushed by occupied cells (here: by a `Mover`).
/// It costs no lives.
fn destroy_builders_from_cells(
    s: &mut GameState,
    occupied: &[(u16, u16)],
    events: &mut Vec<GameEvent>,
) {
    if occupied.is_empty() || s.builders.is_empty() {
        return;
    }
    s.builders.retain(|builder| {
        // Same as with the balls: a Mover can only break the halves that are
        // still growing, not the one that already sealed against the wall.
        let vulnerable = builder.vulnerable_cells(&s.arena.grid);
        let hit = vulnerable.iter().any(|c| occupied.contains(c));
        if hit {
            events.push(GameEvent::WallBlocked);
        }
        !hit
    });
}

/// Seals the wall halves that have already reached their limit.
///
/// Rule from the original JezzBall: when a front touches a wall (or a
/// previous wall), that half "sets" and is no longer at risk. Here it is
/// committed to the grid as `Filled` and marked `*_sealed`, so that
/// `vulnerable_cells` stops including it and a ball touching it costs
/// nothing — just like any other already consolidated wall.
fn seal_finished_halves(s: &mut GameState) {
    // A half does not seal over the cell a ball occupies: it would enclose it
    // inside the wall. In that situation we wait for the next frame (the ball
    // will have moved, or will have broken the front in `wall_impacts`).
    let occupied: Vec<(u16, u16)> = s
        .balls
        .iter()
        .filter_map(|b| ball_cell(b, &s.arena.grid))
        .collect();

    for builder in &mut s.builders {
        if builder.lo_done && !builder.lo_sealed {
            let cells = builder.lo_cells();
            if !cells.iter().any(|c| occupied.contains(c)) {
                for (cx, cy) in cells {
                    if s.arena.grid.is_open(cx, cy) {
                        s.arena.grid.set(cx, cy, Cell::Filled);
                    }
                }
                builder.lo_sealed = true;
            }
        }
        if builder.hi_done && !builder.hi_sealed {
            let cells = builder.hi_cells();
            if !cells.iter().any(|c| occupied.contains(c)) {
                for (cx, cy) in cells {
                    if s.arena.grid.is_open(cx, cy) {
                        s.arena.grid.set(cx, cy, Cell::Filled);
                    }
                }
                builder.hi_sealed = true;
            }
        }
    }
}

/// e) Ball <-> wall-under-construction impact.
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
        // Only the LIVE halves are at risk: the one that already touched a
        // wall has sealed and behaves like a normal wall (rule from the
        // original).
        let cells = s.builders[bi].vulnerable_cells(&s.arena.grid);
        if cells.is_empty() {
            continue;
        }
        let mut shielded = s.builders[bi].shielded;
        for ball in &s.balls {
            // Swept AABB between the previous and current positions
            // (anti-tunnelling).
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
                    // The shield absorbs exactly this impact.
                    shielded = false;
                } else {
                    *d = true;
                    break;
                }
            }
        }
        s.builders[bi].shielded = shielded;
    }

    // Apply the destructions. The already sealed half is NOT erased: its
    // cells were left `Filled` in the grid and stay there, just like in the
    // original.
    let mut keep: Vec<WallBuilder> = Vec::new();
    for (bi, builder) in s.builders.drain(..).enumerate() {
        if destroyed[bi] {
            // Only the cells of the halves that were still alive are removed.
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

/// f+g+h) Consolidate finished walls, close regions, score and raise combo.
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
        // f) The wall consolidates: its cells become `Filled`.
        let cells = builder.cells();
        for (cx, cy) in &cells {
            if s.arena.grid.is_open(*cx, *cy) {
                s.arena.grid.set(*cx, *cy, Cell::Filled);
            }
        }

        // Risk and speed for the scoring formula (§5).
        let balls_near = count_balls_near(s, &cells);
        let max_speed = max_ball_speed(s);
        let open_before = s.arena.grid.open_count() as u32;

        // g) Partition: 4-connected flood fill, close the regions with no
        //    ball, score each one and split the splitters.
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

        // h) Every wall consolidated without losing a life raises the combo
        //    (x8 cap).
        if !s.level.purist {
            s.combo.bump();
            s.max_combo_reached = s.max_combo_reached.max(s.combo.multiplier);
            events.push(GameEvent::ComboUp(s.combo.multiplier));
        }
    }
}

/// Balls less than 4 cells away from the wall segment on consolidation (§5).
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

/// Maximum speed among all the balls (§5, `speed_b` formula).
fn max_ball_speed(s: &GameState) -> f32 {
    let mut max: f32 = 0.0;
    for ball in &s.balls {
        max = max.max(ball.speed());
    }
    max
}

/// g) 4-connected flood fill and closing of the regions with no ball.
///
/// Returns `(points, closed_cells)` for the `WallCompleted` event.
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
            // Region with ball(s): it is not closed. If it only holds
            // Splitters, they split into 2 Normal ones (enclosed region with
            // room, §8).
            split_splitters_in_region(s, &region);
            continue;
        }
        // Empty region -> closed: all of its cells to `Filled` and it scores.
        let n = region.len() as u32;
        for &(cx, cy) in &region {
            s.arena.grid.set(cx, cy, Cell::Filled);
        }
        points += region_points(n, open_before, balls_near, max_speed, s.combo.multiplier);
        cells_closed += n;
    }

    (points, cells_closed)
}

/// `BallKind::Splitter`: when its region becomes "enclosed" (only splitters
/// in it), it splits into 2 `Normal` balls if there is room (>= 2 open
/// cells).
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
        // The two new Normal balls are born very close together (they are
        // separated by their velocity) and go slightly rotated so they do not
        // travel exactly in parallel.
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

/// i) Evaluate end of game, time, objectives and stars.
fn check_end(s: &mut GameState, events: &mut Vec<GameEvent>) {
    if s.phase != GamePhase::Running {
        return;
    }

    // Loss due to running out of lives or to an arena with no balls.
    if s.lives == 0 || s.balls.is_empty() {
        s.phase = GamePhase::Lost;
        events.push(GameEvent::GameOver);
        return;
    }

    // Time out.
    if let Some(limit) = s.level.time_limit {
        if s.elapsed >= limit {
            s.phase = GamePhase::Lost;
            events.push(GameEvent::GameOver);
            return;
        }
    }

    // Win by filled area.
    let ratio = s.arena.grid.filled_ratio();
    if ratio >= s.level.target_ratio {
        let stars = compute_stars(s);
        s.stars = stars;
        s.phase = GamePhase::Won;
        events.push(GameEvent::LevelCleared { stars });
    }
}

/// Stars: 1 per objective achieved (max 3). In Original mode (purist) there
/// are no objectives and the number of stars is 0.
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
