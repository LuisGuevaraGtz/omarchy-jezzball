//! Wall front growth (ARCHITECTURE.md §3).
//!
//! A front must stop at the FIRST blocking cell it meets. With a high
//! `wall_speed` (up to 30 cells/s) and a long frame, a front can advance
//! several cells at once: if only the destination cell is checked, it jumps
//! over walls and obstacles.

use jezzball_core::grid::{Cell, Grid};
use jezzball_core::wall::{WallAxis, WallBuilder};

/// The `hi` front must not skip past an obstacle even if it advances several
/// cells in a single step.
#[test]
fn hi_front_does_not_cross_an_obstacle_with_a_large_step() {
    let mut g = Grid::new(40, 20);
    // Solid block at column 12, row 10.
    g.set(12, 10, Cell::Solid);

    let mut b = WallBuilder::new(WallAxis::Horizontal, (5, 10), 30.0, false);
    // A long frame (~3 cells at 30 cells/s): the front goes from 5 to ~14,
    // skipping column 12 if only the destination cell is looked at.
    b.advance(&g, 0.3);

    let cells = b.cells();
    assert!(
        !cells.iter().any(|&(x, y)| x >= 12 && y == 10),
        "the front crossed the obstacle on column 12: {cells:?}"
    );
    assert!(b.hi_done, "the front should have stopped at the obstacle");
}

/// Same case for the `lo` front (which already swept the interval, but is
/// verified so it does not break in the future).
#[test]
fn lo_front_does_not_cross_an_obstacle_with_a_large_step() {
    let mut g = Grid::new(40, 20);
    g.set(8, 10, Cell::Solid);

    let mut b = WallBuilder::new(WallAxis::Horizontal, (18, 10), 30.0, false);
    b.advance(&g, 0.5);

    let cells = b.cells();
    assert!(
        !cells.iter().any(|&(x, y)| x <= 8 && y == 10),
        "the lo front crossed the obstacle on column 8: {cells:?}"
    );
    assert!(b.lo_done, "the lo front should have stopped");
}

/// A front must never cover a cell that is not traversable for walls.
#[test]
fn covered_cells_are_always_valid() {
    let mut g = Grid::new(30, 30);
    // A board with scattered obstacles.
    for i in 0..30 {
        if i % 7 == 0 {
            g.set(i, 15, Cell::Filled);
        }
    }

    for origin_x in [1u16, 5, 10, 16, 22, 29] {
        if !g.is_wall_open(origin_x as i64, 15) {
            continue;
        }
        let mut b = WallBuilder::new(WallAxis::Horizontal, (origin_x, 15), 28.0, false);
        for _ in 0..200 {
            b.advance(&g, 1.0 / 60.0);
            for (x, y) in b.cells() {
                assert!(
                    g.is_wall_open(x as i64, y as i64),
                    "the front from x={origin_x} covered the invalid cell ({x}, {y})"
                );
            }
            if b.is_done() {
                break;
            }
        }
    }
}

/// A wall in a fully open arena must reach both borders and cover the whole
/// line.
#[test]
fn wall_in_open_arena_covers_the_whole_line() {
    let g = Grid::new(20, 12);
    let mut b = WallBuilder::new(WallAxis::Vertical, (10, 6), 25.0, false);
    for _ in 0..600 {
        b.advance(&g, 1.0 / 60.0);
        if b.is_done() {
            break;
        }
    }
    assert!(b.is_done(), "the wall never finished growing");
    let cells = b.cells();
    assert_eq!(
        cells.len(),
        12,
        "the vertical wall should cover all 12 rows, it covered {}",
        cells.len()
    );
}

/// Minimal arena: must neither panic nor cover out of range.
#[test]
fn minimal_arena_does_not_panic() {
    let g = Grid::new(3, 3);
    let mut b = WallBuilder::new(WallAxis::Horizontal, (1, 1), 30.0, false);
    for _ in 0..120 {
        b.advance(&g, 1.0 / 60.0);
        for (x, y) in b.cells() {
            assert!(x < 3 && y < 3, "cell outside the arena: ({x}, {y})");
        }
        if b.is_done() {
            break;
        }
    }
}
