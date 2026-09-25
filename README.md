# Omarchy-Jezzball

Juego nativo tipo JezzBall para Omarchy (Arch Linux + Hyprland, Wayland).
Cierra regiones de la arena trazando muros sin que las bolas los rompan.
Sin Electron, sin motor pesado, sin cuentas: un binario, teclado primero,
estética terminal que sigue tu tema de Omarchy.

Contrato de arquitectura: `docs/ARCHITECTURE.md` (normativo).
Esquema de niveles: `docs/LEVEL_SCHEMA.md`. Theming: `docs/THEMING.md`.

## Filosofía y modos

Dos modos, dos intenciones distintas:

- **Modo Original** — preservación fiel. 10 niveles, arena rectangular,
  bolas normales, sin power-ups, sin obstáculos, sin objetivos secundarios
  ni combos (multiplicador fijo x1). La curva es la del clásico: el nivel N
  tiene `1 + N` bolas, cada vez un poco más rápidas. Flag `purist = true`.
- **Modo Enhanced** — evolución. 60 niveles en 6 mundos × 10:
  1 Clásico, 2 Velocidad, 3 Obstáculos, 4 Bolas especiales,
  5 Power-ups, 6 Retos avanzados. Con combos (hasta x8), 5 tipos de bola
  especial, 5 power-ups, obstáculos (`Block`, `Mover`, `NoSplit`) y hasta
  3 objetivos por nivel (estrellas). Los niveles 5/10 de cada mundo son
  especiales: `SpeedRun`, `Boss`, `Chaos`. Los niveles viven en
  `assets/levels/*.ron`, así que se pueden ampliar sin recompilar.

## Requisitos

- Arch Linux (o derivada), compositor Wayland (Hyprland recomendado).
- Rust estable (`rustup`, toolchain `stable`).
- Librerías de sistema: `wayland`, `libxkbcommon`, `libglvnd`, `alsa-lib`.
- `git`, `cargo`. Para el lanzador: un menú que lea `.desktop`.

## Instalación

### Opción A — Paquete Arch (recomendado)

```bash
cd packaging
makepkg -si
```

Esto compila con `cargo build --frozen --release --all-features`, pasa los
tests del core y instala:

- binario en `/usr/bin/omarchy-jezzball`
- niveles y fuentes en `/usr/share/omarchy-jezzball/`
- lanzador en `/usr/share/applications/omarchy-jezzball.desktop`
- licencia en `/usr/share/licenses/omarchy-jezzball/LICENSE`

### Opción B — Instalación local sin pacman (desarrollo)

```bash
./packaging/install.sh
```

Compila en release e instala en `~/.local/bin`,
`~/.local/share/omarchy-jezzball` y `~/.local/share/applications`.
Avisa si `~/.local/bin` no está en tu `PATH`. Para desinstalar:

```bash
./packaging/install.sh --uninstall
```

(Tu partida no se toca: vive en la ruta XDG, ver abajo.)

### ¿Dónde busca el juego los niveles?

Los niveles (`original.ron` y `enhanced.ron`) se cargan en ejecución desde
disco y se amplían sin recompilar. El juego prueba estas rutas, en este
orden (primera que exista gana):

1. `$OMARCHY_JEZZBALL_ASSETS/levels/` — escotilla de desarrollo/tests;
   apunta a la carpeta `assets/` del repo.
2. `<directorio del binario>/assets/levels/` — binario junto a los assets.
3. `<directorio del binario>/../share/omarchy-jezzball/levels/` — lo que
   produce `packaging/install.sh` (`~/.local/bin` + `~/.local/share`).
4. `$XDG_DATA_HOME/omarchy-jezzball/levels/`
   (si no está definida: `~/.local/share/omarchy-jezzball/levels/`).
5. `/usr/share/omarchy-jezzball/levels/` — lo que instala el `PKGBUILD`.
6. `./assets/levels/` — relativo al directorio actual (para `cargo run`
   desde la raíz del repo).

Si ninguna ruta contiene los niveles, la pantalla de error lista las rutas
que se probaron, una por línea, y la misma lista se imprime por `stderr` al
arrancar.

### Hyprland (opcional)

`packaging/hyprland.conf.example` trae reglas sugeridas (ventana flotante
de 1280×800 centrada) y un bind de ejemplo (`SUPER+J`). Copia lo que
quieras a tu `hyprland.conf`.

## Cómo se juega

Ganas un nivel cerrando al menos el 75 % del área rellenable
(`target_ratio`, puede variar en Enhanced). Cómo:

1. Haz clic en una celda libre: desde ahí crece un muro en ambas
   direcciones del eje actual (horizontal o vertical).
2. Si el muro se consolida antes de que una bola lo toque, la región sin
   bolas que quede aislada se rellena y suma puntos.
3. Si una bola toca el muro mientras crece, lo destruye y pierdes una vida
   (salvo escudo; la bola `Heavy` rompe siempre).

Puntuar bien es arriesgar: cerrar regiones pequeñas cerca de bolas rápidas
paga más (multiplicador de tamaño × riesgo × velocidad × combo). Encadenar
muros sin perder vidas dentro de 4 s sube el combo hasta x8.

Flags de línea de comandos:

```bash
omarchy-jezzball                 # selector de modo
omarchy-jezzball --mode original # arranque directo en Modo Original
omarchy-jezzball --mode enhanced # arranque directo en Modo Enhanced
```

## Atajos de teclado

Fuente: contrato §13. El ratón solo traza muros; todo lo demás es teclado.

| Tecla              | Acción                              |
|--------------------|-------------------------------------|
| `Espacio`          | pausar / reanudar                   |
| `R`                | reiniciar nivel                     |
| `Esc`              | menú / atrás                        |
| `Tab`              | alternar eje del muro (H/V)         |
| `1`–`5`            | usar power-up (slots 1 a 5)         |
| `Enter`            | confirmar                           |
| `↑` `↓` `←` `→` / `hjkl` | navegar menús                 |
| `F`                | alternar HUD compacto               |
| `Q`                | salir (pide confirmación)           |
| clic izquierdo     | trazar muro según el eje actual     |
| clic derecho       | trazar muro con el eje contrario    |

## Dónde se guarda la partida

Contrato §11. Sin red, sin cuentas, escritura atómica
(`save.ron.tmp` + `rename`):

- Partida: `$XDG_DATA_HOME/omarchy-jezzball/save.ron`
  (si `XDG_DATA_HOME` no está definido: `~/.local/share/omarchy-jezzball/save.ron`).
  Guarda por modo y nivel: mejor puntuación, estrellas (0–3), completado
  y mejor tiempo.
- Config: `$XDG_CONFIG_HOME/omarchy-jezzball/config.ron`
  (normalmente `~/.config/omarchy-jezzball/config.ron`).
- Override de tema: `~/.config/omarchy-jezzball/theme.ron` (ver theming).

## Si el juego no arranca o se cierra

Cada fallo interno se registra en disco, además de imprimirse por stderr:

```
~/.local/state/omarchy-jezzball/crash.log
```

Ese fichero es lo único necesario para diagnosticar un problema: contiene la
línea exacta del fallo. Si abres una incidencia, adjúntalo.

**Ventana que no abre.** El juego usa el backend X11 de miniquad por defecto
(bajo Hyprland funciona vía XWayland, que es lo habitual). El backend nativo
de Wayland de miniquad está marcado como inestable por sus propios autores,
así que sólo se usa si X11 no está disponible. Puedes forzarlo:

```bash
OMARCHY_JEZZBALL_BACKEND=wayland  omarchy-jezzball   # sólo Wayland
OMARCHY_JEZZBALL_BACKEND=x11      omarchy-jezzball   # sólo X11
OMARCHY_JEZZBALL_BACKEND=wayland-first omarchy-jezzball
```

**"No hay niveles disponibles".** El juego no encontró los `.ron`. Las rutas
que probó se listan por stderr al arrancar. Puedes indicarla a mano:

```bash
OMARCHY_JEZZBALL_ASSETS=/ruta/que/contiene/levels omarchy-jezzball
```

**Texto demasiado pequeño o demasiado grande.** La interfaz escala sola con
el tamaño de la ventana (referencia 1280x720, mínimo x1.15). Para ajustarla
a mano:

```bash
OMARCHY_JEZZBALL_UI_SCALE=1.5 omarchy-jezzball   # 1.5x
OMARCHY_JEZZBALL_UI_SCALE=1.0 omarchy-jezzball   # tamano base
```

**Depurar un fallo con comprobaciones extra.** El perfil `release-checked`
compila optimizado pero con detección de desbordamientos y símbolos de
depuración:

```bash
cargo build --profile release-checked
./target/release-checked/omarchy-jezzball
```

## Theming

El juego no trae paleta propia: deriva sus 9 roles de color del tema activo
de Omarchy (`bg`, `fg`, `wall`, `wall_building`, `ball`, `ball_special`,
`danger`, `ok`, `accent`). Orden: tu `theme.ron` manual →
`~/.config/omarchy/current/theme/alacritty.toml` → tema por nombre →
fallback oscuro. Cambiar de tema se aplica al recuperar el foco, sin
reiniciar. Detalle completo y ejemplo de `theme.ron` en `docs/THEMING.md`.

## Estructura del repo

```text
Omarchy-Jezzball/
├─ Cargo.toml                    # workspace virtual (miembros: core, app)
├─ crates/
│  ├─ jezzball-core/             # lógica pura: sin render, sin SO, sin I/O
│  └─ jezzball-app/              # macroquad: render, input, audio, guardado
├─ assets/
│  ├─ levels/original.ron        # 10 niveles Modo Original
│  ├─ levels/enhanced.ron        # 60 niveles Modo Enhanced
│  └─ fonts/
├─ packaging/
│  ├─ PKGBUILD
│  ├─ omarchy-jezzball.desktop
│  ├─ install.sh
│  └─ hyprland.conf.example
├─ docs/
│  ├─ ARCHITECTURE.md            # contrato normativo
│  ├─ LEVEL_SCHEMA.md            # esquema de niveles (normativo)
│  └─ THEMING.md
└─ README.md
```

## Compilar y testear desde fuente

```bash
cargo build --locked                    # debug
cargo build --locked --release          # release
cargo test -p jezzball-core             # tests obligatorios del core
cargo clippy -p jezzball-core -- -D warnings   # clippy limpio en core
```

El core es determinista y puro: mismo estado + misma entrada + mismo `dt`
da el mismo resultado. No hay `cargo` en el empaquetado local más allá de
lo aquí descrito; `packaging/install.sh` cubre el flujo de desarrollo.
