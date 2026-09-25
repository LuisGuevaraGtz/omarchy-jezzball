//! Render de la arena: rejilla, celdas, bolas, muros en construcción,
//! power-ups del suelo y la previsualización del eje activo (línea fantasma).

use macroquad::prelude::*;

use jezzball_core::grid::Cell;
use jezzball_core::powerup::PowerUpKind;
use jezzball_core::wall::WallAxis;

use crate::render::{draw_text_c, Layout};
use crate::screens::App;

/// Fuerza con la que se ve la previsualización del eje mientras hay un muro
/// creciendo (se atenúa para no confundir con el muro real).
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

/// Overlays de power-ups temporales (SlowMotion/Freeze) sobre toda la arena.
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
            "LENTO",
            layout.arena_x + a_w / 2.0,
            layout.arena_y + 8.0,
            12.0,
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
            "CONGELADO",
            layout.arena_x + a_w / 2.0,
            layout.arena_y + 8.0,
            12.0,
            app.theme.ball.to_mq(0.9),
        );
    }
}

/// Celdas especiales: muros consolidados (Filled), obstáculos (Solid) y
/// zonas `NoSplit`. El resto es fondo.
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

/// Líneas de rejilla finas + borde de la arena.
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

/// Power-ups esperando en el suelo.
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
            12.0 * crate::render::ui_scale(),
            t.accent.to_mq(0.9),
        );
    }
}

/// Bolas: la Normal usa `ball`, el resto `ball_special`.
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

/// Muros en construcción: celdas del segmento en `wall_building` (el color
/// que "canta" porque es el momento de riesgo).
fn draw_builders(app: &App, layout: &Layout) {
    let t = &app.theme;
    let c = layout.cell;
    for b in &app.state.builders {
        for (cx, cy) in b.cells() {
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

/// Previsualización del eje activo bajo el cursor (requisito: siempre visible
/// junto al ratón). El eje del botón izquierdo se ve fuerte; el del derecho
/// (contrario) se insinúa más tenue.
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

    // Marca sobre la celda bajo el cursor.
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
        13.0 * crate::render::ui_scale(),
        t.accent.to_mq(0.9),
    );
}

/// Línea fantasma a lo largo de todo el eje por la fila/columna indicada.
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
