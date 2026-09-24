//! HUD (ARCHITECTURE.md §13): en el Modo Original es deliberadamente pobre
//! (solo lo esencial); en Enhanced muestra objetivos, combos, power-ups y
//! tiempo. `F` alterna a una sola línea compacta.

use macroquad::prelude::*;

use jezzball_core::level::Mode;
use jezzball_core::powerup::PowerUpKind;

use crate::render::{FONT, HUD_ROW, Layout, draw_segments, draw_text_c, fmt_time, mode_name};
use crate::screens::App;

pub fn draw_hud(app: &App, layout: &Layout) {
    if app.hud_compact {
        draw_hud_compact(app);
    } else if app.mode == Mode::Original {
        draw_hud_original(app);
    } else {
        draw_hud_enhanced(app);
    }
    let _ = layout;
}

/// Inventario de power-ups con la ranura activa (cada una con su tecla).
pub fn draw_inventory(app: &App, cx: f32, y: f32) {
    let t = &app.theme;
    let inv = &app.state.inventory;
    let mut parts: Vec<(String, Color)> = Vec::new();
    for i in 0..5 {
        let slot = format!("{}:", i + 1);
        let kind = inv.get(i).copied();
        let letter = kind.map(powerup_letter).unwrap_or(".");
        let active = app.select == i;
        let has = kind.is_some();
        let color = if active && has {
            t.accent
        } else if active {
            t.fg_dim
        } else if has {
            t.fg
        } else {
            t.fg_dim
        };
        parts.push((format!("{}{} ", slot, letter), color.to_mq(0.9)));
    }
    let refs: Vec<(&str, Color)> = parts.iter().map(|(s, c)| (s.as_str(), *c)).collect();
    draw_segments(cx, y, 13.0, &refs);
}

pub fn powerup_letter(k: PowerUpKind) -> &'static str {
    match k {
        PowerUpKind::SlowMotion => "S",
        PowerUpKind::Freeze => "F",
        PowerUpKind::DoubleWall => "D",
        PowerUpKind::Shield => "E",
        PowerUpKind::RemoveBall => "R",
    }
}

/// Efectos temporales activos con su tiempo restante.
fn draw_active_effects(app: &App, cx: f32, y: f32) {
    let t = &app.theme;
    let act: Vec<String> = app
        .state
        .powerups_active
        .iter()
        .map(|p| match p.kind {
            PowerUpKind::SlowMotion => format!("LENTO {:.0}s", p.remaining),
            PowerUpKind::Freeze => format!("CONGELADO {:.0}s", p.remaining),
            PowerUpKind::DoubleWall => format!("DOBLE {:.0}s", p.remaining),
            PowerUpKind::Shield => format!("ESCUDO {:.0}s", p.remaining),
            PowerUpKind::RemoveBall => format!("QUITAR {:.0}s", p.remaining),
        })
        .collect();
    if !act.is_empty() {
        let line = act.join("   ");
        draw_text_c(&line, cx, y, 13.0, t.ok.to_mq(0.9));
    }
}

/// Porcentaje de zona capturada.
fn pct(app: &App) -> u32 {
    let g = &app.state.arena.grid;
    let mut open = 0u32;
    let mut total = 0u32;
    for y in 0..g.h {
        for x in 0..g.w {
            if g.is_open(x, y) {
                open += 1;
            }
            total += 1;
        }
    }
    (open * 100).checked_div(total).unwrap_or(0)
}

/// Tiempo restante si el nivel tiene límite (mundos velocidad).
fn remaining_time(app: &App) -> Option<f32> {
    app.state
        .level
        .time_limit
        .map(|tl| (tl - app.state.elapsed).max(0.0))
}

/// Primera línea del HUD (modo + nivel + mundo).
fn level_line(app: &App) -> (String, Color) {
    let t = &app.theme;
    let n = app.level_number;
    if app.mode == Mode::Enhanced {
        let world = app.state.level.world;
        (
            format!("NIVEL {} ({})", n + 1, crate::render::world_name(world)),
            t.accent.to_mq(1.0),
        )
    } else {
        (format!("NIVEL {}", n + 1), t.accent.to_mq(1.0))
    }
}

fn draw_hud_original(app: &App) {
    let t = &app.theme;
    let y = FONT + 2.0;
    let dim = t.fg_dim;
    let (lv, lvc) = level_line(app);
    draw_segments(
        12.0,
        y,
        FONT,
        &[
            (&lv, lvc),
            (&format!("  PUNTOS {}", app.state.score), dim.to_mq(0.9)),
            (&format!("  AREA {}%", pct(app)), dim.to_mq(0.9)),
            (&format!("  VIDAS {}", app.state.lives), t.danger.to_mq(0.9)),
        ],
    );
}

fn draw_hud_enhanced(app: &App) {
    let t = &app.theme;
    let y0 = FONT + 2.0;
    let dim = t.fg_dim;
    let ok = t.ok;
    let (lv, lvc) = level_line(app);
    let has_limit = remaining_time(app).is_some();
    draw_segments(
        12.0,
        y0,
        FONT,
        &[
            ("MODO ", dim.to_mq(0.8)),
            (mode_name(app.mode), dim.to_mq(0.9)),
            (&format!("  {lv}"), lvc),
            (&format!("  TIEMPO {}", fmt_time(app.state.elapsed)), dim.to_mq(0.9)),
            (
                &format!(
                    "  RESTANTE {}",
                    remaining_time(app).map(fmt_time).unwrap_or_else(|| "--".to_string())
                ),
                t.danger.to_mq(if has_limit { 0.95 } else { 0.35 }),
            ),
        ],
    );

    // Objetivos con su estado.
    let mut parts: Vec<(String, Color)> = vec![("OBJETIVOS ".to_string(), dim.to_mq(0.8))];
    for o in &app.state.objectives {
        let done = o.done;
        let txt = format!(
            "[{}] {}",
            if done { "OK" } else { "  " },
            crate::render::objective_label(&o.objective)
        );
        parts.push((txt, if done { ok.to_mq(0.9) } else { dim.to_mq(0.75) }));
    }
    parts.push((
        format!("  ESTRELLAS {}", app.state.stars),
        ok.to_mq(0.9),
    ));
    let refs: Vec<(&str, Color)> = parts.iter().map(|(s, c)| (s.as_str(), *c)).collect();
    draw_segments(12.0, y0 + HUD_ROW, FONT, &refs);

    // Combo y estado de partida.
    draw_segments(
        12.0,
        y0 + 2.0 * HUD_ROW,
        FONT,
        &[
            (&format!("COMBO x{}", app.state.combo.multiplier), t.accent.to_mq(0.95)),
            (&format!("  MAX x{}", app.state.max_combo_reached), dim.to_mq(0.9)),
            (&format!("  AREA {}%", pct(app)), dim.to_mq(0.9)),
            (&format!("  PUNTOS {}", app.state.score), dim.to_mq(0.9)),
            (&format!("  VIDAS {}", app.state.lives), t.danger.to_mq(0.9)),
        ],
    );

    // Inventario y efectos.
    draw_inventory(app, 12.0, y0 + 3.0 * HUD_ROW);
    draw_active_effects(app, screen_width() * 0.5 + 40.0, y0 + 3.0 * HUD_ROW);
}

fn draw_hud_compact(app: &App) {
    let t = &app.theme;
    let dim = t.fg_dim;
    let remaining = remaining_time(app);
    draw_segments(
        12.0,
        FONT + 2.0,
        13.0,
        &[
            (
                &format!("{} L{}", mode_name(app.mode), app.level_number + 1),
                t.accent.to_mq(0.9),
            ),
            (&format!("P {}", app.state.score), dim.to_mq(0.9)),
            (&format!("A {}%", pct(app)), dim.to_mq(0.9)),
            (&format!("V {}", app.state.lives), t.danger.to_mq(0.9)),
            (&format!("T {}", fmt_time(app.state.elapsed)), dim.to_mq(0.9)),
            (
                &format!(
                    "R {}",
                    remaining.map(fmt_time).unwrap_or_else(|| "--".to_string())
                ),
                dim.to_mq(0.9),
            ),
        ],
    );
}