//! Pure-logic crate of Omarchy-Jezzball (ARCHITECTURE.md §1).
//!
//! Comments, documentation, type names and function names are all in English.
//! Hard rules:
//! - No `macroquad`, `std::fs`, `std::time`, `rand` or OS dependencies.
//! - Time always comes in as `dt: f32`.
//! - Randomness is our own deterministic PRNG (`rng::Rng64`), seeded from
//!   `LevelSpec::seed`. Same seed => same game.
//! - No `unsafe`. No `unwrap()` outside tests.

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
