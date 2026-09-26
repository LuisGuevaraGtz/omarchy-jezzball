#!/usr/bin/env python3
"""Checks that no Spanish text is left in the files that ship to GitHub.

Looks for accented characters and a list of common Spanish words, skipping
what is legitimately Spanish: the `assets/i18n/es.ron` catalogue (that IS the
Spanish translation), the i18n keys, and the git history.

Usage:  python3 tools/check_english.py
Exit code 1 if anything is found, so CI can gate on it.
"""
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Files allowed to contain Spanish.
ALLOWED = {
    "assets/i18n/es.ron",  # the Spanish catalogue itself
    "tools/check_english.py",  # this file lists Spanish words on purpose
    # The 71 level names are still Spanish ("Salida de Consola", "Kernel
    # Panic"). They are game DATA, not interface strings: moving them to the
    # catalogue changes the level schema, so it is a deliberate pending
    # decision rather than an oversight. `assets/levels/*.ron` is not scanned
    # (not a source extension); this entry covers the generator that writes it.
    "tools/gen_levels.py",
}

ACCENTS = re.compile(r"[áéíóúÁÉÍÓÚñÑ¿¡]")

# Common Spanish words that would betray an untranslated comment. Deliberately
# short and unambiguous in English source: "para", "como" and friends appear in
# no English sentence.
WORDS = re.compile(
    r"\b("
    r"para|pero|porque|cuando|donde|desde|hasta|entre|sobre|"
    r"este|esta|esto|esos|esas|aquel|"
    r"que|quien|cual|"
    r"del|los|las|una|unos|unas|"
    r"con|sin|por|"
    r"muro|bola|celda|nivel|mundo|vidas|juego|jugador|pantalla|modo|"
    r"tema|fichero|archivo|ruta|"
    r"siempre|nunca|tambien|ademas|entonces|mientras|aunque|"
    r"hacer|tener|poder|deber|estar|"
    r"ninguno|ninguna|cualquier"
    r")\b",
    re.IGNORECASE,
)

# i18n keys are Spanish on purpose ("menu.titulo"): they are identifiers.
I18N_KEY = re.compile(r'"[a-z_]+(\.[a-z_0-9]+)+"')

# A `.desktop` entry localises a key by suffixing the locale: `Name[es]=...`
# IS the Spanish translation, exactly like assets/i18n/es.ron. The untagged
# `Name=` on the line above it is the one that must be English -- and it was
# not: both actions shipped as "Modo Original" / "Modo Enhanced", which no
# word in this list happened to catch.
DESKTOP_ES_KEY = re.compile(r"^\s*[A-Za-z]+\[es(_[A-Z]{2})?\]\s*=")


# Lines that legitimately contain Spanish-looking text and must not be flagged:
# trigonometry (`sin`/`cos`), the accent list used by the i18n guard itself,
# and comments that quote a Spanish string while explaining a fixed bug.
FALSE_POSITIVES = (
    "angle.sin_cos()",
    "self.x * sin",
    "math.sin(",
    "math.cos(",
    'áéíóúñÁÉÍÓÚÑ¿¡',
    'the literal theme "corriente"',
    '"MODO ENHANCED" slip through',
)


def tracked_files():
    out = subprocess.run(
        ["git", "ls-files"], cwd=ROOT, capture_output=True, text=True, check=True
    ).stdout.split()
    keep = (".rs", ".md", ".toml", ".yml", ".yaml", ".sh", ".py", ".desktop")
    return [
        f
        for f in out
        if f not in ALLOWED and (f.endswith(keep) or f.endswith("PKGBUILD"))
    ]


def main():
    findings = []
    for rel in tracked_files():
        path = ROOT / rel
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, FileNotFoundError):
            continue
        for n, line in enumerate(text.splitlines(), 1):
            if any(fp in line for fp in FALSE_POSITIVES):
                continue
            if DESKTOP_ES_KEY.match(line):
                continue
            clean = I18N_KEY.sub("", line)
            if ACCENTS.search(clean) or WORDS.search(clean):
                findings.append(f"{rel}:{n}  {line.strip()[:100]}")

    if findings:
        print(f"{len(findings)} lines still look like Spanish:\n")
        for f in findings[:60]:
            print(f"  {f}")
        if len(findings) > 60:
            print(f"  ... and {len(findings) - 60} more")
        return 1

    print("No Spanish text found outside assets/i18n/es.ron")
    return 0


if __name__ == "__main__":
    sys.exit(main())
