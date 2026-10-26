//! Launch-at-login via a freedesktop autostart entry in `$XDG_CONFIG_HOME/autostart`.

use std::path::{Path, PathBuf};

use crate::prefs::config_dir;

const ENTRY: &str = "[Desktop Entry]\n\
Type=Application\n\
Name=Tuple UI (unofficial)\n\
Comment=Tuple pair programming client\n\
Exec=tuple-ui --tray\n\
Icon=tuple\n\
Terminal=false\n\
X-GNOME-Autostart-enabled=true\n";

pub fn autostart_path(config: &Path) -> PathBuf {
    config.join("autostart").join("tuple-ui.desktop")
}

pub fn is_enabled() -> bool {
    autostart_path(&config_dir()).exists()
}

pub fn set_enabled(enabled: bool) -> std::io::Result<()> {
    set_enabled_in(&config_dir(), enabled)
}

pub fn set_enabled_in(config: &Path, enabled: bool) -> std::io::Result<()> {
    let path = autostart_path(config);
    if enabled {
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(path, ENTRY)
    } else if path.exists() {
        std::fs::remove_file(path)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_and_removes_the_desktop_entry() {
        let dir = std::env::temp_dir().join(format!("tuple-ui-autostart-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        set_enabled_in(&dir, true).unwrap();
        let text = std::fs::read_to_string(autostart_path(&dir)).unwrap();
        assert!(text.contains("Exec=tuple-ui --tray"));
        set_enabled_in(&dir, false).unwrap();
        assert!(!autostart_path(&dir).exists());
        set_enabled_in(&dir, false).unwrap(); // idempotent
        let _ = std::fs::remove_dir_all(&dir);
    }
}
