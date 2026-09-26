//! Main loop of Omarchy-Jezzball (ARCHITECTURE.md §6 and §13).
//!
//! A single loop: sample input -> `step` (pure core) -> render. `step` is
//! never called expensively more than once per frame. `dt` is clamped
//! to 1/30 so that a long frame does not cause absurd wall flights.

use macroquad::prelude::*;

use jezzball_core::level::Mode;

use crate::persist::save_save;
use crate::screens::{update, App};

mod i18n;
mod input;
mod persist;
mod render;
mod screens;
mod theme;

/// Window: 1024x768, resizable, high resolution, 4x MSAA, V-Sync.
///
/// Window backend: miniquad documents its own Wayland backend as
/// UNSTABLE ("The Wayland implementation is currently unstable"), so we
/// prefer X11 —which under Hyprland works via XWayland— and leave Wayland
/// as the alternative for systems without XWayland installed. The opposite
/// (Wayland first) sounds purer but bets the game's startup on a
/// backend its own authors mark as unstable.
///
/// It can be forced with the `OMARCHY_JEZZBALL_BACKEND` variable:
///   `x11`, `wayland`, `x11-first` (the default) or `wayland-first`.
///
/// We also handle `--help` here: `#[macroquad::main]` calls this function
/// and opens the window BEFORE running the body of `main`, so printing
/// the help from `main` would require a graphics server. A `--help` that only
/// works with a display is not help; that is why we exit right here.
fn window_conf() -> Conf {
    // The hook is installed HERE, not in `main`: `#[macroquad::main]` calls this
    // function and opens the window BEFORE running the body of `main`, so
    // a failure to create the window (the most common case: no graphics
    // server available) would happen with no hook and leave no trace on disk.
    install_panic_hook();

    if std::env::args().skip(1).any(|a| a == "--help" || a == "-h") {
        print_help();
        std::process::exit(0);
    }
    Conf {
        window_title: "Omarchy-Jezzball".to_owned(),
        window_width: 1024,
        window_height: 768,
        window_resizable: true,
        high_dpi: true,
        sample_count: 4,
        platform: miniquad::conf::Platform {
            linux_backend: linux_backend_from_env(),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Window backend according to `OMARCHY_JEZZBALL_BACKEND`, with X11 first by
/// default (see the note on `window_conf`).
fn linux_backend_from_env() -> miniquad::conf::LinuxBackend {
    use miniquad::conf::LinuxBackend as B;
    match std::env::var("OMARCHY_JEZZBALL_BACKEND")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "x11" => B::X11Only,
        "wayland" => B::WaylandOnly,
        "wayland-first" => B::WaylandWithX11Fallback,
        // Default, and for any unrecognised value.
        _ => B::X11WithWaylandFallback,
    }
}

/// Path of the crash log: `$XDG_STATE_HOME/omarchy-jezzball/crash.log`
/// (with the usual fallback to `~/.local/state`).
fn crash_log_path() -> std::path::PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/state"))
        })
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    base.join("omarchy-jezzball").join("crash.log")
}

/// Writes the crash to disk as well as to stderr.
///
/// A full-screen game closes and takes the terminal down with it:
/// asking the player to "copy the error" is not realistic. Leaving the detail in
/// a known file is the difference between diagnosing the failure and having
/// to guess it.
fn log_failure(text: &str) {
    eprintln!("{text}");
    let path = crash_log_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "{text}");
        eprintln!("omarchy-jezzball: details saved to {}", path.display());
    }
}

/// Installs the panic hook that records the crash to stderr and to disk.
///
/// It adds a concrete hint when the failure is "there is no graphics server", which
/// is the most common reason for the game not to start.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let details = info.to_string();
        let mut text = format!("\n=== omarchy-jezzball: internal failure ===\n{details}\n");
        if details.contains("XOpenDisplay") || details.contains("wayland") {
            text.push_str(
                "Could not open a window. The game needs a graphical session \
                 (Hyprland/Wayland with XWayland, or Xorg).\n\
                 Try forcing the backend:\n\
                 \x20 OMARCHY_JEZZBALL_BACKEND=wayland omarchy-jezzball\n\
                 \x20 OMARCHY_JEZZBALL_BACKEND=x11     omarchy-jezzball\n",
            );
        }
        text.push_str(
            "Please report this along with the steps to reproduce it:\n\
             https://github.com/LuisGuevaraGtz/omarchy-jezzball/issues",
        );
        log_failure(&text);
    }));
}

/// CLI entry point: `--mode original|enhanced`, `--level N` (1-based), `--help`.
fn parse_args() -> (Option<Mode>, Option<u16>, bool) {
    let mut mode = None;
    let mut level = None;
    let mut help = false;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--mode" => {
                if let Some(v) = it.next() {
                    mode = match v.to_ascii_lowercase().as_str() {
                        "original" => Some(Mode::Original),
                        "enhanced" => Some(Mode::Enhanced),
                        other => {
                            eprintln!("--mode: invalid value '{other}' (use original|enhanced)");
                            None
                        }
                    };
                } else {
                    eprintln!("--mode needs a value (original|enhanced)");
                }
            }
            "--level" => {
                level = it.next().and_then(|v| v.trim().parse::<u16>().ok());
                if level.is_none() {
                    eprintln!("--level needs a number >= 1");
                }
            }
            "--help" | "-h" => help = true,
            _ => eprintln!("unknown argument: {a}"),
        }
    }
    (mode, level, help)
}

#[macroquad::main(window_conf)]
async fn main() {
    // `--help` was already handled in `window_conf` (see the note there): the
    // macroquad macro opens the window BEFORE running this body, so the
    // help cannot be printed here without requiring a graphics server.
    let (mode, level, _help) = parse_args();

    // The panic hook was already installed in `window_conf` (it runs before
    // this body). Here we only protect the game loop.

    let mut app = App::new();
    crate::screens::launch(&mut app, mode, level);

    loop {
        let dt = get_frame_time().min(1.0 / 30.0);

        // A panic inside `update` must not close the game abruptly: the
        // save file is written and we exit in an orderly fashion.
        let tick = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            update(&mut app, dt);
        }));
        if tick.is_err() {
            eprintln!("omarchy-jezzball: error en la simulacion; guardando y saliendo.");
            break;
        }

        crate::render::render(&app);
        if app.quit_requested {
            break;
        }
        next_frame().await;
    }

    if !save_save(&app.save) {
        eprintln!("omarchy-jezzball: could not save the game on exit");
    }
    println!("omarchy-jezzball: see you!");
}

fn print_help() {
    println!(
        "Omarchy-Jezzball: JezzBall for Omarchy (keyboard driven).
Usage: omarchy-jezzball [--mode original|enhanced] [--level N] [--help]
  --mode    start directly in the given mode.
  --level   start at level N (1-based) of the chosen mode.
  --help    this help.

In-game controls: arrows/k/j navigate, enter/space select,
space pause, R restart, TAB axis, 1..5 power-ups, F compact HUD,
M back to menu, Q quit, ESC pause; mouse: left button = current axis,
right button = opposite axis."
    );
}
