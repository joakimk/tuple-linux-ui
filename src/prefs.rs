//! UI-only preferences stored under `$XDG_CONFIG_HOME/tuple-ui/prefs.json`.
//!
//! Separate from Tuple's own settings (`tuple settings` / `tuple set`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub show_window_on_start: bool,
    pub start_in_tray: bool,
    pub notify_incoming_calls: bool,
    pub notify_missed_calls: bool,
    pub last_tab: String,
    pub window_width: f32,
    pub window_height: f32,
    #[serde(skip)]
    path: PathBuf,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            show_window_on_start: true,
            start_in_tray: false,
            notify_incoming_calls: true,
            notify_missed_calls: true,
            last_tab: "people".into(),
            window_width: 420.0,
            window_height: 640.0,
            path: default_path(),
        }
    }
}

pub fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn default_path() -> PathBuf {
    config_dir().join("tuple-ui").join("prefs.json")
}

impl Prefs {
    pub fn load() -> Prefs {
        Prefs::load_from(default_path())
    }

    pub fn load_from(path: PathBuf) -> Prefs {
        let mut prefs = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Prefs>(&text).ok())
            .unwrap_or_default();
        prefs.path = path;
        prefs
    }

    pub fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self).expect("prefs serialize");
        std::fs::write(&self.path, text + "\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tuple-ui-prefs-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("tuple-ui").join("prefs.json")
    }

    #[test]
    fn defaults_when_no_file() {
        let prefs = Prefs::load_from(temp_path("missing"));
        assert!(prefs.show_window_on_start);
        assert!(prefs.notify_incoming_calls);
        assert!(!prefs.start_in_tray);
        assert_eq!(prefs.last_tab, "people");
    }

    #[test]
    fn round_trip_persists() {
        let path = temp_path("roundtrip");
        let mut prefs = Prefs::load_from(path.clone());
        prefs.show_window_on_start = false;
        prefs.last_tab = "rooms".into();
        prefs.save().unwrap();
        assert!(path.exists());
        let reloaded = Prefs::load_from(path);
        assert!(!reloaded.show_window_on_start);
        assert_eq!(reloaded.last_tab, "rooms");
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let path = temp_path("corrupt");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{not json").unwrap();
        assert!(Prefs::load_from(path).show_window_on_start);
    }
}
