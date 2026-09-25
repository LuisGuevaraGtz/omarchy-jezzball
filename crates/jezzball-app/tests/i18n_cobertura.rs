//! Guardia de internacionalización.
//!
//! El objetivo de mover los texts a `assets/i18n/*.ron` se pierde en cuanto
//! alguien vuelve a escribir una cadena visible inside del código. Este test
//! recorre las fuentes de la capa de presentación y falla si encuentra
//! string_literals que parezcan texto de interfaz.
//!
//! No pretende ser un analizador de Rust: busca string_literals con pinta de frase
//! (varias words, o words con acentos) fuera de comentarios, y mantiene
//! una lista explícita de excepciones para lo que legítimamente no es texto de
//! interfaz (rutas, nombres de variables de entorno, keys de i18n).

use std::path::Path;

/// Ficheros que no deben contener texto visible incrustado.
///
/// Incluye `screens.rs` además de la capa de render: ahí se construyen las
/// etiquetas de los menús (`MenuItem`), y por dejarlo fuera se coló un
/// "MODO ENHANCED" sin traducir que el jugador vio en pantalla.
const SOURCES: &[&str] = &[
    "src/render/menu.rs",
    "src/render/hud.rs",
    "src/render/arena.rs",
    "src/render/mod.rs",
    "src/screens.rs",
];

/// Fragmentos permitidos: no son texto de interfaz.
fn is_allowed(lit: &str) -> bool {
    // Claves de i18n: "menu.title", "ayuda.p1.body"...
    if lit.contains('.') && !lit.contains(' ') {
        return true;
    }
    const EXCEPTIONS: &[&str] = &[
        "OMARCHY_JEZZBALL_UI_SCALE",
        "OMARCHY_JEZZBALL_BACKEND",
        "OMARCHY_JEZZBALL_LANG",
        "{}  (BLOQUEADO)",
        "{:.0}",
        "ESTRELLAS",
    ];
    EXCEPTIONS.iter().any(|e| lit.contains(e))
}

/// ¿Este literal parece una frase de interfaz?
fn looks_like_ui_text(lit: &str) -> bool {
    let trimmed = lit.trim();
    if trimmed.len() < 6 {
        return false;
    }
    // Acentos o eñe: inequívocamente texto en español.
    if trimmed.chars().any(|c| "áéíóúñÁÉÍÓÚÑ¿¡".contains(c)) {
        return true;
    }
    // Varias words alfabéticas seguidas.
    let words: Vec<&str> = trimmed
        .split_whitespace()
        .filter(|p| p.chars().filter(|c| c.is_alphabetic()).count() >= 3)
        .collect();
    words.len() >= 2
}

/// Extrae string_literals de cadena de una línea, ignorando comentarios.
///
/// También ignora las líneas que claramente no pintan interfaz: mensajes de
/// aserción de los tests, diagnóstico por consola (`eprintln!`) y cadenas de
/// error internas. Lo que se persigue es el texto que ve el jugador.
fn string_literals(line: &str) -> Vec<String> {
    let l = line.trim_start();
    if l.starts_with("//") || l.starts_with("/*") || l.starts_with('*') {
        return Vec::new();
    }
    // Mensajes que nunca llegan a la interfaz del juego.
    const NOT_UI: &[&str] = &[
        "assert",
        "panic!",
        "expect(",
        "unwrap_or_else",
        "eprintln!",
        "println!",
        "#[test]",
        "debug_assert",
        // Cadenas de error internas (`Result<_, String>`): son diagnóstico
        // técnico para el log, no texto que el jugador lea en pantalla. Lo que
        // sí ve (`app.error_msg`) pasa por el catálogo.
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
            .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", path.display()));
        // El módulo de tests del propio fichero no pinta interfaz: se corta ahí.
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
        "hay {} texts escritos directamente en el code de render.\n\
         Deben vivir en assets/i18n/*.ron y usarse con crate::i18n::t(\"key\"):\n  {}",
        findings.len(),
        findings.join("\n  ")
    );
}

#[test]
fn the_language_catalogs_exist_in_the_repository() {
    // Los catalogos se empotran con include_str!, pero deben seguir siendo
    // ficheros editables: es lo que permite traducir sin tocar code.
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
