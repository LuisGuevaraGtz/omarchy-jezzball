//! Arena render: grid, cells, balls, walls under construction,
//! power-ups on the floor and the preview of the active axis (ghost line).

use macroquad::prelude::*;

use crate::render::font;
use jezzball_core::grid::Cell;
use jezzball_core::powerup::PowerUpKind;
use jezzball_core::wall::WallAxis;

use crate::render::{draw_text_c, Layout};
use crate::screens::App;

/// Strength with which the axis preview is shown while a wall is
/// growing (it is dimmed so it is not confused with the real wall).
const GHOST_LIVE: f32 = 0.16;
const GHOST_BUSY: f32 = 0.06;

pub fn draw_arena(app: &App, layout: &Layout) {
    draw_overlays(app, layout);
    draw_cells(app, layout);
    draw_grid(app, layout);
    draw_pickups(app, layout);
    draw_builders(app, layout);
    draw_balls(app, layout);
    draw_ghost(app, layout);
}

/// Overlays of temporary power-ups (SlowMotion/Freeze) over the whole arena.
fn draw_overlays(app: &App, layout: &Layout) {
    let s = &app.state;
    let any_slow = s
        .powerups_active
        .iter()
        .any(|p| p.kind == PowerUpKind::SlowMotion);
    let any_freeze = s
        .powerups_active
        .iter()
        .any(|p| p.kind == PowerUpKind::Freeze);
    let a_w = layout.grid_w as f32 * layout.cell;
    let a_h = layout.grid_h as f32 * layout.cell;
    if any_slow {
        draw_rectangle(
            layout.arena_x,
            layout.arena_y,
            a_w,
            a_h,
            app.theme.fg_dim.to_mq(0.05),
        );
        draw_text_c(
            crate::i18n::t("hud.overlay_lento"),
            layout.arena_x + a_w / 2.0,
            layout.arena_y + 8.0,
            font(),
            app.theme.fg_dim.to_mq(0.9),
        );
    }
    if any_freeze {
        draw_rectangle(
            layout.arena_x,
            layout.arena_y,
            a_w,
            a_h,
            app.theme.ball.to_mq(0.06),
        );
        draw_text_c(
            crate::i18n::t("hud.overlay_congelado"),
            layout.arena_x + a_w / 2.0,
            layout.arena_y + 8.0,
            font(),
            app.theme.ball.to_mq(0.9),
        );
    }
}

/// Special cells: consolidated walls (Filled), obstacles (Solid) and
/// `NoSplit` zones. The rest is background.
fn draw_cells(app: &App, layout: &Layout) {
    let t = &app.theme;
    let g = &app.state.arena.grid;
    let c = layout.cell;
    for cy in 0..g.h {
        for cx in 0..g.w {
            let cell = g.get(cx, cy);
            let px = layout.arena_x + cx as f32 * c;
            let py = layout.arena_y + cy as f32 * c;
            match cell {
                Cell::Filled => {
                    draw_rectangle(px, py, c, c, t.wall.to_mq(0.7));
                }
                Cell::Solid => {
                    draw_rectangle(px, py, c, c, t.bg_panel.to_mq(1.0));
                    let inset = c * 0.28;
                    draw_rectangle(
                        px + c / 2.0 - inset / 2.0,
                        py + c / 2.0 - inset / 2.0,
                        inset,
                        inset,
                        t.fg_dim.to_mq(0.5),
                    );
                }
                Cell::Open => {
                    if g.is_no_split(cx, cy) {
                        draw_rectangle(px, py, c, c, t.bg_panel.to_mq(0.45));
                        let d = c * 0.16;
                        draw_circle(px + c / 2.0, py + c / 2.0, d, t.danger.to_mq(0.55));
                    }
                }
            }
        }
    }
}

/// Thin grid lines + arena border.
fn draw_grid(app: &App, layout: &Layout) {
    let t = &app.theme;
    let c = layout.cell;
    let w = layout.grid_w as f32 * c;
    let h = layout.grid_h as f32 * c;
    let line = t.fg_dim.to_mq(0.07);
    for x in 0..=layout.grid_w {
        let px = layout.arena_x + x as f32 * c;
        draw_line(px, layout.arena_y, px, layout.arena_y + h, 1.0, line);
    }
    for y in 0..=layout.grid_h {
        let py = layout.arena_y + y as f32 * c;
        draw_line(layout.arena_x, py, layout.arena_x + w, py, 1.0, line);
    }
    draw_rectangle_lines(
        layout.arena_x,
        layout.arena_y,
        w,
        h,
        1.0,
        t.fg_dim.to_mq(0.5),
    );
}

/// Power-ups waiting on the floor.
fn draw_pickups(app: &App, layout: &Layout) {
    let t = &app.theme;
    let c = layout.cell;
    if c < 12.0 {
        return;
    }
    for p in &app.state.pickups {
        let (px, py) = layout.cell_to_px(p.cell.0 as f32, p.cell.1 as f32);
        let box_s = c * 0.55;
        draw_rectangle(
            px - box_s / 2.0,
            py - box_s / 2.0,
            box_s,
            box_s,
            t.accent.to_mq(0.3),
        );
        draw_rectangle_lines(
            px - box_s / 2.0,
            py - box_s / 2.0,
            box_s,
            box_s,
            1.0,
            t.accent.to_mq(0.8),
        );
        let letter = match p.kind {
            PowerUpKind::SlowMotion => "S",
            PowerUpKind::Freeze => "F",
            PowerUpKind::DoubleWall => "D",
            PowerUpKind::Shield => "E",
            PowerUpKind::RemoveBall => "R",
        };
        draw_text_c(
            letter,
            px,
            py + 4.0 * crate::render::ui_scale(),
            font(),
            t.accent.to_mq(0.9),
        );
    }
}

/// Balls: the Normal one uses `ball`, the rest use `ball_special`.
fn draw_balls(app: &App, layout: &Layout) {
    let t = &app.theme;
    let c = layout.cell;
    for b in &app.state.balls {
        let (px, py) = layout.cell_to_px(b.pos.x, b.pos.y);
        let r = (b.radius * c).max(2.0);
        let color = if b.kind == jezzball_core::level::BallKind::Normal {
            t.ball
        } else {
            t.ball_special
        };
        draw_circle(px, py, r, color.to_mq(0.95));
        draw_circle_lines(px, py, r, 1.0, t.fg_dim.to_mq(0.6));
    }
}

/// Walls under construction.
///
/// The two halves are distinguished because they are NOT equivalent any more (rule of
/// the original JezzBall): the one that has touched a wall is sealed and immune, so
/// it is drawn as a normal wall; the one that keeps growing is painted in
/// `wall_building`, the colour that "stands out" because it is the only one that costs lives.
/// Painting them the same would mislead the player about where the risk is.
fn draw_builders(app: &App, layout: &Layout) {
    let t = &app.theme;
    let c = layout.cell;
    for b in &app.state.builders {
        // Half already anchored: consolidated wall colour.
        let sealed_cells: Vec<(u16, u16)> = {
            let mut v = Vec::new();
            if b.lo_sealed {
                v.extend(b.lo_cells());
            }
            if b.hi_sealed {
                v.extend(b.hi_cells());
            }
            v
        };
        for (cx, cy) in sealed_cells {
            draw_rectangle(
                layout.arena_x + cx as f32 * c,
                layout.arena_y + cy as f32 * c,
                c,
                c,
                t.wall.to_mq(0.95),
            );
        }
        // Live half: at risk.
        for (cx, cy) in b.vulnerable_cells(&app.state.arena.grid) {
            draw_rectangle(
                layout.arena_x + cx as f32 * c,
                layout.arena_y + cy as f32 * c,
                c,
                c,
                t.wall_building.to_mq(0.95),
            );
        }
        let (ox, oy) = layout.cell_to_px(b.origin.0 as f32, b.origin.1 as f32);
        draw_circle_lines(ox, oy, c * 0.4, 1.5, t.bg.to_mq(0.9));
        if b.shielded {
            draw_circle_lines(ox, oy, c * 0.45, 1.5, t.accent.to_mq(0.95));
        }
    }
}

/// Preview of the active axis under the cursor (requirement: always visible
/// next to the mouse). The left-button axis is shown strongly; the right-button
/// one (the opposite) is only hinted at, more faintly.
fn draw_ghost(app: &App, layout: &Layout) {
    let t = &app.theme;
    let c = layout.cell;
    let busy = !app.state.builders.is_empty();
    let alpha = if busy { GHOST_BUSY } else { GHOST_LIVE };
    let Some((hx, hy)) = layout.hovered_cell(app.mouse_pos.0, app.mouse_pos.1) else {
        return;
    };
    let axis = app.state.current_axis;
    let opp = match axis {
        WallAxis::Horizontal => WallAxis::Vertical,
        WallAxis::Vertical => WallAxis::Horizontal,
    };
    draw_axis_ghost(app, layout, axis, alpha);
    draw_axis_ghost(app, layout, opp, alpha * 0.35);

    // Mark over the cell under the cursor.
    let px = layout.arena_x + hx as f32 * c;
    let py = layout.arena_y + hy as f32 * c;
    draw_rectangle(px, py, c, c, t.accent.to_mq(0.12));
    let (mx, my) = layout.cell_to_px(hx as f32, hy as f32);
    let letter = match opp {
        WallAxis::Horizontal => "H",
        WallAxis::Vertical => "V",
    };
    draw_text_c(
        letter,
        mx,
        my + 5.0 * crate::render::ui_scale(),
        font(),
        t.accent.to_mq(0.9),
    );
}

/// Ghost line along the whole axis through the given row/column.
fn draw_axis_ghost(app: &App, layout: &Layout, axis: WallAxis, alpha: f32) {
    let t = &app.theme;
    let c = layout.cell;
    let w = layout.grid_w as f32 * c;
    let h = layout.grid_h as f32 * c;
    let Some((hx, hy)) = layout.hovered_cell(app.mouse_pos.0, app.mouse_pos.1) else {
        return;
    };
    match axis {
        WallAxis::Horizontal => {
            let y = layout.arena_y + (hy as f32 + 0.5) * c;
            draw_line(
                layout.arena_x,
                y,
                layout.arena_x + w,
                y,
                1.5,
                t.accent.to_mq(alpha),
            );
        }
        WallAxis::Vertical => {
            let x = layout.arena_x + (hx as f32 + 0.5) * c;
            draw_line(
                x,
                layout.arena_y,
                x,
                layout.arena_y + h,
                1.5,
                t.accent.to_mq(alpha),
            );
        }
    }
}
