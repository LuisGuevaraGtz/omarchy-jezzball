# docs/THEMING.md — Cómo se colorea Omarchy-Jezzball

El juego no trae paleta propia. Deriva todos sus colores del tema activo de
Omarchy. Esta página explica el orden de resolución, el mapeo de roles y cómo
fijar colores a mano.

## 1. Orden de resolución

El primer paso que devuelva un tema válido gana:

1. **Override del usuario:** `$XDG_CONFIG_HOME/omarchy-jezzball/theme.ron`
   (normalmente `~/.config/omarchy-jezzball/theme.ron`). Si existe y parsea
   bien, se usa tal cual. Es la única fuente que controlas al 100 %.
2. **Tema live de Omarchy:** `~/.config/omarchy/current/theme/alacritty.toml`.
   Se leen `colors.primary.background`, `colors.primary.foreground` y la tabla
   `colors.normal` (`black, red, green, yellow, blue, magenta, cyan, white`).
3. **Tema por nombre:** si Omarchy expone el tema activo bajo
   `~/.config/omarchy/themes/<activo>/`, se lee su `alacritty.toml` igual
   que en el paso 2.
4. **Fallback hardcodeado:** fondo `#0f0f14`, texto `#c9d1d9`,
   acento `#7aa2f7`. Garantiza que el juego siempre arranca, aunque no haya
   ningún tema instalado.

El tema se re-lee al recuperar el foco de la ventana, así que cambiar de tema
en Omarchy se refleja en el juego sin reiniciar.

## 2. Mapeo de roles

El juego trabaja con 9 roles semánticos. El render nunca usa un color
literal: siempre pide uno de estos roles al `Theme`. El mapeo desde un tema
de Omarchy (pasos 2 y 3) es fijo:

| Rol del juego   | Origen en el tema Omarchy   | Uso en el juego                              |
|-----------------|-----------------------------|----------------------------------------------|
| `bg`            | `primary.background`        | fondo de la arena y de los menús             |
| `fg`            | `primary.foreground`        | texto, HUD, bordes de menú                   |
| `wall`          | `normal.white`              | muros ya consolidados                        |
| `wall_building` | `normal.yellow`             | muro en construcción (frentes creciendo)     |
| `ball`          | `normal.cyan`               | bolas normales                               |
| `ball_special`  | `normal.magenta`            | bolas especiales (Fast, Erratic, …)          |
| `danger`        | `normal.red`                | avisos: bola cerca del muro, última vida     |
| `ok`            | `normal.green`              | confirmaciones, nivel superado, estrellas    |
| `accent`        | `normal.blue`               | selección de menú, resaltados, power-ups     |

`normal.black` se ignora: sobre fondos oscuros de terminal no da contraste
suficiente para ningún rol.

## 3. Override manual: `theme.ron`

Para fijar colores a mano, crea `~/.config/omarchy-jezzball/theme.ron` con
los 9 roles en hexadecimal `#rrggbb`. Los 9 son obligatorios; si falta uno
o el fichero no parsea, se ignora entero y se pasa al paso 2 del orden
(no se mezclan fuentes a medias).

Ejemplo completo:

```ron
(
    bg: "#0f0f14",
    fg: "#c9d1d9",
    wall: "#e6e6e6",
    wall_building: "#e5c07b",
    ball: "#56b6c2",
    ball_special: "#c678dd",
    danger: "#e06c75",
    ok: "#98c379",
    accent: "#7aa2f7",
)
```

Notas:

- Los valores son `String` con formato `#rrggbb` en minúsculas (se aceptan
  mayúsculas, pero el ejemplo usa minúsculas por convención).
- Los 3 campos que no aparecen aquí (`name`, `bg_panel`, `fg_dim`) se derivan:
  `name` es "personalizado", `bg_panel` aclara `bg` y `fg_dim` oscurece `fg`
  ambos ~12 % (ajuste suave de luminosidad, por canal).
- Para volver al tema automático de Omarchy, borra o renombra el fichero.
- IMPLEMENTADO: este esquema `(clave: "valor", …)` lo implementa la app en
  `crates/jezzball-app/src/theme.rs` vía el DTO `ThemeOverrideFile`. El test
  `bloque_exacto_de_theming_md_parsea` copia literal este ejemplo: si el
  formato cambia aquí o en el struct, ese test se cae y ambos deben
  actualizarse a la vez.
