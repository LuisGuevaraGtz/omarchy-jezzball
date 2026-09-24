//! Persistencia en disco (ARCHITECTURE.md §11).
//!
//! Guardado en `$XDG_DATA_HOME/omarchy-jezzball/save.ron` (fallback
//! `~/.local/share/omarchy-jezzball/save.ron`) con escritura ATÓMICA:
//! primero a `save.ron.tmp` y luego `rename`. Cero `unwrap()`/`expect()` en
//! rutas de I/O: todo falla y se degrada a un `SaveData` limpio, nunca panic.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use jezzball_core::level::Mode;
use serde::{Deserialize, Serialize};

/// Versión del formato de guardado. Un fichero con versión distinta se
/// considera incompatible: se respalda y se empieza limpio.
pub const SAVE_VERSION: u32 = 1;

/// Récord de un nivel concreto en un modo.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct LevelRecord {
    pub best_score: u32,
    pub stars: u8,
    pub completed: bool,
    pub best_time: f32,
}

impl Default for LevelRecord {
    fn default() -> Self {
        LevelRecord {
            best_score: 0,
            stars: 0,
            completed: false,
            best_time: 0.0,
        }
    }
}

/// Todo el progreso persistido del juego.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SaveData {
    pub version: u32,
    pub original: BTreeMap<u16, LevelRecord>,
    pub enhanced: BTreeMap<u16, LevelRecord>,
    pub original_completed: bool,
}

impl Default for SaveData {
    fn default() -> Self {
        SaveData {
            version: SAVE_VERSION,
            original: BTreeMap::new(),
            enhanced: BTreeMap::new(),
            original_completed: false,
        }
    }
}

impl SaveData {
    /// Registro de un nivel, si existe.
    pub fn record(&self, mode: Mode, id: u16) -> Option<&LevelRecord> {
        self.map(mode).get(&id)
    }

    /// Registro de un nivel, creándolo si no existe (mutable).
    pub fn record_for(&mut self, mode: Mode, id: u16) -> &mut LevelRecord {
        self.map_mut(mode).entry(id).or_default()
    }

    /// Nº de niveles completados en un modo.
    pub fn completed_count(&self, mode: Mode) -> usize {
        self.map(mode).values().filter(|r| r.completed).count()
    }

    /// Total de estrellas acumuladas en un modo.
    pub fn total_stars(&self, mode: Mode) -> u32 {
        self.map(mode)
            .values()
            .map(|r| u32::from(r.stars))
            .sum()
    }

    fn map(&self, mode: Mode) -> &BTreeMap<u16, LevelRecord> {
        match mode {
            Mode::Original => &self.original,
            Mode::Enhanced => &self.enhanced,
        }
    }

    fn map_mut(&mut self, mode: Mode) -> &mut BTreeMap<u16, LevelRecord> {
        match mode {
            Mode::Original => &mut self.original,
            Mode::Enhanced => &mut self.enhanced,
        }
    }
}

/// Resuelve una variable XDG a una ruta absoluta, o `None` si no está o no es
/// absoluta.
pub fn xdg_dir(var: &str) -> Option<PathBuf> {
    let raw = std::env::var_os(var)?;
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        Some(p)
    } else {
        None
    }
}

/// Directorio home del usuario (best-effort).
pub fn home_dir() -> PathBuf {
    match std::env::var_os("HOME") {
        Some(h) => {
            let p = PathBuf::from(h);
            if p.is_absolute() {
                return p;
            }
            PathBuf::from(".")
        }
        None => PathBuf::from("."),
    }
}

pub fn data_home_dir() -> PathBuf {
    xdg_dir("XDG_DATA_HOME").unwrap_or_else(|| home_dir().join(".local/share"))
}

pub fn config_home_dir() -> PathBuf {
    xdg_dir("XDG_CONFIG_HOME").unwrap_or_else(|| home_dir().join(".config"))
}

pub fn state_home_dir() -> PathBuf {
    xdg_dir("XDG_STATE_HOME").unwrap_or_else(|| home_dir().join(".local/state"))
}

/// Ruta real del fichero de guardado.
pub fn save_file_path() -> PathBuf {
    save_file_path_in(&data_home_dir())
}

/// Ruta del guardado relativa a un `data_home` (útil para tests).
pub fn save_file_path_in(data_home: &Path) -> PathBuf {
    data_home.join("omarchy-jezzball").join("save.ron")
}

/// Parsea y valida el contenido de un guardado. `Err` si el RON no es válido o
/// la versión es incompatible.
pub fn parse_save(content: &str) -> Result<SaveData, String> {
    let data: SaveData = ron::from_str(content).map_err(|e| format!("RON inválido: {e}"))?;
    if data.version != SAVE_VERSION {
        return Err(format!(
            "versión {} incompatible (esperada {SAVE_VERSION})",
            data.version
        ));
    }
    Ok(data)
}

/// Carga el guardado desde la ruta por defecto XDG.
pub fn load_save() -> SaveData {
    load_save_at(&save_file_path())
}

/// Carga un guardado de la ruta indicada. Si el fichero no existe, devuelve un
/// `SaveData` limpio. Si existe pero está corrupto o es de otra versión, hace
/// backup a `.ron.bak` y devuelve uno limpio. Nunca panic.
pub fn load_save_at(path: &Path) -> SaveData {
    let Ok(content) = fs::read_to_string(path) else {
        return SaveData::default();
    };
    match parse_save(&content) {
        Ok(data) => data,
        Err(_) => {
            backup_file(path);
            SaveData::default()
        }
    }
}

/// Copia el fichero corrupto a `.ron.bak` (best-effort, sin errores).
fn backup_file(path: &Path) {
    if let Ok(content) = fs::read_to_string(path) {
        let bak = path.with_extension("ron.bak");
        let _ = fs::write(&bak, content);
    }
}

/// Guarda el progreso en la ruta XDG por defecto. Devuelve `true` si todo OK.
pub fn save_save(data: &SaveData) -> bool {
    save_save_at(data, &save_file_path())
}

/// Escritura atómica: `save.ron.tmp` + `rename`. Devuelve `false` si algo
/// falla, sin panic.
pub fn save_save_at(data: &SaveData, path: &Path) -> bool {
    let Ok(content) = ron::to_string(data) else {
        return false;
    };
    if let Some(dir) = path.parent() {
        if fs::create_dir_all(dir).is_err() {
            return false;
        }
    }
    let tmp = path.with_extension("ron.tmp");
    if fs::write(&tmp, content).is_err() {
        return false;
    }
    fs::rename(&tmp, path).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_root(name: &str) -> PathBuf {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/.tmp")
            .join(format!("persist-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("crear tmp");
        root
    }

    fn sample() -> SaveData {
        let mut s = SaveData::default();
        let r = s.record_for(Mode::Original, 1);
        r.best_score = 1200;
        r.stars = 2;
        r.completed = true;
        r.best_time = 61.0;
        s
    }

    #[test]
    fn roundtrip_ron() {
        let s = sample();
        let text = ron::to_string(&s).unwrap();
        let parsed: SaveData = ron::from_str(&text).unwrap();
        assert_eq!(parsed, s);
        assert_eq!(parsed.record(Mode::Original, 1), s.record(Mode::Original, 1));
        assert_eq!(parsed.completed_count(Mode::Original), 1);
        assert_eq!(parsed.total_stars(Mode::Original), 2);
    }

    #[test]
    fn version_incompatible_se_rechaza() {
        let stale = "(version: 99, original: {}, enhanced: {}, original_completed: false)";
        assert!(parse_save(stale).is_err());
        let good = "(version: 1, original: {}, enhanced: {}, original_completed: false)";
        assert!(parse_save(good).is_ok());
        // Garbage
        assert!(parse_save("no soy ron <<").is_err());
    }

    #[test]
    fn fichero_inexistente_devuelve_limpio() {
        let root = tmp_root("missing");
        let p = save_file_path_in(&root);
        assert_eq!(load_save_at(&p).version, SAVE_VERSION);
        assert_eq!(load_save_at(&p).original_completed, false);
    }

    #[test]
    fn guardar_y_cargar_atomico() {
        let root = tmp_root("atomic");
        let p = save_file_path_in(&root);
        assert!(save_save_at(&sample(), &p), "guarda correctamente");
        assert!(p.exists(), "fichero final presente");
        let loaded = load_save_at(&p);
        assert_eq!(loaded, sample());
        // No deben quedar temporales.
        let tmp = p.with_extension("ron.tmp");
        assert!(!tmp.exists(), "el temporal se renombra, no se queda");
    }

    #[test]
    fn corrupto_hace_backup_y_empieza_limpio() {
        let root = tmp_root("corrupt");
        let p = save_file_path_in(&root);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, "basura incomparable").unwrap();
        let loaded = load_save_at(&p);
        assert_eq!(loaded, SaveData::default());
        let bak = p.with_extension("ron.bak");
        assert!(bak.exists(), "el backup existe");
        assert_eq!(fs::read_to_string(&bak).unwrap(), "basura incomparable");
    }

    #[test]
    fn version_vieja_hace_backup() {
        let root = tmp_root("oldver");
        let p = save_file_path_in(&root);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, "(version: 0, original: {}, enhanced: {}, original_completed: false)").unwrap();
        let loaded = load_save_at(&p);
        assert_eq!(loaded, SaveData::default());
        assert!(p.with_extension("ron.bak").exists());
    }
}