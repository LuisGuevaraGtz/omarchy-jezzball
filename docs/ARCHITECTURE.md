# Omarchy-Jezzball — Contrato de Arquitectura

Documento normativo. Todo worker implementa CONTRA este contrato.
Si algo no está aquí, se decide en el crate que lo usa y se documenta.

## 0. Principios

- Nativo, ligero, sin Electron, sin motor pesado, sin runtime extra.
- Wayland-first: nada de dependencias X11 obligatorias.
- Keyboard-first: todo el menú y meta-acciones son teclado; el mouse solo traza muros.
- Estética terminal/dark, coherente con el tema activo de Omarchy.
- Lógica de juego = determinista, pura, testeable, sin I/O ni render.

## 1. Workspace

```
Omarchy-Jezzball/
├─ Cargo.toml                 # workspace virtual, members = core/app
├─ crates/
│  ├─ jezzball-core/          # LÓGICA PURA. cero deps de render/OS.
│  │  ├─ src/
│  │  │  ├─ lib.rs
│  │  │  ├─ geom.rs           # Vec2, Rect, Aabb, helpers
│  │  │  ├─ ball.rs           # Ball, BallKind, integración + rebote
│  │  │  ├─ wall.rs           # Wall, WallBuilder (muro en construcción)
│  │  │  ├─ grid.rs           # ocupación por celdas + flood fill
│  │  │  ├─ arena.rs          # Arena, layouts, partición
│  │  │  ├─ level.rs          # LevelSpec, World, Objective, Star
│  │  │  ├─ score.rs          # riesgo/recompensa, combos
│  │  │  ├─ powerup.rs        # PowerUp, efectos temporizados
│  │  │  └─ state.rs          # GameState + step() + apply_input()
│  │  └─ tests/
│  ├─ jezzball-app/           # macroquad: render, input, audio, save
│  │  └─ src/
│  │     ├─ main.rs
│  │     ├─ theme.rs          # lectura del tema Omarchy
│  │     ├─ persist.rs        # XDG save/load
│  │     ├─ input.rs          # mapeo teclado/mouse -> PlayerInput
│  │     ├─ render/
│  │     │  ├─ mod.rs
│  │     │  ├─ arena.rs
│  │     │  ├─ hud.rs
│  │     │  └─ menu.rs
│  │     └─ screens.rs        # menú, juego, pausa, resultados
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

Regla dura: `jezzball-core` NO puede depender de `macroquad`, `std::fs`,
`std::time`, ni de nada del SO. Solo `serde` (opt-in) y aritmética.
El tiempo entra como `dt: f32`. La aleatoriedad entra como semilla `u64`
con un PRNG determinista propio (xorshift64*), nunca `rand` con entropía del SO.

## 2. Modelo de la arena (híbrido rejilla+continuo)

El JezzBall original es rejilla. Nosotros usamos:

- **Rejilla lógica** `Grid { w, h, cells: Vec<Cell> }` donde
  `Cell = Open | Filled | Solid` (Solid = obstáculo del mundo 3).
  Es la fuente de verdad para "qué está cerrado" y para el % de área.
- **Bolas continuas** en coordenadas de celda con `f32` (pos, vel, radio),
  para movimiento suave a 60 FPS y rebote correcto en diagonales.

Tamaño de celda en px lo decide la capa de render, NO el core.
El core trabaja en "unidades de celda". Arena por defecto: 64x40 celdas.

### Colisión bola↔rejilla
Integración semi-implícita con resolución por ejes separados:
1. mover en X, si la celda destino no es `Open` → revertir X e invertir `vel.x`.
2. mover en Y, idéntico.
Esto da el rebote de 45° clásico sin túneles si `|vel| * dt < 0.5` celdas.
Si `|vel| * dt >= 0.5`, el core hace substeps internos (máx. 8).

## 3. Muros

```rust
pub enum WallAxis { Horizontal, Vertical }

pub struct WallBuilder {
    pub axis: WallAxis,
    pub origin: (u16, u16),   // celda donde el jugador hizo click
    pub lo: f32,              // frente que crece hacia -X/-Y (en celdas)
    pub hi: f32,              // frente que crece hacia +X/+Y
    pub lo_done: bool,        // llegó a borde/obstáculo
    pub hi_done: bool,
    pub speed: f32,           // celdas por segundo, por frente
    pub shielded: bool,       // power-up Escudo: absorbe 1 impacto
}
```

Comportamiento (idéntico al original):
- Un solo `WallBuilder` activo a la vez (salvo power-up Muro Doble → 2).
- Crece en AMBAS direcciones simultáneamente desde `origin`.
- Un frente se detiene al tocar `Filled`/`Solid`/borde → `*_done = true`.
- Cuando `lo_done && hi_done` → el muro se **consolida**: las celdas del
  segmento pasan a `Filled` y se dispara la partición (sección 4).
- Si una bola toca CUALQUIER celda de un frente aún en construcción:
  - si `shielded` → se consume el escudo, el muro sobrevive.
  - si no → muro destruido, `lives -= 1`, combo a 0.

## 4. Partición y cierre de área (el corazón)

Tras consolidar un muro:
1. Flood fill 4-conexo sobre celdas `Open`, etiquetando regiones.
2. Para cada región, ver si contiene alguna bola (por su celda).
3. Regiones **sin bola** → todas sus celdas pasan a `Filled`.
4. Recalcular `filled_ratio = filled_cells / fillable_cells`.
5. Puntuar cada región cerrada (sección 5).
6. `filled_ratio >= level.target_ratio` (0.75 por defecto) → nivel superado.

Complejidad: O(celdas) por muro. 64x40 = 2560 celdas → trivial a 60 FPS.

## 5. Puntuación riesgo/recompensa

Por cada región cerrada de `n` celdas, en una arena con `open` celdas
abiertas antes del cierre:

```
frac      = n / open                      // 0..1, qué tan grande fue
size_mult = lerp(3.0, 0.4, frac)          // pequeño = arriesgado = paga más
risk      = 1.0 + 0.35 * balls_near       // bolas a <4 celdas del muro al consolidar
speed_b   = 1.0 + 0.10 * max_ball_speed   // arena rápida = más riesgo
base      = 100.0 * n.sqrt()
points    = base * size_mult * risk * speed_b * combo_mult
```

Combos: cada muro consolidado sin perder vida dentro de `COMBO_WINDOW = 4.0 s`
sube el multiplicador: x1 → x2 → x3 → x4 (tope x8). Se reinicia al perder
una vida o al expirar la ventana.

## 6. Entrada del jugador (única superficie de mutación)

```rust
pub enum PlayerInput {
    None,
    StartWall { cell: (u16, u16), axis: WallAxis },
    ToggleAxis,                 // tecla: alterna H/V para el próximo muro
    UsePowerUp(PowerUpKind),
    Pause, Resume, Restart,
}
```

La función pura y central que TODO worker debe respetar:

```rust
pub fn step(state: &GameState, input: PlayerInput, dt: f32) -> (GameState, Vec<GameEvent>)
```

- No muta `state`; devuelve uno nuevo (o usa `&mut self` internamente clonando).
- `GameEvent` describe qué pasó, para que el render/audio reaccione:
  `WallStarted, WallBlocked, WallCompleted{points, cells}, BallLost,
   LifeLost, ComboUp(u8), ComboReset, PowerUpSpawned, LevelCleared{stars}, GameOver`.
- Determinismo: mismo `state` + mismo `input` + mismo `dt` → mismo resultado.
  Esto es lo que hace los tests posibles. No hay `Instant::now()` en el core.

## 7. Modos

```rust
pub enum Mode { Original, Enhanced }
```

- `Original`: 10 niveles. `balls = 2 + (level-1)/2` aprox., sin power-ups,
  sin obstáculos, sin combos (el combo existe pero se fija a x1),
  sin objetivos secundarios. Flag `level.purist = true` desactiva todo lo moderno.
- `Enhanced`: 60 niveles iniciales (6 mundos × 10), estructura ampliable
  sin recompilar (los niveles viven en `assets/levels/enhanced.ron`).

Mundos: 1 Clásico, 2 Velocidad, 3 Obstáculos, 4 Bolas especiales,
5 Power-ups, 6 Retos avanzados. Niveles 5/10 de cada mundo son especiales:
`Boss` (bola grande lenta), `SpeedRun` (reloj), `Chaos` (bolas pequeñas
rápidas + obstáculos + 1 vida).

## 8. Bolas especiales

| Kind | Comportamiento |
|---|---|
| `Normal` | radio 0.45, vel 8 celdas/s |
| `Fast` | vel ×1.8 |
| `Erratic` | cada 1.5 s rota su velocidad ±30° (PRNG determinista) |
| `Splitter` | al ser encerrada su región, se divide en 2 `Normal` si hay sitio |
| `Heavy` | radio 1.2, vel ×0.6, destruye muros en construcción SIEMPRE (ignora escudo) |

## 9. Power-ups (solo Enhanced, mundos ≥5)

`SlowMotion` (dt×0.5, 5 s), `Freeze` (bolas quietas, 3 s),
`DoubleWall` (2 builders simultáneos, 1 uso), `Shield` (1 impacto absorbido),
`RemoveBall` (elimina la bola más lenta, 1 uso).
Spawnean como celda recogible cada `30±10 s`, máx 2 en arena.

## 10. Objetivos y estrellas

```rust
pub enum Objective {
    ClearRatio(f32),      // reducir el área al X%
    NoLivesLost,
    UnderTime(f32),
    MinScore(u32),
    KeepCombo(u8),
    NoPowerUps,
}
```
1 estrella por objetivo cumplido, máx 3 por nivel. Rejugable: se guarda el
máximo histórico de estrellas y puntuación.

## 11. Persistencia (capa app, nunca core)

Ruta: `$XDG_DATA_HOME/omarchy-jezzball/save.ron`
(fallback `~/.local/share/omarchy-jezzball/save.ron`).
Config: `$XDG_CONFIG_HOME/omarchy-jezzball/config.ron`.
Escritura atómica: escribir a `save.ron.tmp` + `rename`. Sin red, sin cuentas.

```rust
struct SaveData {
    version: u32,
    original: BTreeMap<u16, LevelRecord>,   // level -> record
    enhanced: BTreeMap<u16, LevelRecord>,
    original_completed: bool,
}
struct LevelRecord { best_score: u32, stars: u8, completed: bool, best_time: f32 }
```

## 12. Theming Omarchy

CORRECCIÓN (verificado en la máquina real): Omarchy NO guarda el tema activo
en `~/.config/omarchy/current`. La ruta real es
`~/.local/state/omarchy/current/theme/` (un symlink al tema activo) y el
nombre del tema está en `~/.local/state/omarchy/current/theme.name`.
Los temas del sistema viven en `/usr/share/omarchy/themes/<nombre>/`.

Fuente de verdad preferida: `colors.toml` del tema, que es un TOML PLANO
(sin secciones) con claves directas. Ejemplo real (tema `retro-82`):

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

Orden de resolución (primer hit gana):
1. `$XDG_CONFIG_HOME/omarchy-jezzball/theme.ron` (override del usuario).
2. `$XDG_STATE_HOME/omarchy/current/theme/colors.toml`
   (fallback `~/.local/state/omarchy/current/theme/colors.toml`).
3. Leer el nombre en `~/.local/state/omarchy/current/theme.name` y buscar
   `/usr/share/omarchy/themes/<nombre>/colors.toml`.
4. `~/.config/omarchy/themes/<nombre>/colors.toml` (temas de usuario).
5. Como última fuente antes del fallback: `alacritty.toml` del tema, de donde
   se extrae `colors.primary.background/foreground` y `colors.normal.*`.
6. Fallback hardcodeado: bg `#0f0f14`, fg `#c9d1d9`, acento `#7aa2f7`.

Mapeo de roles del juego → claves de `colors.toml`:

| Rol | Clave |
|---|---|
| `bg` | `background` |
| `bg_panel` | `lighter_background` (HUD, paneles) |
| `fg` | `foreground` |
| `fg_dim` | `dark_foreground` (texto secundario) |
| `wall` | `muted` (muro consolidado) |
| `wall_building` | `yellow` (muro EN RIESGO, debe cantar) |
| `ball` | `cyan` |
| `ball_special` | `magenta` |
| `danger` | `red` (vidas, fallo) |
| `ok` | `green` (objetivo cumplido) |
| `accent` | `accent` (selección de menú, combo) |

Nunca hardcodear color en el render: todo sale de `Theme`.
Re-leer el tema al recuperar foco (barato) para seguir cambios en caliente.
Fixtures reales para tests en `crates/jezzball-app/tests/fixtures/`
(`retro-82-colors.toml`, `tokyo-night-colors.toml`, `retro-82-alacritty.toml`).

## 13. Render / input (capa app)

- `macroquad` 0.4 (backend `miniquad`, soporta Wayland nativo vía GLFW/EGL;
  se fuerza Wayland con `SDL_VIDEODRIVER`/`WINIT_UNIX_BACKEND` no aplican —
  miniquad usa X11 por defecto: documentar `--x11` fallback y habilitar
  la feature de Wayland si está disponible; si no, XWayland funciona pero
  se prefiere `ggez`/`raylib-rs` como plan B).
- Loop: `next_frame().await` da vsync del compositor. `dt = get_frame_time()`,
  clamp a 1/30 para evitar saltos tras un stall.
- El render NO decide nada de juego: lee `&GameState` y dibuja.

Teclado (keyboard-first):
`Space` pausa · `R` reinicia nivel · `Esc` menú/atrás · `Tab` alterna eje
del muro · `1..5` usar power-up · `Enter` confirmar · `↑↓←→`/`hjkl` navegar menú
· `F` alternar HUD compacto · `Q` salir (con confirmación).
Mouse: click izq = muro según eje actual; click der = eje contrario.

## 14. Empaquetado

- `PKGBUILD` (`pkgname=omarchy-jezzball`), `makedepends=(cargo)`,
  `depends=(wayland libxkbcommon)`, build con `cargo build --release --locked`,
  instala binario en `/usr/bin`, assets en `/usr/share/omarchy-jezzball`,
  `.desktop` en `/usr/share/applications`.
- `.desktop`: `Categories=Game;ArcadeGame;`, `Terminal=false`,
  `StartupWMClass=omarchy-jezzball` (para reglas de ventana de Hyprland).
- Documentar regla Hyprland sugerida:
  `windowrulev2 = float, class:^(omarchy-jezzball)$`

## 15. Calidad

- `cargo test -p jezzball-core` debe pasar. Mínimo:
  rebote en pared, muro consolidado cierra región vacía, región con bola NO
  se cierra, bola impacta muro en construcción → vida perdida,
  puntuación pequeña < puntuación grande invertida, combo sube y expira,
  determinismo (mismo seed → misma traza de 600 steps).
- `cargo clippy -- -D warnings` limpio en core.
- Sin `unwrap()` en rutas de I/O de la app.
