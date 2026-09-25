//! On-disk persistence (ARCHITECTURE.md §11).
//!
//! Saved to `$XDG_DATA_HOME/omarchy-jezzball/save.ron` (fallback
//! `~/.local/share/omarchy-jezzball/save.ron`) with ATOMIC writes:
//! first to `save.ron.tmp` and then `rename`. Zero `unwrap()`/`expect()` on
//! I/O paths: everything can fail and degrades to a trimmed `SaveData`, never a panic.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use jezzball_core::level::Mode;
use serde::{Deserialize, Serialize};

/// Version of the save format. A file with a different version is
/// considered incompatible: it is backed up and we start trimmed.
pub const SAVE_VERSION: u32 = 1;

/// Record for a specific level in a mode.
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

/// All the game's persisted progress.
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
    /// A level's record, if it exists.
    pub fn record(&self, mode: Mode, id: u16) -> Option<&LevelRecord> {
        self.map(mode).get(&id)
    }

    /// A level's record, creating it if it does not exist (mutable).
    pub fn record_for(&mut self, mode: Mode, id: u16) -> &mut LevelRecord {
        self.map_mut(mode).entry(id).or_default()
    }

    /// Number of levels completed in a mode.
    pub fn completed_count(&self, mode: Mode) -> usize {
        self.map(mode).values().filter(|r| r.completed).count()
    }

    /// Total stars accumulated in a mode.
    pub fn total_stars(&self, mode: Mode) -> u32 {
        self.map(mode).values().map(|r| u32::from(r.stars)).sum()
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

/// Resolves an XDG variable to an absolute path, or `None` if it is unset or not
/// absolute.
pub fn xdg_dir(var: &str) -> Option<PathBuf> {
    let raw = std::env::var_os(var)?;
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        Some(p)
    } else {
        None
    }
}

/// The user's home directory (best-effort).
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

/// The user's data directory (`$XDG_DATA_HOME`, or `~/.local/share`).
///
/// In TEST builds it is redirected to a temporary directory inside the
/// repo itself: a test that builds the whole application must not read nor
/// —above all— WRITE the user's real save file. (It happened: a progression
/// fuzz left `u32::MAX` in the records of a real save file.)
#[cfg(not(test))]
pub fn data_home_dir() -> PathBuf {
    xdg_dir("XDG_DATA_HOME").unwrap_or_else(|| home_dir().join(".local/share"))
}

/// Test variant: an isolated temporary directory, never the real save file.
#[cfg(test)]
pub fn data_home_dir() -> PathBuf {
    std::env::temp_dir().join("omarchy-jezzball-test-data")
}

pub fn config_home_dir() -> PathBuf {
    xdg_dir("XDG_CONFIG_HOME").unwrap_or_else(|| home_dir().join(".config"))
}

pub fn state_home_dir() -> PathBuf {
    xdg_dir("XDG_STATE_HOME").unwrap_or_else(|| home_dir().join(".local/state"))
}

/// Real path of the save file.
pub fn save_file_path() -> PathBuf {
    save_file_path_in(&data_home_dir())
}

/// Path of the save file relative to a `data_home` (useful for tests).
pub fn save_file_path_in(data_home: &Path) -> PathBuf {
    data_home.join("omarchy-jezzball").join("save.ron")
}

/// Parses and validates the contents of a save file. `Err` if the RON is invalid or
/// the version is incompatible.
pub fn parse_save(content: &str) -> Result<SaveData, String> {
    let data: SaveData = ron::from_str(content).map_err(|e| format!("invalid RON: {e}"))?;
    if data.version != SAVE_VERSION {
        return Err(format!(
            "incompatible version {} (expected {SAVE_VERSION})",
            data.version
        ));
    }
    Ok(data)
}

/// Loads the save file from the default XDG path.
pub fn load_save() -> SaveData {
    load_save_at(&save_file_path())
}

/// Loads a save file from the given path. If the file does not exist, it returns a
/// trimmed `SaveData`. If it exists but is corrupt or from another version, it makes
/// a backup to `.ron.bak` and returns a trimmed one. Never panics.
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

/// Copies the corrupt file to `.ron.bak` (best-effort, no errors).
fn backup_file(path: &Path) {
    if let Ok(content) = fs::read_to_string(path) {
        let bak = path.with_extension("ron.bak");
        let _ = fs::write(&bak, content);
    }
}

/// Saves the progress to the default XDG path. Returns `true` if everything is OK.
pub fn save_save(data: &SaveData) -> bool {
    save_save_at(data, &save_file_path())
}

/// Atomic write: `save.ron.tmp` + `rename`. Returns `false` if anything
/// fails, without panicking.
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
        assert_eq!(
            parsed.record(Mode::Original, 1),
            s.record(Mode::Original, 1)
        );
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
    fn missing_file_returns_a_clean_save() {
        let root = tmp_root("missing");
        let p = save_file_path_in(&root);
        assert_eq!(load_save_at(&p).version, SAVE_VERSION);
        assert!(!load_save_at(&p).original_completed);
    }

    #[test]
    fn saving_and_loading_is_atomic() {
        let root = tmp_root("atomic");
        let p = save_file_path_in(&root);
        assert!(save_save_at(&sample(), &p), "guarda correctamente");
        assert!(p.exists(), "final file present");
        let loaded = load_save_at(&p);
        assert_eq!(loaded, sample());
        // No temporary files must be left behind.
        let tmp = p.with_extension("ron.tmp");
        assert!(!tmp.exists(), "el temporal se renombra, no se queda");
    }

    #[test]
    fn corrupt_save_is_backed_up_and_starts_clean() {
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
        fs::write(
            &p,
            "(version: 0, original: {}, enhanced: {}, original_completed: false)",
        )
        .unwrap();
        let loaded = load_save_at(&p);
        assert_eq!(loaded, SaveData::default());
        assert!(p.with_extension("ron.bak").exists());
    }
}
