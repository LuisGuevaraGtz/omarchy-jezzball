//! Render global (ARCHITECTURE.md §13): el render NUNCA decide lógica de
//! juego, se limita a leer `&App` / `&GameState` y dibujar.
//!
//! `Layout` calcula dónde vive la arena dentro de la ventana y se usa tanto
//! para dibujar como para el hit-testing del ratón en `screens`.

pub mod arena;
pub mod hud;
pub mod menu;

use macroquad::prelude::*;

use crate::screens::{App, Screen};

/// Tamaño base de la fuente del HUD, a la escala de referencia.
/// 16 px: a 14 px el HUD resultaba ilegible incluso ya escalado.
pub const FONT_BASE: f32 = 16.0;
/// Altura base de una fila del HUD (px).
pub const HUD_ROW_BASE: f32 = 24.0;
/// Altura base de la barra de pistas inferior.
pub const HINT_H_BASE: f32 = 20.0;

/// Escala de la interfaz.
///
/// El HUD se diseñó con `FONT_BASE` para una ventana pequeña. Con tamaños
/// fijos, en una pantalla grande o con `high_dpi` el texto queda diminuto: la
/// arena crece con la ventana pero las letras no.
///
/// La referencia es 1280x720 (no la resolución real de un monitor moderno, a
/// propósito): así una ventana de 1900x1000 —lo normal en un portátil actual—
/// da del orden de x1.5, que es donde el HUD se lee cómodo. Verificado sobre
/// capturas reales del juego a 1890x1017, donde la referencia anterior
/// (1024x768) se quedaba en x1.32 y el texto seguía siendo ilegible.
///
/// `OMARCHY_JEZZBALL_UI_SCALE` permite ajustarlo a mano (p. ej. `2.0`).
pub fn ui_scale() -> f32 {
    if let Ok(v) = std::env::var("OMARCHY_JEZZBALL_UI_SCALE") {
        if let Ok(f) = v.trim().parse::<f32>() {
            if f.is_finite() && f > 0.1 {
                return f.clamp(0.5, 4.0);
            }
        }
    }
    let w = screen_width().max(1.0);
    let h = screen_height().max(1.0);
    // Se toma la dimensión más restrictiva para no desbordar el ancho en
    // ventanas apaisadas ni el alto en las estrechas.
    let s = (w / 1280.0).min(h / 720.0);
    // Suelo en 1.15: incluso en una ventana pequeña el texto base resultaba
    // justo. Techo en 3.0 para que no invada la arena en pantallas enormes.
    s.clamp(1.15, 3.0)
}

/// Tamaño de fuente del HUD ya escalado.
pub fn font() -> f32 {
    (FONT_BASE * ui_scale()).round()
}

/// Altura de fila del HUD ya escalada.
pub fn hud_row() -> f32 {
    HUD_ROW_BASE * ui_scale()
}

/// Altura de la barra de pistas ya escalada.
pub fn hint_h() -> f32 {
    HINT_H_BASE * ui_scale()
}

/// Geometría de la arena dentro de la ventana.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub arena_x: f32,
    pub arena_y: f32,
    pub cell: f32,
    pub grid_w: u16,
    pub grid_h: u16,
}

impl Layout {
    /// Encaja la arena de `grid_w x grid_h` celdas cuadradas por debajo del
    /// HUD y por encima de la barra de pistas, centrada horizontalmente.
    pub fn compute(win_w: f32, win_h: f32, grid_w: u16, grid_h: u16, hud_rows: u32) -> Self {
        let hud_h = hud_rows.max(1) as f32 * hud_row();
        let top = hud_h + 4.0;
        let avail_w = (win_w - 24.0).max(64.0);
        let avail_h = (win_h - top - hint_h() - 12.0).max(64.0);
        let gw = grid_w.max(1) as f32;
        let gh = grid_h.max(1) as f32;
        let cell = (avail_w / gw).min(avail_h / gh).floor().max(2.0);
        let arena_w = cell * gw;
        let arena_h = cell * gh;
        let arena_x = ((win_w - arena_w) / 2.0).floor();
        let arena_y = (top + (avail_h - arena_h).max(0.0) / 2.0).floor();
        Layout {
            arena_x,
            arena_y,
            cell,
            grid_w,
            grid_h,
        }
    }

    /// Centro del píxel de la celda `(x, y)`.
    pub fn cell_to_px(&self, x: f32, y: f32) -> (f32, f32) {
        (
            self.arena_x + (x + 0.5) * self.cell,
            self.arena_y + (y + 0.5) * self.cell,
        )
    }

    /// Celda bajo el punto `(mx, my)` de la ventana, o `None` si está fuera.
    pub fn hovered_cell(&self, mx: f32, my: f32) -> Option<(u16, u16)> {
        if mx < self.arena_x || my < self.arena_y {
            return None;
        }
        let cx = ((mx - self.arena_x) / self.cell).floor() as i64;
        let cy = ((my - self.arena_y) / self.cell).floor() as i64;
        if cx < 0 || cy < 0 || cx >= self.grid_w as i64 || cy >= self.grid_h as i64 {
            None
        } else {
            Some((cx as u16, cy as u16))
        }
    }
}

/// Dibuja el fotograma completo según la pantalla activa.
pub fn render(app: &App) {
    let (w, h) = (screen_width(), screen_height());
    clear_background(app.theme.bg.to_mq(1.0));
    match app.screen {
        Screen::Menu => menu::draw_menu(app, w, h),
        Screen::ModeMenu | Screen::Selector => menu::draw_level_select(app, w, h),
        Screen::Playing => render_game(app, w, h),
        Screen::Results => menu::draw_results(app, w, h),
        Screen::ModeComplete => menu::draw_mode_complete(app, w, h),
        Screen::Error => menu::draw_error(app, &app.error_msg, w, h),
    }
    if app.quit_confirm {
        menu::draw_quit_confirm(app, w, h);
    }
}

/// Pantalla de juego: HUD + arena + overlays de pausa/toasts.
fn render_game(app: &App, w: f32, h: f32) {
    let rows = crate::screens::hud_rows_for(app);
    let g = &app.state.arena.grid;
    let layout = Layout::compute(w, h, g.w, g.h, rows);
    arena::draw_arena(app, &layout);
    draw_floaters(app);
    hud::draw_hud(app, &layout);
    draw_hint(app, w, h);
    if app.state.phase == jezzball_core::state::GamePhase::Paused {
        menu::draw_pause_overlay(app, &layout);
    }
    if let Some(toast) = &app.toast {
        menu::draw_toast(app, toast, w);
    }
}

/// Barra de pistas con los atajos principales.
fn draw_hint(app: &App, w: f32, h: f32) {
    let line =
        "ESPACIO pausa   R reiniciar   TAB eje   raton izq/der muro   F HUD   Q salir   ESC menu";
    let sz2 = 12.0 * ui_scale();
    let x = ((w - text_w(line, sz2)) / 2.0).max(4.0);
    draw_text(
        line,
        x,
        h - hint_h() / 2.0 + 5.0 * ui_scale(),
        sz2,
        app.theme.fg_dim.to_mq(0.75),
    );
}

/// Textos flotantes de recompensa (puntos, combo, vidas).
fn draw_floaters(app: &App) {
    for f in &app.floaters {
        let alpha = (f.ttl / 1.4).clamp(0.0, 1.0);
        let color = match f.kind {
            crate::screens::FloaterKind::Good => app.theme.ok,
            crate::screens::FloaterKind::Bad => app.theme.danger,
            crate::screens::FloaterKind::Combo => app.theme.accent,
            crate::screens::FloaterKind::Neutral => app.theme.fg,
        }
        .to_mq(alpha);
        let size = match f.kind {
            crate::screens::FloaterKind::Combo => 18.0,
            _ => 16.0,
        };
        draw_text_c(&f.text, f.x, f.y, size, color);
    }
}

/// Ancho aproximado de un texto con la fuente por defecto.
pub fn text_w(text: &str, size: f32) -> f32 {
    measure_text(text, None, size as u16, 1.0).width
}

/// Texto centrado horizontalmente en `cx`, con `y` como borde superior.
pub fn draw_text_c(text: &str, cx: f32, y: f32, size: f32, color: Color) {
    let w = text_w(text, size);
    draw_text(text, cx - w / 2.0, y, size, color);
}

/// Dibuja segmentos de texto coloreados en línea, devolviendo la `x` final.
pub fn draw_segments(mut x: f32, y: f32, size: f32, segs: &[(&str, Color)]) -> f32 {
    for (s, c) in segs {
        draw_text(s, x, y, size, *c);
        x += text_w(s, size) + 3.0;
    }
    x
}

/// Panel de fondo con borde fino, estilo terminal.
pub fn panel(t: &crate::theme::Theme, x: f32, y: f32, w: f32, h: f32) {
    draw_rectangle(x, y, w, h, t.bg_panel.to_mq(1.0));
    draw_rectangle_lines(x, y, w, h, 1.0, t.fg_dim.to_mq(0.35));
}

/// Formatea segundos como `MM:SS`.
pub fn fmt_time(s: f32) -> String {
    let s = s.max(0.0) as u32;
    format!("{:02}:{:02}", s / 60, s % 60)
}

/// Etiqueta corta de un objetivo, en ASCII seguro (fuente por defecto).
pub fn objective_label(o: &jezzball_core::level::Objective) -> String {
    use jezzball_core::level::Objective::*;
    match o {
        ClearRatio(r) => format!("Area {:.0}%", r * 100.0),
        NoLivesLost => "Sin perder vidas".to_string(),
        UnderTime(t) => format!("Tiempo <{:.0}s", t),
        MinScore(m) => format!("{} puntos", m),
        KeepCombo(c) => format!("Combo x{}", c),
        NoPowerUps => "Sin power-ups".to_string(),
    }
}

/// Nombre de un modo.
pub fn mode_name(mode: jezzball_core::level::Mode) -> &'static str {
    match mode {
        jezzball_core::level::Mode::Original => "MODO ORIGINAL",
        jezzball_core::level::Mode::Enhanced => "MODO ENHANCED",
    }
}

/// Nombre de un mundo Enhanced.
pub fn world_name(world: u8) -> &'static str {
    match world {
        1 => "Clasico",
        2 => "Velocidad",
        3 => "Obstaculos",
        4 => "Bolas especiales",
        5 => "Power-ups",
        6 => "Retos avanzados",
        _ => "Desconocido",
    }
}
