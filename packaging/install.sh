#!/usr/bin/env bash
# install.sh — local installation of omarchy-jezzball without going through pacman.
#
# Usage:
#   ./packaging/install.sh            install into ~/.local
#   ./packaging/install.sh --uninstall remove from ~/.local
#
# What it installs:
#   ~/.local/bin/omarchy-jezzball
#   ~/.local/share/omarchy-jezzball/levels/*.ron (+ fonts if present)
#   ~/.local/share/applications/omarchy-jezzball.desktop
#   ~/.local/share/icons/hicolor/scalable/apps/omarchy-jezzball.svg
#
# ASSUMPTION: the workspace defines the `omarchy-jezzball` binary
# (app worker: [[bin]] name = "omarchy-jezzball").

set -euo pipefail

PREFIX="${HOME}/.local"
BINDIR="${PREFIX}/bin"
DATADIR="${PREFIX}/share/omarchy-jezzball"
DESKTOPDIR="${PREFIX}/share/applications"
ICONDIR="${PREFIX}/share/icons/hicolor/scalable/apps"
BIN_NAME="omarchy-jezzball"

# Repo root = the parent directory of this script (packaging/..).
ROOT="$(cd "$(dirname "${0}")/.." && pwd)"

log()  { printf '%s\n' "$*"; }
fail() { printf 'error: %s\n' "$*" >&2; exit 1; }

uninstall() {
    log "Uninstalling ${BIN_NAME} from ${PREFIX}..."
    rm -f "${BINDIR}/${BIN_NAME}"
    rm -rf "${DATADIR}"
    rm -f "${DESKTOPDIR}/omarchy-jezzball.desktop"
    rm -f "${ICONDIR}/omarchy-jezzball.svg"
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "${DESKTOPDIR}" || true
    fi
    log "Uninstalled. (Your save file is untouched: ~/.local/share/omarchy-jezzball/save.ron is not there; the save lives alongside XDG_DATA_HOME, see the README.)"
}

if [ "${1:-}" = "--uninstall" ]; then
    uninstall
    exit 0
fi

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    sed -n '2,/^$/p' "${0}"
    exit 0
fi

# 1. Check the toolchain.
command -v cargo >/dev/null 2>&1 \
    || fail "'cargo' not found. Install stable Rust (rustup: https://rustup.rs) and try again."

# 2. Build in release mode. `--locked` uses the committed Cargo.lock.
log "Building ${BIN_NAME} (release)..."
(
    cd "${ROOT}"
    cargo build --locked --release
)

# 3. Install the binary.
log "Installing the binary at ${BINDIR}/${BIN_NAME}..."
mkdir -p "${BINDIR}"
install -m755 "${ROOT}/target/release/${BIN_NAME}" "${BINDIR}/${BIN_NAME}"

# 4. Install assets (levels are loaded from disk at runtime).
log "Installing assets into ${DATADIR}..."
mkdir -p "${DATADIR}/levels"
install -m644 "${ROOT}"/assets/levels/*.ron "${DATADIR}/levels/"
if compgen -G "${ROOT}/assets/fonts/*" > /dev/null; then
    mkdir -p "${DATADIR}/fonts"
    install -m644 "${ROOT}"/assets/fonts/* "${DATADIR}/fonts/"
else
    log "Note: no fonts in assets/fonts, skipping that step."
fi

# 5. Install the icon (hicolor scalable: every launcher picks it up).
log "Installing the icon into ${ICONDIR}..."
mkdir -p "${ICONDIR}"
install -m644 "${ROOT}/assets/icons/omarchy-jezzball.svg" \
    "${ICONDIR}/omarchy-jezzball.svg"
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -qtf "${PREFIX}/share/icons/hicolor" 2>/dev/null || true
fi

# 6. Install the desktop entry.
log "Installing the launcher into ${DESKTOPDIR}..."
mkdir -p "${DESKTOPDIR}"
install -m644 "${ROOT}/packaging/omarchy-jezzball.desktop" \
    "${DESKTOPDIR}/omarchy-jezzball.desktop"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "${DESKTOPDIR}" || true
else
    log "Note: 'update-desktop-database' not found; the launcher will work after you log out and back in."
fi

# 7. Warn if ~/.local/bin is not on PATH.
case ":${PATH}:" in
    *":${BINDIR}:"*) ;;
    *)
        log "WARNING: ${BINDIR} is not on your PATH."
        log "Add this line to your ~/.bashrc (or equivalent) and reload the shell:"
        log "  export PATH=\"\$HOME/.local/bin:\$PATH\""
        ;;
esac

log "Done. Run '${BIN_NAME}' or look for JezzBall in your launcher."
