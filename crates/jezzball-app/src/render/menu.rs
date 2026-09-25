//! Render of the menus, level selector, results, end of mode,
//! pause, floater and exit dialog.

use macroquad::prelude::*;

use crate::render::{draw_text_c, fmt_time, font, mode_name, panel, text_w, world_name};
use crate::screens::{App, Screen};

/// Omarchy mark drawn with blocks, derived from the official icon
/// (`/usr/share/omarchy/icon.txt`): a frame with a notch at the bottom and two
/// gaps at the top. It is drawn with rectangles, without depending on any system
/// file, so that the game remains self-contained.
///
/// It fits the game's aesthetic all by itself: the Omarchy logo IS ALREADY a
/// grid of filled cells, which is exactly what the player builds
/// when closing off area.
const OMARCHY_MARK: [&str; 13] = [
    "##############",
    "#......#.....#",
    "#.######..##.#",
    "#.#........#.#",
    "#.#........#.#",
    "#.#........#.#",
    "###........#.#",
    "#.#........#.#",
    "#.#........#.#",
    "#.#........#.#",
    "#.##########.#",
    "#......#.....#",
    "########.#####",
];

/// Draws the Omarchy mark centred at `cx`, with cells of `cell` px.
pub fn draw_omarchy_mark(cx: f32, top: f32, cell: f32, color: Color) {
    let w = OMARCHY_MARK[0].len() as f32 * cell;
    let x0 = cx - w / 2.0;
    for (row, line) in OMARCHY_MARK.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            if ch == '#' {
                draw_rectangle(
                    x0 + col as f32 * cell,
                    top + row as f32 * cell,
                    cell,
                    cell,
                    color,
                );
            }
        }
    }
}

/// Common title and header.
fn title(app: &App, text: &str) -> f32 {
    let t = &app.theme;
    let w = screen_width();
    let s = crate::render::ui_scale();

    // Omarchy mark next to the title: small, faint, on the left.
    draw_omarchy_mark(
        w / 2.0 - text_w(text, 40.0 * s) / 2.0 - 22.0 * s,
        62.0 * s,
        2.0 * s,
        t.accent.to_mq(0.55),
    );

    draw_text_c(text, w / 2.0, 90.0 * s, 40.0 * s, t.accent.to_mq(0.9));
    // The subtitle comes from the language catalog. It used to say
    // "... {modo} : {tema}  >" with the literal theme "corriente" (a word-for-word
    // translation of "current"), which means nothing in Spanish.
    let sub = if app.mode == jezzball_core::level::Mode::Enhanced {
        crate::i18n::t("menu.subtitulo_modo")
            .replacen("{}", mode_name(app.mode), 1)
            .replacen("{}", &t.name, 1)
    } else {
        crate::i18n::t("menu.subtitulo").replacen("{}", &t.name, 1)
    };
    draw_text_c(&sub, w / 2.0, 118.0 * s, font(), t.fg_dim.to_mq(1.0));
    150.0 * s
}

fn footer(app: &App, hint: &str) {
    let t = &app.theme;
    let w = screen_width();
    let h = screen_height();
    // The footer had a fixed 12 px base: with the game full screen it was
    // practically unreadable. It now uses the same base as the rest of the HUD.
    let sz = font();
    let x = ((w - text_w(hint, sz)) / 2.0).max(4.0);
    draw_text(
        hint,
        x,
        h - 24.0 * crate::render::ui_scale(),
        sz,
        t.fg_dim.to_mq(1.0),
    );
}

/// Paginated help screen: base rules, scoring and the catalog of
/// Enhanced obstacles / balls / power-ups.
pub fn draw_help(app: &App, w: f32, h: f32) {
    let t = &app.theme;
    let s = crate::render::ui_scale();
    let (title, lines) = crate::screens::help_page(app.help_page);

    let pw = (w - 80.0 * s).min(760.0 * s);
    let ph = h - 150.0 * s;
    let px = (w - pw) / 2.0;
    let py = 96.0 * s;
    panel(t, px, py, pw, ph);

    draw_text_c(
        title,
        w / 2.0,
        py + 34.0 * s,
        font() + 8.0 * s,
        t.accent.to_mq(1.0),
    );

    let sz = font();
    let row = sz * 1.5;
    let mut y = py + 68.0 * s;
    let x = px + 28.0 * s;
    for line in lines {
        if let Some(sub) = line.strip_prefix("# ") {
            // Subheading: accented and with a bit of air above it.
            y += row * 0.35;
            draw_text(sub, x, y, sz, t.accent.to_mq(0.95));
        } else {
            draw_text(line, x, y, sz, t.fg.to_mq(0.9));
        }
        y += row;
    }

    draw_text_c(
        &format!(
            "pagina {} de {}",
            app.help_page + 1,
            crate::screens::HELP_PAGES
        ),
        w / 2.0,
        py + ph - 16.0 * s,
        sz,
        t.fg_dim.to_mq(1.0),
    );
    footer(app, crate::i18n::t("pie.ayuda"));
}

/// Option list centred starting from `y`.
/// List row and the position where it starts: shared by the render and by the
/// scrolling logic, so that both count the same.
pub fn row_height() -> f32 {
    30.0 * crate::render::ui_scale()
}

/// The `y` where the option list starts. Shared by all menu
/// screens: if one scales it and another does not, the texts overlap (it happened).
pub fn list_y() -> f32 {
    170.0 * crate::render::ui_scale()
}

/// How many list rows fit on the screen. Single source of truth: if the
/// render and the scroll used different counts, the selection would fall outside the
/// visible window.
pub fn visible_rows() -> usize {
    crate::screens::visible_rows_for(screen_height(), crate::render::ui_scale())
}

/// Option list with a scroll window.
///
/// With 61 levels the list does not fit on the screen: previously all of them were drawn from
/// the first one, so when scrolling past the edge the player kept seeing the
/// beginning and did not know what was selected. Now only the
/// visible window is drawn and it follows the selection.
fn option_list(app: &App, y: f32) -> f32 {
    let t = &app.theme;
    let w = screen_width();
    let s = crate::render::ui_scale();
    let row = row_height();
    let sz = font() + 2.0 * s;

    let n = app.menu_items.len();
    let visible = visible_rows().min(n.max(1));
    let top = app.list_top.min(n.saturating_sub(visible));

    let mut ny = y;
    for (idx, item) in app.menu_items.iter().enumerate().skip(top).take(visible) {
        let sel = app.select == idx;
        let color = if !item.enabled {
            t.fg_dim
        } else if sel {
            t.accent
        } else {
            t.fg
        };
        let label = if item.enabled {
            item.label.clone()
        } else {
            format!("{}  (BLOQUEADO)", item.label)
        };
        if sel && item.enabled {
            draw_text_c(&format!("> {}", label), w / 2.0, ny, sz, color.to_mq(0.95));
        } else {
            draw_text_c(&label, w / 2.0, ny, sz, color.to_mq(0.75));
        }
        ny += row;
    }

    // Indicators that there is more list outside the window.
    if top > 0 {
        draw_text_c("^", w / 2.0, y - row * 0.6, sz, t.fg_dim.to_mq(1.0));
    }
    if top + visible < n {
        draw_text_c(
            &crate::i18n::t("menu.mas_abajo").replace("{}", &(n - (top + visible)).to_string()),
            w / 2.0,
            ny + row * 0.1,
            sz,
            t.fg_dim.to_mq(1.0),
        );
    }
    ny + 24.0
}

pub fn draw_menu(app: &App, w: f32, _h: f32) {
    title(app, crate::i18n::t("menu.title"));
    // Persisted progress before the list.
    let (done_o, stars_o) =
        crate::screens::mode_progress(app, jezzball_core::level::Mode::Original);
    let (done_e, stars_e) =
        crate::screens::mode_progress(app, jezzball_core::level::Mode::Enhanced);
    let progress = crate::i18n::t("menu.progreso")
        .replacen("{}", &done_o.to_string(), 1)
        .replacen(
            "{}",
            &app.levels
                .len(jezzball_core::level::Mode::Original)
                .to_string(),
            1,
        )
        .replacen("{}", &stars_o.to_string(), 1)
        .replacen("{}", &done_e.to_string(), 1)
        .replacen(
            "{}",
            &app.levels
                .len(jezzball_core::level::Mode::Enhanced)
                .to_string(),
            1,
        )
        .replacen("{}", &stars_e.to_string(), 1);
    let y = option_list(app, list_y());
    draw_text_c(
        &progress,
        w / 2.0,
        y + 10.0 * crate::render::ui_scale(),
        font(),
        app.theme.ok.to_mq(1.0),
    );
    footer(app, crate::i18n::t("pie.menu"));
    let _ = app.screen;
}

pub fn draw_level_select(app: &App, _w: f32, _h: f32) {
    let t = &app.theme;
    if app.screen == Screen::ModeMenu {
        title(app, crate::i18n::t("menu.elegir_nivel"));
        // The list's `y` MUST scale the same as the title: with a large
        // window, an unscaled 170.0 left the options above the
        // subtitle and the texts trampled over each other.
        option_list(app, list_y());
        footer(app, crate::i18n::t("pie.menu"));
        return;
    }
    title(
        app,
        &crate::i18n::t("menu.elegir_nivel_modo").replace("{}", mode_name(app.selector_mode)),
    );
    let y = option_list(app, list_y());
    let _ = y;
    footer(app, crate::i18n::t("pie.selector"));
    let _ = t;
}

pub fn draw_results(app: &App, w: f32, h: f32) {
    let t = &app.theme;
    let r = &app.results;
    let pw = 480.0;
    let ph = 300.0;
    let px = (w - pw) / 2.0;
    let py = (h - ph) / 2.0 - 20.0;
    panel(t, px, py, pw, ph);

    let head = if r.won {
        crate::i18n::t("resultados.completado").replace("{}", &r.level.to_string())
    } else {
        crate::i18n::t("resultados.derrota").replace("{}", &r.level.to_string())
    };
    draw_text_c(&head, w / 2.0, py + 44.0, 26.0, t.accent.to_mq(1.0));

    // Stars (1 per objective, max 3). '*' filled, '.' empty.
    let stars: String = (0..3)
        .map(|i| if i < r.stars { '*' } else { '.' })
        .collect();
    let prev_stars: String = (0..3)
        .map(|i| if i < r.prev_stars { '*' } else { '.' })
        .collect();
    draw_text_c(&stars, w / 2.0, py + 86.0, 26.0, t.ok.to_mq(1.0));
    draw_text_c(
        &crate::i18n::t("resultados.antes").replace("{}", &prev_stars),
        w / 2.0,
        py + 108.0,
        13.0,
        t.fg_dim.to_mq(1.0),
    );

    let mut lines: Vec<String> = Vec::new();
    lines.push(crate::i18n::t("resultados.puntos").replace("{}", &r.score.to_string()));
    if r.new_best {
        lines.push(
            crate::i18n::t("resultados.nuevo_record").replace("{}", &r.prev_best.to_string()),
        );
    } else {
        lines.push(crate::i18n::t("resultados.record").replace("{}", &r.prev_best.to_string()));
    }
    lines.push(crate::i18n::t("resultados.tiempo").replace("{}", &fmt_time(r.time)));
    if let Some(wrld) = r.next_world {
        lines.push(crate::i18n::t("resultados.nuevo_mundo").replace("{}", world_name(wrld)));
    }
    let mut y = py + 140.0;
    let dim = t.fg;
    for line in &lines {
        draw_text_c(line, w / 2.0, y, 15.0, dim.to_mq(0.9));
        y += 24.0;
    }

    if r.no_next {
        footer(app, crate::i18n::t("pie.resultados"));
    } else {
        footer(app, crate::i18n::t("pie.resultados_siguiente"));
    }
}

pub fn draw_mode_complete(app: &App, w: f32, h: f32) {
    let t = &app.theme;
    let (done, stars) = crate::screens::mode_progress(app, app.mode);
    let total = app.levels.len(app.mode);
    draw_text_c(
        &crate::i18n::t("resultados.modo_completado").replace("{}", mode_name(app.mode)),
        w / 2.0,
        h / 2.0 - 60.0,
        32.0,
        t.accent.to_mq(1.0),
    );
    draw_text_c(
        &crate::i18n::t("resultados.resumen_modo")
            .replacen("{}", &done.to_string(), 1)
            .replacen("{}", &total.to_string(), 1)
            .replacen("{}", &stars.to_string(), 1),
        w / 2.0,
        h / 2.0 - 20.0,
        16.0,
        t.ok.to_mq(0.95),
    );
    draw_text_c(
        crate::i18n::t("resultados.rejugar"),
        w / 2.0,
        h / 2.0 + 16.0,
        13.0,
        t.fg_dim.to_mq(1.0),
    );
    footer(app, crate::i18n::t("pie.enter_menu"));
}

pub fn draw_error(app: &App, msg: &str, w: f32, h: f32) {
    let t = &app.theme;
    let lines: Vec<&str> = msg.lines().collect();
    let line_h = 18.0;
    let body_h = lines.len() as f32 * line_h;
    let pw = 740.0;
    let ph = (body_h + 104.0).max(140.0);
    let px = (w - pw) / 2.0;
    let py = ((h - ph) / 2.0).max(8.0);
    panel(t, px, py, pw, ph);
    draw_text_c(
        crate::i18n::t("error.title"),
        w / 2.0,
        py + 30.0,
        24.0,
        t.danger.to_mq(1.0),
    );
    let mut y = py + 62.0;
    for (i, line) in lines.iter().enumerate() {
        let (size, color) = if i == 0 {
            (15.0, t.fg.to_mq(0.95))
        } else {
            (12.0, t.fg_dim.to_mq(0.85))
        };
        draw_text_c(line, w / 2.0, y, size, color);
        y += line_h;
    }
    footer(app, crate::i18n::t("pie.error"));
}

/// Pause overlay over the arena (the REAL game is half-finished).
pub fn draw_pause_overlay(app: &App, layout: &crate::render::Layout) {
    let t = &app.theme;
    let w = screen_width();
    let h = screen_height();
    let _ = layout;
    draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.55));
    draw_text_c(
        crate::i18n::t("hud.pausa"),
        w / 2.0,
        h / 2.0 - 30.0,
        32.0,
        t.accent.to_mq(1.0),
    );
    draw_text_c(
        crate::i18n::t("pie.pausa"),
        w / 2.0,
        h / 2.0 + 12.0,
        15.0,
        t.fg.to_mq(0.95),
    );
}

/// Brief floater centred near the bottom.
pub fn draw_toast(app: &App, toast: &crate::screens::Toast, w: f32) {
    let t = &app.theme;
    let y = screen_height() - 70.0;
    let tw = text_w(&toast.text, 15.0) + 24.0;
    draw_rectangle(
        w / 2.0 - tw / 2.0,
        y - 22.0,
        tw,
        30.0,
        t.bg_panel.to_mq(0.95),
    );
    draw_text_c(&toast.text, w / 2.0, y, 15.0, t.accent.to_mq(0.95));
}

/// Exit confirmation dialog.
pub fn draw_quit_confirm(app: &App, w: f32, h: f32) {
    let t = &app.theme;
    draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
    let pw = 460.0;
    let ph = 120.0;
    panel(t, (w - pw) / 2.0, (h - ph) / 2.0, pw, ph);
    draw_text_c(
        crate::i18n::t("dialogo.salir"),
        w / 2.0,
        h / 2.0 - 10.0,
        22.0,
        t.danger.to_mq(1.0),
    );
    footer(app, crate::i18n::t("dialogo.salir_pie"));
}
