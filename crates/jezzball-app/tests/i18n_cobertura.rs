//! Internationalisation guard.
//!
//! The point of moving the texts into `assets/i18n/*.ron` is lost as soon as
//! someone writes a visible string inside the code again. This test walks the
//! sources of the presentation layer and fails if it finds string literals
//! that look like interface text.
//!
//! It does not aim to be a Rust parser: it looks for string literals that read
//! like a phrase (several words, or words with accents) outside comments, and
//! keeps an explicit list of exceptions for what legitimately is not interface
//! text (paths, environment variable names, i18n keys).

use std::path::Path;

/// Files that must not contain embedded visible text.
///
/// Includes `screens.rs` on top of the render layer: that is where the menu
/// labels (`MenuItem`) are built, and leaving it out let an untranslated
/// "MODO ENHANCED" slip through that the player saw on screen.
const SOURCES: &[&str] = &[
    "src/render/menu.rs",
    "src/render/hud.rs",
    "src/render/arena.rs",
    "src/render/mod.rs",
    "src/screens.rs",
];

/// Allowed fragments: they are not interface text.
fn is_allowed(lit: &str) -> bool {
    // i18n keys: "menu.title", "ayuda.p1.body"...
    if lit.contains('.') && !lit.contains(' ') {
        return true;
    }
    const EXCEPTIONS: &[&str] = &[
        "OMARCHY_JEZZBALL_UI_SCALE",
        "OMARCHY_JEZZBALL_ASSETS",
        "OMARCHY_JEZZBALL_BACKEND",
        "OMARCHY_JEZZBALL_LANG",
        "{}  (BLOQUEADO)",
        "{:.0}",
        "ESTRELLAS",
    ];
    EXCEPTIONS.iter().any(|e| lit.contains(e))
}

/// Does this literal look like a UI phrase?
fn looks_like_ui_text(lit: &str) -> bool {
    let trimmed = lit.trim();
    if trimmed.len() < 4 {
        return false;
    }
    // Accents or n-with-tilde: unmistakably Spanish text.
    if trimmed.chars().any(|c| "áéíóúñÁÉÍÓÚÑ¿¡".contains(c)) {
        return true;
    }
    // Single ALL-CAPS words are HUD labels ("SCORE", "LIVES", "PAUSED").
    // They slipped through an earlier version of this check that demanded two
    // words, and the whole HUD shipped untranslated because of it.
    let letters: String = trimmed
        .chars()
        .filter(|c| c.is_alphabetic() || *c == ' ')
        .collect();
    let caps = letters.trim();
    if caps.len() >= 4
        && caps.chars().any(|c| c.is_alphabetic())
        && caps
            .chars()
            .all(|c| c.is_uppercase() || c == ' ' || !c.is_alphabetic())
    {
        return true;
    }
    // Several alphabetic words in a row.
    let words: Vec<&str> = trimmed
        .split_whitespace()
        .filter(|w| w.chars().filter(|c| c.is_alphabetic()).count() >= 3)
        .collect();
    words.len() >= 2
}

/// Extracts string literals from a line, ignoring comments.
///
/// It also ignores the lines that clearly do not render interface: test
/// assertion messages, console diagnostics (`eprintln!`) and internal error
/// strings. What we are after is the text the player sees.
fn string_literals(line: &str) -> Vec<String> {
    let l = line.trim_start();
    if l.starts_with("//") || l.starts_with("/*") || l.starts_with('*') {
        return Vec::new();
    }
    // Messages that never reach the game's interface.
    const NOT_UI: &[&str] = &[
        "assert",
        "panic!",
        "expect(",
        "unwrap_or_else",
        "eprintln!",
        "println!",
        "#[test]",
        "debug_assert",
        // Internal error strings (`Result<_, String>`): they are technical
        // diagnostics for the log, not text the player reads on screen. What
        // the player does see (`app.error_msg`) goes through the catalog.
        "Err(format!",
        "map_err",
    ];
    if NOT_UI.iter().any(|m| l.contains(m)) {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut inside = false;
    let mut current = String::new();
    let mut prev = '\0';
    for c in line.chars() {
        if c == '"' && prev != '\\' {
            if inside {
                out.push(std::mem::take(&mut current));
            }
            inside = !inside;
        } else if inside {
            current.push(c);
        }
        prev = c;
    }
    out
}

#[test]
fn the_render_layer_has_no_hardcoded_text() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut findings: Vec<String> = Vec::new();

    for rel in SOURCES {
        let path = root.join(rel);
        let contents = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
        // The file's own test module does not render interface: we cut there.
        let code = match contents.find("mod tests {") {
            Some(i) => &contents[..i],
            None => &contents[..],
        };
        for (n, line) in code.lines().enumerate() {
            for lit in string_literals(line) {
                if is_allowed(&lit) || !looks_like_ui_text(&lit) {
                    continue;
                }
                findings.push(format!("{}:{}  {:?}", rel, n + 1, lit));
            }
        }
    }

    assert!(
        findings.is_empty(),
        "{} pieces of text are hardcoded in the render layer.\n\
         They must live in assets/i18n/*.ron and be used via crate::i18n::t(\"key\"):\n  {}",
        findings.len(),
        findings.join("\n  ")
    );
}

#[test]
fn the_language_catalogs_exist_in_the_repository() {
    // The catalogs are embedded with include_str!, but they must remain
    // editable files: that is what allows translating without touching code.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/i18n");
    for language in ["es", "en"] {
        let path = root.join(format!("{language}.ron"));
        assert!(
            path.exists(),
            "falta el catalogo de language {}",
            path.display()
        );
    }
}
