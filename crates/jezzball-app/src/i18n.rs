//! Internationalization.
//!
//! All visible text lives in language files
//! (`assets/i18n/<code>.ron`), never written inside the logic. Adding a
//! new language means adding a file: neither the engine nor the screens are touched.
//!
//! Decisions:
//!
//! * **The catalogs are embedded in the binary** with `include_str!`. The game
//!   must work even if the executable is copied on its own, without depending on
//!   `assets/` being installed; the files remain editable in the
//!   repository, which is what matters for maintaining and translating them.
//! * **Missing key = the key is returned**. It never panics and never leaves a
//!   blank gap: an incomplete translation degrades to visible text, and the
//!   coverage test forces it to be completed.
//! * The language is resolved once at startup and stored in a `OnceLock`.

use std::collections::HashMap;
use std::sync::OnceLock;

/// Included languages. To add one: create `assets/i18n/<code>.ron`,
/// add the entry here and you are done.
pub const CATALOGS: &[(&str, &str)] = &[
    ("es", include_str!("../../../assets/i18n/es.ron")),
    ("en", include_str!("../../../assets/i18n/en.ron")),
];

/// Default language if none is recognised from the environment.
pub const DEFAULT_LANGUAGE: &str = "es";

static ACTIVO: OnceLock<Catalog> = OnceLock::new();

/// Text catalog for a language.
#[derive(Debug, Clone)]
pub struct Catalog {
    /// Code of the loaded language ("es", "en"...). The coverage tests query it
    /// and it is useful for diagnostics.
    #[allow(dead_code)]
    pub code: String,
    texts: HashMap<String, String>,
}

impl Catalog {
    /// Text for a key. If it is missing, it returns the key itself: visible and
    /// traceable, instead of an empty gap or a panic.
    ///
    /// The game uses `t()`, which returns `&'static str`; this method exists
    /// to query a specific catalog (comparing languages in the tests).
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

/// Parses a RON catalog (`{"key": "texto", ...}`).
pub fn parse(code: &str, fuente: &str) -> Option<Catalog> {
    let texts: HashMap<String, String> = ron::from_str(fuente).ok()?;
    Some(Catalog {
        code: code.to_string(),
        texts,
    })
}

/// Looks up the catalog for a specific language code.
pub fn catalog_for(code: &str) -> Option<Catalog> {
    CATALOGS
        .iter()
        .find(|(c, _)| *c == code)
        .and_then(|(c, fuente)| parse(c, fuente))
}

/// Resolves the language from the environment, in priority order:
///
/// 1. `OMARCHY_JEZZBALL_LANG` — the game's explicit override.
/// 2. `LC_ALL`, `LC_MESSAGES`, `LANG` — standard POSIX configuration.
/// 3. `DEFAULT_LANGUAGE`.
///
/// From `es_MX.UTF-8` it keeps `es`. A pure function so it can be tested without
/// touching the process environment.
pub fn resolve_code(vars: &[(&str, Option<String>)]) -> String {
    for (_, value) in vars {
        let Some(v) = value else { continue };
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

/// Reads the real environment and resolves the language.
fn code_from_env() -> String {
    let vars: Vec<(&str, Option<String>)> =
        ["OMARCHY_JEZZBALL_LANG", "LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .map(|k| (*k, std::env::var(k).ok()))
            .collect();
    resolve_code(&vars)
}

/// Active catalog, resolved only once.
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

/// Translates a key with the active catalog.
///
/// It returns `&'static str` because the catalog lives in a `OnceLock` for
/// the whole execution; if the key is missing, the key itself is returned (which is
/// a literal from the code, also `'static`).
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
                parse(code, fuente).unwrap_or_else(|| panic!("catalog '{code}' does not parse"));
            assert!(!cat.is_empty(), "catalog '{code}' is empty");
        }
    }

    #[test]
    fn every_language_has_the_same_keys() {
        // An incomplete translation leaves text in the wrong language (or the
        // raw key) in front of the player. It is compared against the reference
        // language so that it does not go unnoticed.
        let reference = catalog_for(DEFAULT_LANGUAGE).expect("catalogo de reference");
        for (code, _) in CATALOGS {
            if *code == DEFAULT_LANGUAGE {
                continue;
            }
            let other = catalog_for(code).expect("catalog");
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
                "language '{code}' has keys that do not exist in '{DEFAULT_LANGUAGE}': {:?}",
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
        // Language with no catalog -> default.
        assert_eq!(resolve_code(&v("fr_FR.UTF-8")), DEFAULT_LANGUAGE);
        // "C"/"POSIX" are not real languages.
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
