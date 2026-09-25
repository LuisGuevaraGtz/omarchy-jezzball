//! Internacionalización.
//!
//! Todos los texts visible viven en ficheros de language
//! (`assets/i18n/<código>.ron`), nunca escritos inside de la lógica. Añadir un
//! language nuevo es añadir un fichero: no se toca ni el motor ni las pantallas.
//!
//! Decisiones:
//!
//! * **Los catálogos se empotran en el binario** con `include_str!`. El juego
//!   debe funcionar aunque se copie el ejecutable suelto, sin depender de que
//!   `assets/` esté instalado; los ficheros siguen siendo editables en el
//!   repositorio, que es lo que importa para mantenerlos y traducirlos.
//! * **Clave ausente = se devuelve la key**. Nunca se panica ni se deja un
//!   hueco en blanco: una traducción incompleta degrada a texto visible, y el
//!   test de cobertura obliga a completarla.
//! * El language se resuelve una vez al arrancar y se guarda en `OnceLock`.

use std::collections::HashMap;
use std::sync::OnceLock;

/// Idiomas incluidos. Para añadir uno: crear `assets/i18n/<código>.ron`,
/// añadir la entrada aquí y listo.
pub const CATALOGS: &[(&str, &str)] = &[
    ("es", include_str!("../../../assets/i18n/es.ron")),
    ("en", include_str!("../../../assets/i18n/en.ron")),
];

/// Idioma por defecto si no se reconoce ninguno del entorno.
pub const DEFAULT_LANGUAGE: &str = "es";

static ACTIVO: OnceLock<Catalog> = OnceLock::new();

/// Catálogo de texts de un language.
#[derive(Debug, Clone)]
pub struct Catalog {
    /// Código del language cargado ("es", "en"...). Lo consultan los tests de
    /// cobertura y es útil para diagnóstico.
    #[allow(dead_code)]
    pub code: String,
    texts: HashMap<String, String>,
}

impl Catalog {
    /// Texto de una key. Si falta, devuelve la propia key: visible y
    /// rastreable, en lugar de un hueco vacío o un panic.
    ///
    /// El juego usa `t()`, que devuelve `&'static str`; este método existe
    /// para consultar un catálogo concreto (comparar idiomas en los tests).
    #[allow(dead_code)]
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        self.texts.get(key).map(|s| s.as_str()).unwrap_or(key)
    }

    #[allow(dead_code)]
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.texts.keys()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.texts.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.texts.is_empty()
    }
}

/// Parsea un catálogo RON (`{"key": "texto", ...}`).
pub fn parse(code: &str, fuente: &str) -> Option<Catalog> {
    let texts: HashMap<String, String> = ron::from_str(fuente).ok()?;
    Some(Catalog {
        code: code.to_string(),
        texts,
    })
}

/// Busca el catálogo de un código de language concreto.
pub fn catalog_for(code: &str) -> Option<Catalog> {
    CATALOGS
        .iter()
        .find(|(c, _)| *c == code)
        .and_then(|(c, fuente)| parse(c, fuente))
}

/// Resuelve el language a partir del entorno, en orden de prioridad:
///
/// 1. `OMARCHY_JEZZBALL_LANG` — escotilla explícita del juego.
/// 2. `LC_ALL`, `LC_MESSAGES`, `LANG` — configuración estándar de POSIX.
/// 3. `DEFAULT_LANGUAGE`.
///
/// De `es_MX.UTF-8` se queda con `es`. Función pura para poder testearla sin
/// tocar el entorno del proceso.
pub fn resolve_code(vars: &[(&str, Option<String>)]) -> String {
    for (_, valor) in vars {
        let Some(v) = valor else { continue };
        let v = v.trim();
        if v.is_empty() || v == "C" || v == "POSIX" {
            continue;
        }
        // "es_MX.UTF-8" -> "es"
        let code: String = v
            .split(['_', '.', '@'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if CATALOGS.iter().any(|(c, _)| *c == code) {
            return code;
        }
    }
    DEFAULT_LANGUAGE.to_string()
}

/// Lee el entorno real y resuelve el language.
fn code_from_env() -> String {
    let vars: Vec<(&str, Option<String>)> =
        ["OMARCHY_JEZZBALL_LANG", "LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .map(|k| (*k, std::env::var(k).ok()))
            .collect();
    resolve_code(&vars)
}

/// Catálogo active, resuelto una sola vez.
pub fn active() -> &'static Catalog {
    ACTIVO.get_or_init(|| {
        let code = code_from_env();
        catalog_for(&code)
            .or_else(|| catalog_for(DEFAULT_LANGUAGE))
            .unwrap_or_else(|| Catalog {
                code: "vacio".to_string(),
                texts: HashMap::new(),
            })
    })
}

/// Traduce una key con el catálogo active.
///
/// Devuelve `&'static str` porque el catálogo vive en un `OnceLock` durante
/// toda la ejecución; si la key falta, se devuelve la propia key (que es
/// un literal del código, también `'static`).
pub fn t(key: &'static str) -> &'static str {
    let cat: &'static Catalog = active();
    match cat.texts.get(key) {
        Some(s) => s.as_str(),
        None => key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_catalog_parses() {
        for (code, fuente) in CATALOGS {
            let cat =
                parse(code, fuente).unwrap_or_else(|| panic!("el catalogo '{code}' no parsea"));
            assert!(!cat.is_empty(), "el catalogo '{code}' esta vacio");
        }
    }

    #[test]
    fn every_language_has_the_same_keys() {
        // Una traduccion incompleta deja texto en el language equivocado (o la
        // key cruda) delante del jugador. Se compara contra el language de
        // reference para que no pase inadvertido.
        let reference = catalog_for(DEFAULT_LANGUAGE).expect("catalogo de reference");
        for (code, _) in CATALOGS {
            if *code == DEFAULT_LANGUAGE {
                continue;
            }
            let other = catalog_for(code).expect("catalogo");
            let missing: Vec<&String> = reference
                .keys()
                .filter(|k| other.get(k) == k.as_str())
                .collect();
            assert!(
                missing.is_empty(),
                "al language '{code}' le missing {} keys: {:?}",
                missing.len(),
                &missing[..missing.len().min(10)]
            );
            let extra: Vec<&String> = other
                .keys()
                .filter(|k| reference.get(k) == k.as_str())
                .collect();
            assert!(
                extra.is_empty(),
                "el language '{code}' tiene keys que no existen en '{DEFAULT_LANGUAGE}': {:?}",
                &extra[..extra.len().min(10)]
            );
        }
    }

    #[test]
    fn unknown_key_returns_the_key_itself() {
        let cat = catalog_for("es").unwrap();
        assert_eq!(cat.get("no.existe.esta.key"), "no.existe.esta.key");
    }

    #[test]
    fn resolves_the_language_from_the_env() {
        let v = |s: &str| vec![("LANG", Some(s.to_string()))];
        assert_eq!(resolve_code(&v("es_MX.UTF-8")), "es");
        assert_eq!(resolve_code(&v("en_US.UTF-8")), "en");
        assert_eq!(resolve_code(&v("en")), "en");
        // Idioma sin catalogo -> por defecto.
        assert_eq!(resolve_code(&v("fr_FR.UTF-8")), DEFAULT_LANGUAGE);
        // "C"/"POSIX" no son idiomas reales.
        assert_eq!(resolve_code(&v("C")), DEFAULT_LANGUAGE);
        assert_eq!(resolve_code(&[]), DEFAULT_LANGUAGE);
    }

    #[test]
    fn the_game_override_wins_over_the_system_locale() {
        let vars = vec![
            ("OMARCHY_JEZZBALL_LANG", Some("en".to_string())),
            ("LANG", Some("es_MX.UTF-8".to_string())),
        ];
        assert_eq!(resolve_code(&vars), "en");
    }
}
