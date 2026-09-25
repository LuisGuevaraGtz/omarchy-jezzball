//! HUD (ARCHITECTURE.md §13): in Original Mode it is deliberately sparse
//! (only the essentials); in Enhanced it shows objectives, combos, power-ups and
//! time. `F` toggles a single compact line.

use macroquad::prelude::*;

use jezzball_core::level::Mode;
use jezzball_core::powerup::PowerUpKind;

use crate::render::{draw_segments, draw_text_c, fmt_time, font, hud_row, mode_name, Layout};
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

/// Power-up inventory with the active slot (each one with its key).
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

/// Active temporary effects with their remaining time.
fn draw_active_effects(app: &App, cx: f32, y: f32) {
    let t = &app.theme;
    let act: Vec<String> =
        app.state
            .powerups_active
            .iter()
            .map(|p| match p.kind {
                PowerUpKind::SlowMotion => {
                    crate::i18n::t("powerup.lento").replace("{:.0}", &format!("{:.0}", p.remaining))
                }
                PowerUpKind::Freeze => crate::i18n::t("powerup.congelado")
                    .replace("{:.0}", &format!("{:.0}", p.remaining)),
                PowerUpKind::DoubleWall => {
                    crate::i18n::t("powerup.doble").replace("{:.0}", &format!("{:.0}", p.remaining))
                }
                PowerUpKind::Shield => crate::i18n::t("powerup.escudo")
                    .replace("{:.0}", &format!("{:.0}", p.remaining)),
                PowerUpKind::RemoveBall => crate::i18n::t("powerup.quitar")
                    .replace("{:.0}", &format!("{:.0}", p.remaining)),
            })
            .collect();
    if !act.is_empty() {
        let line = act.join("   ");
        draw_text_c(&line, cx, y, font(), t.ok.to_mq(0.9));
    }
}

/// Percentage of captured area.
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

/// Remaining time if the level has a limit (speed worlds).
fn remaining_time(app: &App) -> Option<f32> {
    app.state
        .level
        .time_limit
        .map(|tl| (tl - app.state.elapsed).max(0.0))
}

/// First line of the HUD (mode + level + world).
fn level_line(app: &App) -> (String, Color) {
    let t = &app.theme;
    let n = app.level_number;
    if app.mode == Mode::Enhanced {
        let world = app.state.level.world;
        (
            crate::i18n::t("hud.nivel_mundo")
                .replacen("{}", &n.saturating_add(1).to_string(), 1)
                .replacen("{}", crate::render::world_name(world), 1),
            t.accent.to_mq(1.0),
        )
    } else {
        (
            crate::i18n::t("hud.nivel").replace("{}", &n.saturating_add(1).to_string()),
            t.accent.to_mq(1.0),
        )
    }
}

fn draw_hud_original(app: &App) {
    let t = &app.theme;
    let y = font() + 2.0 * crate::render::ui_scale();
    let dim = t.fg_dim;
    let (lv, lvc) = level_line(app);
    draw_segments(
        12.0,
        y,
        font(),
        &[
            (&lv, lvc),
            (
                &crate::i18n::t("hud.puntos_etq").replace("{}", &app.state.score.to_string()),
                dim.to_mq(0.9),
            ),
            (
                &crate::i18n::t("hud.area_etq").replace("{}", &pct(app).to_string()),
                dim.to_mq(0.9),
            ),
            (
                &crate::i18n::t("hud.vidas_etq").replace("{}", &app.state.lives.to_string()),
                t.danger.to_mq(0.9),
            ),
        ],
    );
}

fn draw_hud_enhanced(app: &App) {
    let t = &app.theme;
    let y0 = font() + 2.0 * crate::render::ui_scale();
    let dim = t.fg_dim;
    let ok = t.ok;
    let (lv, lvc) = level_line(app);
    let has_limit = remaining_time(app).is_some();
    draw_segments(
        12.0,
        y0,
        font(),
        &[
            (crate::i18n::t("hud.modo_etq"), dim.to_mq(1.0)),
            (mode_name(app.mode), dim.to_mq(0.9)),
            (&format!("  {lv}"), lvc),
            (
                &crate::i18n::t("hud.tiempo_etq").replace("{}", &fmt_time(app.state.elapsed)),
                dim.to_mq(0.9),
            ),
            (
                &crate::i18n::t("hud.restante_etq").replace(
                    "{}",
                    &remaining_time(app)
                        .map(fmt_time)
                        .unwrap_or_else(|| "--".to_string()),
                ),
                t.danger.to_mq(if has_limit { 0.95 } else { 0.35 }),
            ),
        ],
    );

    // Objectives with their state.
    let mut parts: Vec<(String, Color)> =
        vec![(crate::i18n::t("hud.objetivos").to_string(), dim.to_mq(1.0))];
    for o in &app.state.objectives {
        let done = o.done;
        let txt = format!(
            "[{}] {}",
            if done { "OK" } else { "  " },
            crate::render::objective_label(&o.objective)
        );
        parts.push((txt, if done { ok.to_mq(0.9) } else { dim.to_mq(1.0) }));
    }
    parts.push((
        crate::i18n::t("hud.estrellas_etq").replace("{}", &app.state.stars.to_string()),
        ok.to_mq(0.9),
    ));
    let refs: Vec<(&str, Color)> = parts.iter().map(|(s, c)| (s.as_str(), *c)).collect();
    draw_segments(font(), y0 + hud_row(), font(), &refs);

    // Combo and game state.
    draw_segments(
        12.0,
        y0 + 2.0 * hud_row(),
        font(),
        &[
            (
                &crate::i18n::t("hud.combo_mayus")
                    .replace("{}", &app.state.combo.multiplier.to_string()),
                t.accent.to_mq(0.95),
            ),
            (
                &crate::i18n::t("hud.max_etq")
                    .replace("{}", &app.state.max_combo_reached.to_string()),
                dim.to_mq(0.9),
            ),
            (
                &crate::i18n::t("hud.area_etq").replace("{}", &pct(app).to_string()),
                dim.to_mq(0.9),
            ),
            (
                &crate::i18n::t("hud.puntos_etq").replace("{}", &app.state.score.to_string()),
                dim.to_mq(0.9),
            ),
            (
                &crate::i18n::t("hud.vidas_etq").replace("{}", &app.state.lives.to_string()),
                t.danger.to_mq(0.9),
            ),
        ],
    );

    // Inventory and effects.
    draw_inventory(app, 12.0, y0 + 3.0 * hud_row());
    draw_active_effects(app, screen_width() * 0.5 + 40.0, y0 + 3.0 * hud_row());
}

fn draw_hud_compact(app: &App) {
    let t = &app.theme;
    let dim = t.fg_dim;
    let remaining = remaining_time(app);
    draw_segments(
        12.0,
        font() + 2.0 * crate::render::ui_scale(),
        13.0,
        &[
            (
                &format!(
                    "{} L{}",
                    mode_name(app.mode),
                    app.level_number.saturating_add(1)
                ),
                t.accent.to_mq(0.9),
            ),
            (&format!("P {}", app.state.score), dim.to_mq(0.9)),
            (&format!("A {}%", pct(app)), dim.to_mq(0.9)),
            (&format!("V {}", app.state.lives), t.danger.to_mq(0.9)),
            (
                &format!("T {}", fmt_time(app.state.elapsed)),
                dim.to_mq(0.9),
            ),
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
