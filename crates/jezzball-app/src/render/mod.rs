//! Global render (ARCHITECTURE.md §13): the render layer NEVER decides game
//! logic, it merely reads `&App` / `&GameState` and draws.
//!
//! `Layout` computes where the arena lives inside the window and is used both
//! for drawing and for mouse hit-testing in `screens`.

pub mod arena;
pub mod hud;
pub mod menu;

use macroquad::prelude::*;

use crate::screens::{App, Screen};

/// Base font size of the HUD, at the reference scale.
/// 16 px: at 14 px the HUD was unreadable even after scaling.
pub const FONT_BASE: f32 = 16.0;
/// Base height of a HUD row (px).
pub const HUD_ROW_BASE: f32 = 24.0;
/// Base height of the bottom hint bar.
pub const HINT_H_BASE: f32 = 20.0;

/// UI scale.
///
/// The HUD was designed with `FONT_BASE` for a small window. With fixed
/// sizes, on a large screen or with `high_dpi` the text ends up tiny: the
/// arena grows with the window but the letters do not.
///
/// The reference is 1280x720 (deliberately not the real resolution of a modern
/// monitor): that way a 1900x1000 window —normal on a current laptop—
/// gives around x1.5, which is where the HUD reads comfortably. Verified against
/// real screenshots of the game at 1890x1017, where the previous reference
/// (1024x768) stayed at x1.32 and the text was still unreadable.
///
/// `OMARCHY_JEZZBALL_UI_SCALE` allows adjusting it by hand (e.g. `2.0`).
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
    // The most restrictive dimension is taken so as not to overflow the width in
    // landscape windows nor the height in narrow ones.
    let s = (w / 1280.0).min(h / 720.0);
    // Floor at 1.15: even in a small window the base text was
    // tight. Ceiling at 3.0 so it does not invade the arena on huge screens.
    s.clamp(1.15, 3.0)
}

/// HUD font size, already scaled.
pub fn font() -> f32 {
    (FONT_BASE * ui_scale()).round()
}

/// HUD row height, already scaled.
pub fn hud_row() -> f32 {
    HUD_ROW_BASE * ui_scale()
}

/// Height of the hint bar, already scaled.
pub fn hint_h() -> f32 {
    HINT_H_BASE * ui_scale()
}

/// Geometry of the arena inside the window.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub arena_x: f32,
    pub arena_y: f32,
    pub cell: f32,
    pub grid_w: u16,
    pub grid_h: u16,
}

impl Layout {
    /// Fits the arena of `grid_w x grid_h` square cells below the
    /// HUD and above the hint bar, horizontally centred.
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

    /// Pixel centre of the cell `(x, y)`.
    pub fn cell_to_px(&self, x: f32, y: f32) -> (f32, f32) {
        (
            self.arena_x + (x + 0.5) * self.cell,
            self.arena_y + (y + 0.5) * self.cell,
        )
    }

    /// Cell under the window point `(mx, my)`, or `None` if it is outside.
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

/// Draws the whole frame according to the active screen.
pub fn render(app: &App) {
    let (w, h) = (screen_width(), screen_height());
    clear_background(app.theme.bg.to_mq(1.0));
    match app.screen {
        Screen::Menu => menu::draw_menu(app, w, h),
        Screen::ModeMenu | Screen::Selector => menu::draw_level_select(app, w, h),
        Screen::Playing => render_game(app, w, h),
        Screen::Results => menu::draw_results(app, w, h),
        Screen::ModeComplete => menu::draw_mode_complete(app, w, h),
        Screen::Help => menu::draw_help(app, w, h),
        Screen::Error => menu::draw_error(app, &app.error_msg, w, h),
    }
    if app.quit_confirm {
        menu::draw_quit_confirm(app, w, h);
    }
}

/// Game screen: HUD + arena + pause/toast overlays.
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

/// Hint bar with the main shortcuts.
fn draw_hint(app: &App, w: f32, h: f32) {
    let line = crate::i18n::t("pie.juego");
    let sz2 = 12.0 * ui_scale();
    let x = ((w - text_w(line, sz2)) / 2.0).max(4.0);
    draw_text(
        line,
        x,
        h - hint_h() / 2.0 + 5.0 * ui_scale(),
        sz2,
        app.theme.fg_dim.to_mq(1.0),
    );
}

/// Floating reward texts (points, combo, lives).
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

/// Approximate width of a text with the default font.
pub fn text_w(text: &str, size: f32) -> f32 {
    measure_text(text, None, size as u16, 1.0).width
}

/// Text horizontally centred at `cx`, with `y` as the top edge.
pub fn draw_text_c(text: &str, cx: f32, y: f32, size: f32, color: Color) {
    let w = text_w(text, size);
    draw_text(text, cx - w / 2.0, y, size, color);
}

/// Draws coloured text segments in a line, returning the final `x`.
pub fn draw_segments(mut x: f32, y: f32, size: f32, segs: &[(&str, Color)]) -> f32 {
    for (s, c) in segs {
        draw_text(s, x, y, size, *c);
        x += text_w(s, size) + 3.0;
    }
    x
}

/// Background panel with a thin border, terminal style.
pub fn panel(t: &crate::theme::Theme, x: f32, y: f32, w: f32, h: f32) {
    draw_rectangle(x, y, w, h, t.bg_panel.to_mq(1.0));
    draw_rectangle_lines(x, y, w, h, 1.0, t.fg_dim.to_mq(0.35));
}

/// Formats seconds as `MM:SS`.
pub fn fmt_time(s: f32) -> String {
    let s = s.max(0.0) as u32;
    format!("{:02}:{:02}", s / 60, s % 60)
}

/// Short label for an objective, in safe ASCII (default font).
pub fn objective_label(o: &jezzball_core::level::Objective) -> String {
    use crate::i18n::t;
    use jezzball_core::level::Objective::*;
    match o {
        ClearRatio(r) => t("objetivo.area").replace("{:.0}", &format!("{:.0}", r * 100.0)),
        NoLivesLost => t("objetivo.sin_vidas").to_string(),
        UnderTime(s) => t("objetivo.tiempo").replace("{:.0}", &format!("{:.0}", s)),
        MinScore(m) => t("objetivo.puntos").replace("{}", &m.to_string()),
        KeepCombo(c) => t("objetivo.combo").replace("{}", &c.to_string()),
        NoPowerUps => t("objetivo.sin_powerups").to_string(),
    }
}

/// Name of a mode.
pub fn mode_name(mode: jezzball_core::level::Mode) -> &'static str {
    use crate::i18n::t;
    match mode {
        jezzball_core::level::Mode::Original => t("menu.modo_original"),
        jezzball_core::level::Mode::Enhanced => t("menu.modo_enhanced"),
    }
}

/// Name of an Enhanced world.
pub fn world_name(world: u8) -> &'static str {
    use crate::i18n::t;
    match world {
        1 => t("mundo.1"),
        2 => t("mundo.2"),
        3 => t("mundo.3"),
        4 => t("mundo.4"),
        5 => t("mundo.5"),
        6 => t("mundo.6"),
        _ => t("mundo.desconocido"),
    }
}
