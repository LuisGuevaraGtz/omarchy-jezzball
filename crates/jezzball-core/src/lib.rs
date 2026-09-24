//! Crate de lógica pura de Omarchy-Jezzball (ARCHITECTURE.md §1).
//!
//! Comentarios y documentación en español; nombres de tipos y funciones en inglés.
//! Reglas duras:
//! - Sin `macroquad`, `std::fs`, `std::time`, `rand` ni dependencias del SO.
//! - El tiempo entra siempre como `dt: f32`.
//! - La aleatoriedad es un PRNG determinista propio (`rng::Rng64`), sembrado
//!   desde `LevelSpec::seed`. Mismo seed => misma partida.
//! - Sin `unsafe`. Sin `unwrap()` fuera de tests.

pub mod arena;
pub mod ball;
pub mod geom;
pub mod grid;
pub mod level;
pub mod powerup;
pub mod rng;
pub mod score;
pub mod state;
pub mod wall;

pub use state::{step, GameEvent, GamePhase, GameState, PlayerInput};
