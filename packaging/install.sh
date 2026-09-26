#!/usr/bin/env bash
# install.sh — local installation of omarchy-jezzball without going through pacman.
#
# Usage:
#   ./packaging/install.sh            install into ~/.local
#   ./packaging/install.sh --uninstall remove from ~/.local, keep your save
#   ./packaging/install.sh --purge     remove from ~/.local, save included
#
# What it installs:
#   ~/.local/bin/omarchy-jezzball
#   ~/.local/share/omarchy-jezzball/levels/*.ron (+ fonts if present)
#   ~/.local/share/applications/omarchy-jezzball.desktop
#   ~/.local/share/icons/hicolor/scalable/apps/omarchy-jezzball.svg

set -euo pipefail

PREFIX="${HOME}/.local"
BINDIR="${PREFIX}/bin"
DATADIR="${PREFIX}/share/omarchy-jezzball"
DESKTOPDIR="${PREFIX}/share/applications"
ICONDIR="${PREFIX}/share/icons/hicolor/scalable/apps"
BIN_NAME="omarchy-jezzball"

# The save file lives under $XDG_DATA_HOME, which DEFAULTS to ~/.local/share —
# the very directory this script installs levels into. A blanket
# `rm -rf "${DATADIR}"` therefore took save.ron with it while the script
# printed that it had not. Resolve the user's directories exactly the way
# persist.rs does, and treat them as the player's data: only --purge removes
# them, and only after saying so.
SAVEDIR="${XDG_DATA_HOME:-${HOME}/.local/share}/omarchy-jezzball"
CONFIGDIR="${XDG_CONFIG_HOME:-${HOME}/.config}/omarchy-jezzball"
STATEDIR="${XDG_STATE_HOME:-${HOME}/.local/state}/omarchy-jezzball"

# Repo root = the parent directory of this script (packaging/..).
ROOT="$(cd "$(dirname "${0}")/.." && pwd)"

log()  { printf '%s\n' "$*"; }
fail() { printf 'error: %s\n' "$*" >&2; exit 1; }

# Removes exactly what this script installs. `purge` additionally removes the
# player's own data (save, config, crash log).
uninstall() {
    local purge="${1:-no}"

    log "Uninstalling ${BIN_NAME} from ${PREFIX}..."
    rm -f "${BINDIR}/${BIN_NAME}"

    # Only the asset directories we created, never the whole DATADIR: the save
    # file may be sitting right next to them.
    rm -rf "${DATADIR}/levels" "${DATADIR}/fonts"
    # Clean up the parent only if nothing of the player's is left in it.
    rmdir "${DATADIR}" 2>/dev/null || true

    rm -f "${DESKTOPDIR}/omarchy-jezzball.desktop"
    rm -f "${ICONDIR}/omarchy-jezzball.svg"
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "${DESKTOPDIR}" || true
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -qtf "${PREFIX}/share/icons/hicolor" 2>/dev/null || true
    fi

    if [ "${purge}" = "purge" ]; then
        log "Purging your saved progress and configuration..."
        rm -rf "${SAVEDIR}" "${CONFIGDIR}" "${STATEDIR}"
        log "Purged. Save, config and crash log are gone."
        return
    fi

    log "Uninstalled. Your progress is kept at ${SAVEDIR}/save.ron"
    log "(and your config at ${CONFIGDIR}). Use --purge to remove those too."
}

if [ "${1:-}" = "--uninstall" ]; then
    uninstall
    exit 0
fi

if [ "${1:-}" = "--purge" ]; then
    uninstall purge
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
