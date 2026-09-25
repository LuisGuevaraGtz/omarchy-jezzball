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
/// Backend de ventana: miniquad documenta su propio backend de Wayland como
/// INESTABLE ("The Wayland implementation is currently unstable"), así que
/// preferimos X11 —que bajo Hyprland funciona vía XWayland— y dejamos Wayland
/// como alternativa para sistemas sin XWayland instalado. Lo contrario
/// (Wayland primero) suena más puro pero apuesta el arranque del juego a un
/// backend que sus propios autores marcan como inestable.
///
/// Se puede forzar con la variable `OMARCHY_JEZZBALL_BACKEND`:
///   `x11`, `wayland`, `x11-first` (por defecto) o `wayland-first`.
///
/// Aquí también atendemos `--help`: `#[macroquad::main]` llama a esta función
/// y abre la ventana ANTES de ejecutar el cuerpo de `main`, así que imprimir
/// la ayuda desde `main` exigiría un servidor gráfico. Un `--help` que solo
/// funciona con pantalla no es ayuda; por eso salimos aquí mismo.
fn window_conf() -> Conf {
    // El hook se instala AQUÍ, no en `main`: `#[macroquad::main]` llama a esta
    // función y abre la ventana ANTES de ejecutar el cuerpo de `main`, así que
    // un fallo al crear la ventana (el caso más común: no hay servidor
    // gráfico disponible) ocurriría sin hook y sin dejar rastro en disco.
    instalar_hook_de_panico();

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

/// Backend de ventana según `OMARCHY_JEZZBALL_BACKEND`, con X11 primero por
/// defecto (ver la nota de `window_conf`).
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
        // Por defecto y para cualquier valor no reconocido.
        _ => B::X11WithWaylandFallback,
    }
}

/// Ruta del registro de fallos: `$XDG_STATE_HOME/omarchy-jezzball/crash.log`
/// (con el fallback habitual a `~/.local/state`).
fn crash_log_path() -> std::path::PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/state"))
        })
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    base.join("omarchy-jezzball").join("crash.log")
}

/// Escribe el fallo en disco además de en stderr.
///
/// Un juego a pantalla completa se cierra y se lleva la terminal por delante:
/// pedirle al jugador que "copie el error" no es realista. Dejar el detalle en
/// un fichero conocido es la diferencia entre diagnosticar el fallo y tener
/// que adivinarlo.
fn registrar_fallo(texto: &str) {
    eprintln!("{texto}");
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
        let _ = writeln!(f, "{texto}");
        eprintln!("omarchy-jezzball: detalle guardado en {}", path.display());
    }
}

/// Instala el hook de panic que registra el fallo en stderr y en disco.
///
/// Añade una pista concreta cuando el fallo es "no hay servidor gráfico", que
/// es el motivo más habitual de que el juego no arranque.
fn instalar_hook_de_panico() {
    std::panic::set_hook(Box::new(|info| {
        let detalle = info.to_string();
        let mut texto = format!("\n=== omarchy-jezzball: fallo interno ===\n{detalle}\n");
        if detalle.contains("XOpenDisplay") || detalle.contains("wayland") {
            texto.push_str(
                "No se pudo abrir una ventana. El juego necesita una sesion grafica \
                 (Hyprland/Wayland con XWayland, o Xorg).\n\
                 Prueba a forzar el backend:\n\
                 \x20 OMARCHY_JEZZBALL_BACKEND=wayland omarchy-jezzball\n\
                 \x20 OMARCHY_JEZZBALL_BACKEND=x11     omarchy-jezzball\n",
            );
        }
        texto.push_str(
            "Reporta esto con los pasos para reproducirlo:\n\
             https://github.com/LuisGuevaraGtz/omarchy-jezzball/issues",
        );
        registrar_fallo(&texto);
    }));
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

    // El hook de panic ya se instaló en `window_conf` (se ejecuta antes que
    // este cuerpo). Aquí sólo protegemos el bucle de juego.

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
