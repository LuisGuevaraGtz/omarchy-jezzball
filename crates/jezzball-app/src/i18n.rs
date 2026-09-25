//! Internacionalización.
//!
//! Todos los textos visibles viven en ficheros de idioma
//! (`assets/i18n/<código>.ron`), nunca escritos dentro de la lógica. Añadir un
//! idioma nuevo es añadir un fichero: no se toca ni el motor ni las pantallas.
//!
//! Decisiones:
//!
//! * **Los catálogos se empotran en el binario** con `include_str!`. El juego
//!   debe funcionar aunque se copie el ejecutable suelto, sin depender de que
//!   `assets/` esté instalado; los ficheros siguen siendo editables en el
//!   repositorio, que es lo que importa para mantenerlos y traducirlos.
//! * **Clave ausente = se devuelve la clave**. Nunca se panica ni se deja un
//!   hueco en blanco: una traducción incompleta degrada a texto visible, y el
//!   test de cobertura obliga a completarla.
//! * El idioma se resuelve una vez al arrancar y se guarda en `OnceLock`.

use std::collections::HashMap;
use std::sync::OnceLock;

/// Idiomas incluidos. Para añadir uno: crear `assets/i18n/<código>.ron`,
/// añadir la entrada aquí y listo.
pub const CATALOGOS: &[(&str, &str)] = &[
    ("es", include_str!("../../../assets/i18n/es.ron")),
    ("en", include_str!("../../../assets/i18n/en.ron")),
];

/// Idioma por defecto si no se reconoce ninguno del entorno.
pub const IDIOMA_POR_DEFECTO: &str = "es";

static ACTIVO: OnceLock<Catalogo> = OnceLock::new();

/// Catálogo de textos de un idioma.
#[derive(Debug, Clone)]
pub struct Catalogo {
    /// Código del idioma cargado ("es", "en"...). Lo consultan los tests de
    /// cobertura y es útil para diagnóstico.
    #[allow(dead_code)]
    pub codigo: String,
    textos: HashMap<String, String>,
}

impl Catalogo {
    /// Texto de una clave. Si falta, devuelve la propia clave: visible y
    /// rastreable, en lugar de un hueco vacío o un panic.
    ///
    /// El juego usa `t()`, que devuelve `&'static str`; este método existe
    /// para consultar un catálogo concreto (comparar idiomas en los tests).
    #[allow(dead_code)]
    pub fn get<'a>(&'a self, clave: &'a str) -> &'a str {
        self.textos.get(clave).map(|s| s.as_str()).unwrap_or(clave)
    }

    #[allow(dead_code)]
    pub fn claves(&self) -> impl Iterator<Item = &String> {
        self.textos.keys()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.textos.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.textos.is_empty()
    }
}

/// Parsea un catálogo RON (`{"clave": "texto", ...}`).
pub fn parsear(codigo: &str, fuente: &str) -> Option<Catalogo> {
    let textos: HashMap<String, String> = ron::from_str(fuente).ok()?;
    Some(Catalogo {
        codigo: codigo.to_string(),
        textos,
    })
}

/// Busca el catálogo de un código de idioma concreto.
pub fn catalogo_de(codigo: &str) -> Option<Catalogo> {
    CATALOGOS
        .iter()
        .find(|(c, _)| *c == codigo)
        .and_then(|(c, fuente)| parsear(c, fuente))
}

/// Resuelve el idioma a partir del entorno, en orden de prioridad:
///
/// 1. `OMARCHY_JEZZBALL_LANG` — escotilla explícita del juego.
/// 2. `LC_ALL`, `LC_MESSAGES`, `LANG` — configuración estándar de POSIX.
/// 3. `IDIOMA_POR_DEFECTO`.
///
/// De `es_MX.UTF-8` se queda con `es`. Función pura para poder testearla sin
/// tocar el entorno del proceso.
pub fn resolver_codigo(vars: &[(&str, Option<String>)]) -> String {
    for (_, valor) in vars {
        let Some(v) = valor else { continue };
        let v = v.trim();
        if v.is_empty() || v == "C" || v == "POSIX" {
            continue;
        }
        // "es_MX.UTF-8" -> "es"
        let codigo: String = v
            .split(['_', '.', '@'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if CATALOGOS.iter().any(|(c, _)| *c == codigo) {
            return codigo;
        }
    }
    IDIOMA_POR_DEFECTO.to_string()
}

/// Lee el entorno real y resuelve el idioma.
fn codigo_del_entorno() -> String {
    let vars: Vec<(&str, Option<String>)> =
        ["OMARCHY_JEZZBALL_LANG", "LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .map(|k| (*k, std::env::var(k).ok()))
            .collect();
    resolver_codigo(&vars)
}

/// Catálogo activo, resuelto una sola vez.
pub fn activo() -> &'static Catalogo {
    ACTIVO.get_or_init(|| {
        let codigo = codigo_del_entorno();
        catalogo_de(&codigo)
            .or_else(|| catalogo_de(IDIOMA_POR_DEFECTO))
            .unwrap_or_else(|| Catalogo {
                codigo: "vacio".to_string(),
                textos: HashMap::new(),
            })
    })
}

/// Traduce una clave con el catálogo activo.
///
/// Devuelve `&'static str` porque el catálogo vive en un `OnceLock` durante
/// toda la ejecución; si la clave falta, se devuelve la propia clave (que es
/// un literal del código, también `'static`).
pub fn t(clave: &'static str) -> &'static str {
    let cat: &'static Catalogo = activo();
    match cat.textos.get(clave) {
        Some(s) => s.as_str(),
        None => clave,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todos_los_catalogos_parsean() {
        for (codigo, fuente) in CATALOGOS {
            let cat = parsear(codigo, fuente)
                .unwrap_or_else(|| panic!("el catalogo '{codigo}' no parsea"));
            assert!(!cat.is_empty(), "el catalogo '{codigo}' esta vacio");
        }
    }

    #[test]
    fn todos_los_idiomas_tienen_las_mismas_claves() {
        // Una traduccion incompleta deja texto en el idioma equivocado (o la
        // clave cruda) delante del jugador. Se compara contra el idioma de
        // referencia para que no pase inadvertido.
        let referencia = catalogo_de(IDIOMA_POR_DEFECTO).expect("catalogo de referencia");
        for (codigo, _) in CATALOGOS {
            if *codigo == IDIOMA_POR_DEFECTO {
                continue;
            }
            let otro = catalogo_de(codigo).expect("catalogo");
            let faltan: Vec<&String> = referencia
                .claves()
                .filter(|k| otro.get(k) == k.as_str())
                .collect();
            assert!(
                faltan.is_empty(),
                "al idioma '{codigo}' le faltan {} claves: {:?}",
                faltan.len(),
                &faltan[..faltan.len().min(10)]
            );
            let sobran: Vec<&String> = otro
                .claves()
                .filter(|k| referencia.get(k) == k.as_str())
                .collect();
            assert!(
                sobran.is_empty(),
                "el idioma '{codigo}' tiene claves que no existen en '{IDIOMA_POR_DEFECTO}': {:?}",
                &sobran[..sobran.len().min(10)]
            );
        }
    }

    #[test]
    fn clave_desconocida_devuelve_la_propia_clave() {
        let cat = catalogo_de("es").unwrap();
        assert_eq!(cat.get("no.existe.esta.clave"), "no.existe.esta.clave");
    }

    #[test]
    fn resuelve_el_idioma_del_entorno() {
        let v = |s: &str| vec![("LANG", Some(s.to_string()))];
        assert_eq!(resolver_codigo(&v("es_MX.UTF-8")), "es");
        assert_eq!(resolver_codigo(&v("en_US.UTF-8")), "en");
        assert_eq!(resolver_codigo(&v("en")), "en");
        // Idioma sin catalogo -> por defecto.
        assert_eq!(resolver_codigo(&v("fr_FR.UTF-8")), IDIOMA_POR_DEFECTO);
        // "C"/"POSIX" no son idiomas reales.
        assert_eq!(resolver_codigo(&v("C")), IDIOMA_POR_DEFECTO);
        assert_eq!(resolver_codigo(&[]), IDIOMA_POR_DEFECTO);
    }

    #[test]
    fn la_escotilla_del_juego_gana_al_locale_del_sistema() {
        let vars = vec![
            ("OMARCHY_JEZZBALL_LANG", Some("en".to_string())),
            ("LANG", Some("es_MX.UTF-8".to_string())),
        ];
        assert_eq!(resolver_codigo(&vars), "en");
    }
}
