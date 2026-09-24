//! Bucle principal de Omarchy-Jezzball (ARCHITECTURE.md §6 y §13).
//!
//! Un solo bucle: muestrear entrada -> `step` (core puro) -> render. Nunca
//! se llama a `step` costosamente más de una vez por frame. `dt` se clamp
//! a 1/30 para que un frame largo no provoque vuelos de pared absurdos.

use macroquad::prelude::*;

use jezzball_core::level::Mode;

use crate::persist::save_save;
use crate::screens::{update, App};

mod input;
mod persist;
mod render;
mod screens;
mod theme;

/// Ventana: 1024x768, reescalable, alta resolución, 4x MSAA, V-Sync.
///
/// Wayland-first (ARCHITECTURE.md §1): miniquad usa `X11Only` por defecto y
/// PANICA si no hay X11, lo que rompería el juego en un Hyprland puro sin
/// XWayland. Preferimos Wayland y dejamos X11 como red de seguridad para
/// quien siga en Xorg.
///
/// Aquí también atendemos `--help`: `#[macroquad::main]` llama a esta función
/// y abre la ventana ANTES de ejecutar el cuerpo de `main`, así que imprimir
/// la ayuda desde `main` exigiría un servidor gráfico. Un `--help` que solo
/// funciona con pantalla no es ayuda; por eso salimos aquí mismo.
fn window_conf() -> Conf {
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
            linux_backend: miniquad::conf::LinuxBackend::WaylandWithX11Fallback,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Entrada CLI: `--mode original|enhanced`, `--level N` (1-based), `--help`.
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
                            eprintln!("--mode invalido: {other} (usa original|enhanced)");
                            None
                        }
                    };
                } else {
                    eprintln!("--mode requiere un valor (original|enhanced)");
                }
            }
            "--level" => {
                level = it.next().and_then(|v| v.trim().parse::<u16>().ok());
                if level.is_none() {
                    eprintln!("--level requiere un numero >= 1");
                }
            }
            "--help" | "-h" => help = true,
            _ => eprintln!("argumento desconocido: {a}"),
        }
    }
    (mode, level, help)
}

#[macroquad::main(window_conf)]
async fn main() {
    // `--help` ya se atendió en `window_conf` (ver nota allí): la macro de
    // macroquad abre la ventana ANTES de ejecutar este cuerpo, así que la
    // ayuda no puede imprimirse aquí sin exigir un servidor gráfico.
    let (mode, level, _help) = parse_args();

    // Red de seguridad: si la lógica de un frame panica, el proceso moriría
    // llevándose la partida sin guardar y sin dejar rastro útil. Capturamos
    // el panic, guardamos, y avisamos por stderr con la info para reportarlo.
    // No enmascara el fallo (se sigue imprimiendo), solo evita perder datos.
    std::panic::set_hook(Box::new(|info| {
        eprintln!("\n=== omarchy-jezzball: fallo interno ===");
        eprintln!("{info}");
        eprintln!(
            "Por favor reporta esto con los pasos para reproducirlo:\n\
             https://github.com/LuisGuevaraGtz/omarchy-jezzball/issues"
        );
    }));

    let mut app = App::new();
    crate::screens::launch(&mut app, mode, level);

    loop {
        let dt = get_frame_time().min(1.0 / 30.0);

        // Un panic dentro de `update` no debe cerrar el juego en seco: se
        // guarda la partida y se sale de forma ordenada.
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
        eprintln!("omarchy-jezzball: no se pudo guardar la partida al salir");
    }
    println!("omarchy-jezzball: hasta luego!");
}

fn print_help() {
    println!(
        "Omarchy-Jezzball: JezzBall para Omarchy (modo teclado).
Usa: omarchy-jezzball [--mode original|enhanced] [--level N] [--help]
  --mode    arranca directamente en el modo dado.
  --level   arranca en el nivel N (1-based) del modo elegido.
  --help    esta ayuda.

Controles en partida: flechas/k/j navegar, enter/espacio elegir,
espacio pausa, R reiniciar, TAB eje, 1..5 power-ups, F HUD compacto,
Q salir, ESC menu; raton: boton izq = eje actual, boton der = opuesto."
    );
}
