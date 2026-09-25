//! Render de los menús, selector de niveles, resultados, fin de modo,
//! pausa, aviso y diálogo de salida.

use macroquad::prelude::*;

use crate::render::{draw_text_c, fmt_time, font, mode_name, panel, text_w, world_name};
use crate::screens::{App, Screen};

/// Marca de Omarchy dibujada con blocks, derivada del icono oficial
/// (`/usr/share/omarchy/icon.txt`): un marco con una muesca abajo y dos
/// huecos arriba. Se dibuja con rectángulos, sin depender de ningún fichero
/// del sistema, para que el juego siga siendo autónomo.
///
/// Encaja con la estética del juego por sí sola: el logo de Omarchy YA es una
/// rejilla de celdas rellenas, que es exactamente lo que el jugador construye
/// al cerrar área.
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

/// Dibuja la marca de Omarchy centrada en `cx`, con celdas de `cell` px.
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

/// Título y cabecera comunes.
fn title(app: &App, text: &str) -> f32 {
    let t = &app.theme;
    let w = screen_width();
    let s = crate::render::ui_scale();

    // Marca de Omarchy junto al título: pequeña, tenue, a la izquierda.
    draw_omarchy_mark(
        w / 2.0 - text_w(text, 40.0 * s) / 2.0 - 22.0 * s,
        62.0 * s,
        2.0 * s,
        t.accent.to_mq(0.55),
    );

    draw_text_c(text, w / 2.0, 90.0 * s, 40.0 * s, t.accent.to_mq(0.9));
    // El subtitulo sale del catalogo de language. Antes decia
    // "... {modo} : {tema}  >" con el tema literal "corriente" (traduccion
    // palabra por palabra de "current"), que no significa nada en espanol.
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
    // El pie tenia 12 px fijos de base: con el juego a pantalla completa era
    // practicamente ilegible. Va a la misma base que el resto del HUD.
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

/// Pantalla de ayuda paginada: reglas base, puntuacion y catalogo de
/// obstaculos / bolas / power-ups de Enhanced.
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
            // Subtitulo: acentuado y con algo de aire por encima.
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

/// Lista de opciones centrada a partir de `y`.
/// Fila de lista y posición donde empieza: compartidas por el render y por la
/// lógica de desplazamiento, para que ambos cuenten lo mismo.
pub fn row_height() -> f32 {
    30.0 * crate::render::ui_scale()
}

/// Y donde arranca la lista de opciones. Compartida por todas las pantallas
/// de menú: si una la scale y otra no, los texts se solapan (ocurrió).
pub fn list_y() -> f32 {
    170.0 * crate::render::ui_scale()
}

/// Cuántas filas de lista caben en pantalla. Única fuente de verdad: si el
/// render y el scroll usaran cuentas distintas, la selección se saldría de la
/// ventana visible.
pub fn visible_rows() -> usize {
    crate::screens::visible_rows_for(screen_height(), crate::render::ui_scale())
}

/// Lista de opciones con ventana de desplazamiento.
///
/// Con 61 niveles la lista no cabe en pantalla: antes se dibujaban todos desde
/// el first, así que al bajar más allá del borde el jugador seguía viendo el
/// principio y no sabía qué tenía seleccionado. Ahora se dibuja sólo la
/// ventana visible y se acompaña la selección.
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

    // Indicadores de que hay más lista fuera de la ventana.
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
    // Progreso persistido antes de la lista.
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
        // La `y` de la lista DEBE escalar igual que el título: con la ventana
        // grande, un 170.0 sin escalar dejaba las opciones por encima del
        // subtítulo y los texts se pisaban unos a otros.
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

    // Estrellas (1 por objetivo, max 3). '*' llena, '.' vacia.
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

/// Overlay de pausa sobre la arena (el juego REAL está a medias).
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
        crate::i18n::t("dialogo.salir"),
        w / 2.0,
        h / 2.0 - 10.0,
        22.0,
        t.danger.to_mq(1.0),
    );
    footer(app, crate::i18n::t("dialogo.salir_pie"));
}
