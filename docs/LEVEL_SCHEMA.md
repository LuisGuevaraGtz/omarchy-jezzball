# Apéndice A — Esquema de datos de niveles (normativo)

Los workers de niveles y el crate `jezzball-core` DEBEN usar exactamente
estos tipos. Cualquier cambio se propaga a ambos lados.

## Tipos Rust (definidos en `jezzball-core/src/level.rs`)

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LevelSpec {
    pub id: u16,                  // 1-based, único dentro del modo
    pub name: String,             // corto, mostrable en HUD
    pub world: u8,                // 0 = Original; 1..=6 = mundos Enhanced
    pub kind: LevelKind,
    pub arena: ArenaSpec,
    pub balls: Vec<BallSpawn>,
    pub target_ratio: f32,        // 0.0..1.0, típico 0.75
    pub lives: u8,
    pub wall_speed: f32,          // celdas/seg por frente, típico 22.0
    pub time_limit: Option<f32>,  // segundos; None = sin reloj
    pub powerups_enabled: bool,
    pub objectives: Vec<Objective>, // máx 3; vacío en Original
    pub purist: bool,             // true => sin combos, sin power-ups, sin objetivos
    pub seed: u64,                // semilla del PRNG determinista
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum LevelKind { Standard, Boss, SpeedRun, Chaos }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArenaSpec {
    pub w: u16,                   // celdas, 32..=96
    pub h: u16,                   // celdas, 20..=60
    pub shape: ArenaShape,
    pub obstacles: Vec<Obstacle>, // celdas Solid pre-colocadas
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum ArenaShape {
    Rect,                         // rectángulo estándar
    Wide,                         // ratio ancho
    Tall,                         // ratio alto
    Irregular { notch: u16 },     // esquinas recortadas de `notch` celdas
    Circle,                       // elipse inscrita; fuera = Solid
    Maze { density: u8 },         // 0..=100, corredores generados por `seed`
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Obstacle {
    Block { x: u16, y: u16, w: u16, h: u16 },
    // se mueve en línea recta rebotando; destruye muros en construcción
    Mover { x: f32, y: f32, w: u16, h: u16, vx: f32, vy: f32 },
    // zona que NUNCA puede rellenarse ni cerrarse (cuenta como no-fillable)
    NoSplit { x: u16, y: u16, w: u16, h: u16 },
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct BallSpawn {
    pub x: f32, pub y: f32,       // celdas
    pub vx: f32, pub vy: f32,     // celdas/seg
    pub kind: BallKind,
    pub radius_mul: f32,          // 1.0 = normal
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum BallKind { Normal, Fast, Erratic, Splitter, Heavy }

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Objective {
    ClearRatio(f32), NoLivesLost, UnderTime(f32),
    MinScore(u32), KeepCombo(u8), NoPowerUps,
}
```

## Formato en disco

`assets/levels/original.ron` y `assets/levels/enhanced.ron` contienen
un `Vec<LevelSpec>` en RON. Ejemplo mínimo válido:

```ron
[
    (
        id: 1,
        name: "Primeros Rebotes",
        world: 0,
        kind: Standard,
        arena: (w: 64, h: 40, shape: Rect, obstacles: []),
        balls: [
            (x: 20.0, y: 12.0, vx: 7.0, vy: 6.0, kind: Normal, radius_mul: 1.0),
            (x: 44.0, y: 28.0, vx: -6.0, vy: 7.0, kind: Normal, radius_mul: 1.0),
        ],
        target_ratio: 0.75,
        lives: 3,
        wall_speed: 22.0,
        time_limit: None,
        powerups_enabled: false,
        objectives: [],
        purist: true,
        seed: 1001,
    ),
]
```

## Reglas de generación

### Original (10 niveles, `original.ron`)
- `world: 0`, `kind: Standard`, `purist: true`, `objectives: []`,
  `powerups_enabled: false`, `obstacles: []`, `shape: Rect`, `time_limit: None`.
- Bolas: nivel N tiene `1 + N` bolas (nivel 1 → 2 bolas ... nivel 10 → 11).
  Esto reproduce la curva del JezzBall original.
- Todas las bolas `Normal`, `radius_mul: 1.0`.
- Velocidad base 8.0 celdas/s en nivel 1, subiendo ~4% por nivel (tope 11.5).
- `lives`: 3 en niveles 1-3, 4 en 4-7, 5 en 8-10 (el original da vidas = nivel,
  nosotros lo acotamos para que siga siendo justo).
- `target_ratio: 0.75` fijo.
- `wall_speed`: 22.0 fijo.
- Posiciones iniciales: repartidas, nunca a menos de 4 celdas del borde
  ni a menos de 5 celdas entre sí. Velocidades con componentes no nulas
  y no exactamente iguales entre bolas (evitar simetrías aburridas).
- `seed`: 1000 + id.

### Enhanced (60 niveles, `enhanced.ron`)
`world = 1 + (id-1)/10`, o sea ids 1-10 → mundo 1, 11-20 → mundo 2, etc.
`purist: false` siempre. `seed`: 2000 + id.

Niveles especiales dentro de cada mundo:
- el nivel 5 de cada mundo → `kind: SpeedRun` con `time_limit: Some(...)`.
- el nivel 10 de cada mundo → `kind: Boss` (mundos 1,2,4) o `Chaos` (mundos 3,5,6).

Por mundo:
- **M1 Clásico (1-10)**: como Original pero `purist: false`, combos activos,
  1-2 objetivos por nivel, sin obstáculos, `shape: Rect`. Bolas 2→6.
- **M2 Velocidad (11-20)**: velocidades 12-18 celdas/s, algunas `Fast`,
  `time_limit` en la mitad de los niveles, `wall_speed` 24-28.
  `shape` alterna Rect/Wide/Tall. Bolas 3→7.
- **M3 Obstáculos (21-30)**: 2-6 `Block` por nivel, a partir del 25 aparecen
  `Mover`, a partir del 27 aparece 1 `NoSplit`. `shape` incluye Irregular.
  Bolas 3→6. Bajar `target_ratio` a 0.70 donde haya mucho `Solid`.
- **M4 Bolas especiales (31-40)**: introducción gradual — 31-32 `Erratic`,
  33-34 `Splitter`, 35-36 `Heavy`, 37-40 mezclas. Máx 2 `Heavy` por nivel,
  máx 2 `Splitter`. Mezclar con obstáculos ligeros. `shape` incluye Circle.
- **M5 Power-ups (41-50)**: `powerups_enabled: true`, dificultad claramente
  por encima de M4 para que el power-up sea necesario, no un regalo.
  Objetivo `NoPowerUps` en 2-3 niveles como reto opcional. `shape` variado.
- **M6 Retos avanzados (51-60)**: todo combinado, `shape: Maze` en 3+ niveles,
  `target_ratio` hasta 0.82, `lives` 2-3, 3 objetivos por nivel,
  nivel 60 = `Chaos` final, el más duro del juego.

Objetivos: 2-3 por nivel en Enhanced, coherentes con el nivel
(no pedir `UnderTime(30.0)` en un nivel que razonablemente toma 90 s;
no pedir `NoPowerUps` si `powerups_enabled: false`;
`ClearRatio` debe ser > `target_ratio`).

### Validaciones obligatorias
Ningún spawn de bola dentro de un `Obstacle` o fuera de la forma de arena.
Ninguna bola con `vx == 0.0 && vy == 0.0`.
`target_ratio` en 0.60..=0.85. `lives` en 1..=5. `wall_speed` en 18.0..=30.0.
`objectives.len() <= 3`. Ids consecutivos sin huecos empezando en 1.
