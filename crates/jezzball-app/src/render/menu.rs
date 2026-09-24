//! Render de los menús, selector de niveles, resultados, fin de modo,
//! pausa, aviso y diálogo de salida.

use macroquad::prelude::*;

use crate::render::{draw_text_c, fmt_time, mode_name, panel, text_w, world_name, FONT};
use crate::screens::{App, Screen};

/// Título y cabecera comunes.
fn title(app: &App, text: &str) -> f32 {
    let t = &app.theme;
    let w = screen_width();
    draw_text_c(text, w / 2.0, 90.0, 40.0, t.accent.to_mq(0.9));
    let sub = if app.mode == jezzball_core::level::Mode::Enhanced {
        format!(
            "JezzBall para Omarchy  -  {} : {}  >",
            mode_name(app.mode),
            t.name
        )
    } else {
        format!("JezzBall para Omarchy  -  {}", t.name)
    };
    draw_text_c(&sub, w / 2.0, 118.0, 13.0, t.fg_dim.to_mq(0.7));
    150.0
}

fn footer(app: &App, hint: &str) {
    let t = &app.theme;
    let w = screen_width();
    let h = screen_height();
    let x = ((w - text_w(hint, 12.0)) / 2.0).max(4.0);
    draw_text(hint, x, h - 24.0, 12.0, t.fg_dim.to_mq(0.8));
}

/// Lista de opciones centrada a partir de `y`.
fn option_list(app: &App, y: f32) -> f32 {
    let t = &app.theme;
    let w = screen_width();
    let mut ny = y;
    for (idx, item) in app.menu_items.iter().enumerate() {
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
            draw_text_c(
                &format!("> {}", label),
                w / 2.0,
                ny,
                FONT + 2.0,
                color.to_mq(0.95),
            );
        } else {
            draw_text_c(&label, w / 2.0, ny, FONT + 2.0, color.to_mq(0.75));
        }
        ny += 30.0;
    }
    ny + 24.0
}

pub fn draw_menu(app: &App, w: f32, _h: f32) {
    title(app, "OMARCHY-JEZZBALL");
    // Progreso persistido antes de la lista.
    let (done_o, stars_o) =
        crate::screens::mode_progress(app, jezzball_core::level::Mode::Original);
    let (done_e, stars_e) =
        crate::screens::mode_progress(app, jezzball_core::level::Mode::Enhanced);
    let progress = format!(
        "Original {}/{} *{}     Enhanced {}/{} *{}",
        done_o,
        app.levels.len(jezzball_core::level::Mode::Original),
        stars_o,
        done_e,
        app.levels.len(jezzball_core::level::Mode::Enhanced),
        stars_e
    );
    let y = option_list(app, 170.0);
    draw_text_c(&progress, w / 2.0, y + 10.0, 13.0, app.theme.ok.to_mq(0.9));
    footer(
        app,
        "flechas/k/j mover   enter elegir   esc salir   modo original -> 1o nivel",
    );
    let _ = app.screen;
}

pub fn draw_level_select(app: &App, _w: f32, _h: f32) {
    let t = &app.theme;
    if app.screen == Screen::ModeMenu {
        title(app, "ELEGIR NIVEL");
        option_list(app, 170.0);
        footer(app, "flechas mover   enter elegir   esc atras");
        return;
    }
    title(
        app,
        &format!("ELEGIR NIVEL - {}", mode_name(app.selector_mode)),
    );
    let y = option_list(app, 170.0);
    let _ = y;
    footer(app, "flechas mover   enter jugar   esc atras");
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
        format!("NIVEL {}  COMPLETADO", r.level)
    } else {
        format!("NIVEL {}  DERROTA", r.level)
    };
    draw_text_c(&head, w / 2.0, py + 44.0, 26.0, t.accent.to_mq(1.0));

    // Estrellas (1 por objetivo, max 3). '*' llena, '.' vacia.
    let stars: String = (0..3)
        .map(|i| if i < r.stars { '*' } else { '.' })
        .collect();
    let prev_stars: String = (0..3)
        .map(|i| if i < r.prev_stars { '*' } else { '.' })
        .collect();
    draw_text_c(&stars, w / 2.0, py + 86.0, 26.0, t.ok.to_mq(1.0));
    draw_text_c(
        &format!("antes: {}", prev_stars),
        w / 2.0,
        py + 108.0,
        13.0,
        t.fg_dim.to_mq(0.8),
    );

    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("Puntos: {}", r.score));
    if r.new_best {
        lines.push(format!("NUEVO RECORD  (record: {})", r.prev_best));
    } else {
        lines.push(format!("Record: {}", r.prev_best));
    }
    lines.push(format!("Tiempo: {}", fmt_time(r.time)));
    if let Some(wrld) = r.next_world {
        lines.push(format!("Nuevo mundo desbloqueado: {}", world_name(wrld)));
    }
    let mut y = py + 140.0;
    let dim = t.fg;
    for line in &lines {
        draw_text_c(line, w / 2.0, y, 15.0, dim.to_mq(0.9));
        y += 24.0;
    }

    if r.no_next {
        footer(app, "enter continuar   R repetir   M menu");
    } else {
        footer(app, "enter siguiente nivel   R repetir   M menu");
    }
}

pub fn draw_mode_complete(app: &App, w: f32, h: f32) {
    let t = &app.theme;
    let (done, stars) = crate::screens::mode_progress(app, app.mode);
    let total = app.levels.len(app.mode);
    draw_text_c(
        &format!("MODO {} COMPLETADO", mode_name(app.mode)),
        w / 2.0,
        h / 2.0 - 60.0,
        32.0,
        t.accent.to_mq(1.0),
    );
    draw_text_c(
        &format!("{}/{} niveles   {} estrellas", done, total, stars),
        w / 2.0,
        h / 2.0 - 20.0,
        16.0,
        t.ok.to_mq(0.95),
    );
    draw_text_c(
        "Puedes rejugar cualquiera de los niveles desde el selector.",
        w / 2.0,
        h / 2.0 + 16.0,
        13.0,
        t.fg_dim.to_mq(0.8),
    );
    footer(app, "enter menu");
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
    draw_text_c("ERROR", w / 2.0, py + 30.0, 24.0, t.danger.to_mq(1.0));
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
    footer(app, "enter / esc para volver al menu");
}

/// Overlay de pausa sobre la arena (el juego REAL está a medias).
pub fn draw_pause_overlay(app: &App, layout: &crate::render::Layout) {
    let t = &app.theme;
    let w = screen_width();
    let h = screen_height();
    let _ = layout;
    draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.55));
    draw_text_c("PAUSA", w / 2.0, h / 2.0 - 30.0, 32.0, t.accent.to_mq(1.0));
    draw_text_c(
        "espacio / esc reanudar   R reiniciar   Q salir",
        w / 2.0,
        h / 2.0 + 12.0,
        15.0,
        t.fg.to_mq(0.95),
    );
}

/// Aviso breve centrado cerca del fondo.
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

/// Diálogo de confirmación de salida.
pub fn draw_quit_confirm(app: &App, w: f32, h: f32) {
    let t = &app.theme;
    draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
    let pw = 460.0;
    let ph = 120.0;
    panel(t, (w - pw) / 2.0, (h - ph) / 2.0, pw, ph);
    draw_text_c(
        "SALIR DEL JUEGO?",
        w / 2.0,
        h / 2.0 - 10.0,
        22.0,
        t.danger.to_mq(1.0),
    );
    footer(app, "enter confirmar   esc / Q cancelar");
}
