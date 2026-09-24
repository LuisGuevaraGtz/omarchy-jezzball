#!/usr/bin/env bash
# install.sh — instalación local de omarchy-jezzball sin pasar por pacman.
#
# Uso:
#   ./packaging/install.sh            instala en ~/.local
#   ./packaging/install.sh --uninstall desinstala de ~/.local
#
# Qué instala:
#   ~/.local/bin/omarchy-jezzball
#   ~/.local/share/omarchy-jezzball/levels/*.ron (+ fonts si existen)
#   ~/.local/share/applications/omarchy-jezzball.desktop
#
# SUPUESTO: el workspace define el binario `omarchy-jezzball`
# (worker de la app: [[bin]] name = "omarchy-jezzball").

set -euo pipefail

PREFIX="${HOME}/.local"
BINDIR="${PREFIX}/bin"
DATADIR="${PREFIX}/share/omarchy-jezzball"
DESKTOPDIR="${PREFIX}/share/applications"
BIN_NAME="omarchy-jezzball"

# Raíz del repo = directorio padre de este script (packaging/..).
ROOT="$(cd "$(dirname "${0}")/.." && pwd)"

log()  { printf '%s\n' "$*"; }
fail() { printf 'error: %s\n' "$*" >&2; exit 1; }

uninstall() {
    log "Desinstalando ${BIN_NAME} de ${PREFIX}..."
    rm -f "${BINDIR}/${BIN_NAME}"
    rm -rf "${DATADIR}"
    rm -f "${DESKTOPDIR}/omarchy-jezzball.desktop"
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "${DESKTOPDIR}" || true
    fi
    log "Desinstalado. (No se toca tu partida: ~/.local/share/omarchy-jezzball/save.ron no existe ahí; el save vive junto a XDG_DATA_HOME, ver README.)"
}

if [ "${1:-}" = "--uninstall" ]; then
    uninstall
    exit 0
fi

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    sed -n '2,/^$/p' "${0}"
    exit 0
fi

# 1. Comprobar toolchain.
command -v cargo >/dev/null 2>&1 \
    || fail "no se encontró 'cargo'. Instala Rust estable (rustup: https://rustup.rs) y reintenta."

# 2. Compilar en release. `--locked` usa el Cargo.lock versionado.
log "Compilando ${BIN_NAME} (release)..."
(
    cd "${ROOT}"
    cargo build --locked --release
)

# 3. Instalar binario.
log "Instalando binario en ${BINDIR}/${BIN_NAME}..."
mkdir -p "${BINDIR}"
install -m755 "${ROOT}/target/release/${BIN_NAME}" "${BINDIR}/${BIN_NAME}"

# 4. Instalar assets (los niveles se cargan en ejecución desde disco).
log "Instalando assets en ${DATADIR}..."
mkdir -p "${DATADIR}/levels"
install -m644 "${ROOT}"/assets/levels/*.ron "${DATADIR}/levels/"
if compgen -G "${ROOT}/assets/fonts/*" > /dev/null; then
    mkdir -p "${DATADIR}/fonts"
    install -m644 "${ROOT}"/assets/fonts/* "${DATADIR}/fonts/"
else
    log "Aviso: no hay fuentes en assets/fonts, se omite ese paso."
fi

# 5. Instalar entrada de escritorio.
log "Instalando lanzador en ${DESKTOPDIR}..."
mkdir -p "${DESKTOPDIR}"
install -m644 "${ROOT}/packaging/omarchy-jezzball.desktop" \
    "${DESKTOPDIR}/omarchy-jezzball.desktop"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "${DESKTOPDIR}" || true
else
    log "Aviso: 'update-desktop-database' no encontrado; el lanzador funcionará tras reiniciar sesión."
fi

# 6. Avisar si ~/.local/bin no está en el PATH.
case ":${PATH}:" in
    *":${BINDIR}:"*) ;;
    *)
        log "AVISO: ${BINDIR} no está en tu PATH."
        log "Añade esta línea a tu ~/.bashrc (o equivalente) y recarga la shell:"
        log "  export PATH=\"\$HOME/.local/bin:\$PATH\""
        ;;
esac

log "Listo. Ejecuta '${BIN_NAME}' o búscalo como JezzBall en tu lanzador."
