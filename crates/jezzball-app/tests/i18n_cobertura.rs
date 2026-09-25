//! Guardia de internacionalización.
//!
//! El objetivo de mover los textos a `assets/i18n/*.ron` se pierde en cuanto
//! alguien vuelve a escribir una cadena visible dentro del código. Este test
//! recorre las fuentes de la capa de presentación y falla si encuentra
//! literales que parezcan texto de interfaz.
//!
//! No pretende ser un analizador de Rust: busca literales con pinta de frase
//! (varias palabras, o palabras con acentos) fuera de comentarios, y mantiene
//! una lista explícita de excepciones para lo que legítimamente no es texto de
//! interfaz (rutas, nombres de variables de entorno, claves de i18n).

use std::path::Path;

/// Ficheros de presentación que no deben contener texto visible incrustado.
const FUENTES: &[&str] = &[
    "src/render/menu.rs",
    "src/render/hud.rs",
    "src/render/arena.rs",
    "src/render/mod.rs",
];

/// Fragmentos permitidos: no son texto de interfaz.
fn permitido(lit: &str) -> bool {
    // Claves de i18n: "menu.titulo", "ayuda.p1.cuerpo"...
    if lit.contains('.') && !lit.contains(' ') {
        return true;
    }
    const EXCEPCIONES: &[&str] = &[
        "OMARCHY_JEZZBALL_UI_SCALE",
        "OMARCHY_JEZZBALL_BACKEND",
        "OMARCHY_JEZZBALL_LANG",
        "{}  (BLOQUEADO)",
        "{:.0}",
        "ESTRELLAS",
    ];
    EXCEPCIONES.iter().any(|e| lit.contains(e))
}

/// ¿Este literal parece una frase de interfaz?
fn parece_texto_visible(lit: &str) -> bool {
    let limpio = lit.trim();
    if limpio.len() < 6 {
        return false;
    }
    // Acentos o eñe: inequívocamente texto en español.
    if limpio.chars().any(|c| "áéíóúñÁÉÍÓÚÑ¿¡".contains(c)) {
        return true;
    }
    // Varias palabras alfabéticas seguidas.
    let palabras: Vec<&str> = limpio
        .split_whitespace()
        .filter(|p| p.chars().filter(|c| c.is_alphabetic()).count() >= 3)
        .collect();
    palabras.len() >= 2
}

/// Extrae literales de cadena de una línea, ignorando comentarios.
fn literales(linea: &str) -> Vec<String> {
    let l = linea.trim_start();
    if l.starts_with("//") || l.starts_with("/*") || l.starts_with('*') {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut dentro = false;
    let mut actual = String::new();
    let mut previo = '\0';
    for c in linea.chars() {
        if c == '"' && previo != '\\' {
            if dentro {
                out.push(std::mem::take(&mut actual));
            }
            dentro = !dentro;
        } else if dentro {
            actual.push(c);
        }
        previo = c;
    }
    out
}

#[test]
fn la_capa_de_render_no_tiene_textos_incrustados() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut hallazgos: Vec<String> = Vec::new();

    for rel in FUENTES {
        let ruta = raiz.join(rel);
        let contenido = std::fs::read_to_string(&ruta)
            .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", ruta.display()));
        for (n, linea) in contenido.lines().enumerate() {
            for lit in literales(linea) {
                if permitido(&lit) || !parece_texto_visible(&lit) {
                    continue;
                }
                hallazgos.push(format!("{}:{}  {:?}", rel, n + 1, lit));
            }
        }
    }

    assert!(
        hallazgos.is_empty(),
        "hay {} textos escritos directamente en el codigo de render.\n\
         Deben vivir en assets/i18n/*.ron y usarse con crate::i18n::t(\"clave\"):\n  {}",
        hallazgos.len(),
        hallazgos.join("\n  ")
    );
}

#[test]
fn los_catalogos_de_idioma_existen_en_el_repositorio() {
    // Los catalogos se empotran con include_str!, pero deben seguir siendo
    // ficheros editables: es lo que permite traducir sin tocar codigo.
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/i18n");
    for idioma in ["es", "en"] {
        let ruta = raiz.join(format!("{idioma}.ron"));
        assert!(
            ruta.exists(),
            "falta el catalogo de idioma {}",
            ruta.display()
        );
    }
}
