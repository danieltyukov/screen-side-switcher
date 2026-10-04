//! Screen Side 1.0 was a Python package installed into ~/.local. Its
//! launchers sit early on PATH and its menu entry stays in the menu, so a
//! 2.0 package would keep starting 1.0. Each file is removed only when it
//! is recognisably 1.0's; site/public/install.sh applies the same rules.

use std::fs;
use std::path::{Path, PathBuf};

const ENTRY: &str = ".local/share/applications/io.github.danieltyukov.ScreenSide.desktop";
const ICON: &str = ".local/share/icons/hicolor/scalable/apps/io.github.danieltyukov.ScreenSide.svg";

fn contains(path: &Path, needle: &str) -> bool {
    fs::read(path)
        .map(|bytes| String::from_utf8_lossy(&bytes).contains(needle))
        .unwrap_or(false)
}

/// Removes a 1.0 install from `home` and says what went.
pub fn cleanup_v1(home: &Path) -> Vec<PathBuf> {
    let mut removed = Vec::new();
    let library = home.join(".local/share/screen-side-switcher");
    if library.join("screenside").is_dir() && fs::remove_dir_all(&library).is_ok() {
        removed.push(library);
    }
    for name in ["screen-side-gui", "screen-side"] {
        let launcher = home.join(".local/bin").join(name);
        if contains(&launcher, "from screenside.") && fs::remove_file(&launcher).is_ok() {
            removed.push(launcher);
        }
    }
    let entry = home.join(ENTRY);
    let is_v1_entry = fs::read_to_string(&entry)
        .map(|text| {
            text.lines().any(|l| {
                l.starts_with("Exec=") && l.trim_end().ends_with("/.local/bin/screen-side-gui")
            })
        })
        .unwrap_or(false);
    if is_v1_entry && fs::remove_file(&entry).is_ok() {
        removed.push(entry);
        let icon = home.join(ICON);
        if fs::remove_file(&icon).is_ok() {
            removed.push(icon);
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn v1_home() -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        let h = home.path();
        fs::create_dir_all(h.join(".local/bin")).unwrap();
        fs::create_dir_all(h.join(".local/share/screen-side-switcher/screenside")).unwrap();
        fs::create_dir_all(h.join(".local/share/applications")).unwrap();
        fs::create_dir_all(h.join(".local/share/icons/hicolor/scalable/apps")).unwrap();
        fs::write(
            h.join(".local/bin/screen-side"),
            "#!/usr/bin/env python3\nimport sys\nfrom screenside.cli import main\n",
        )
        .unwrap();
        fs::write(
            h.join(".local/bin/screen-side-gui"),
            "#!/usr/bin/env python3\nfrom screenside.app import main\n",
        )
        .unwrap();
        fs::write(
            h.join(".local/share/applications/io.github.danieltyukov.ScreenSide.desktop"),
            format!(
                "[Desktop Entry]\nExec={}/.local/bin/screen-side-gui\n",
                h.display()
            ),
        )
        .unwrap();
        fs::write(
            h.join(
                ".local/share/icons/hicolor/scalable/apps/io.github.danieltyukov.ScreenSide.svg",
            ),
            "<svg/>",
        )
        .unwrap();
        fs::write(h.join(".local/bin/unrelated"), "keep").unwrap();
        home
    }

    #[test]
    fn removes_only_what_is_recognisably_1_0() {
        let home = v1_home();
        let h = home.path();
        let removed = cleanup_v1(h);
        assert_eq!(removed.len(), 5, "{removed:?}");
        assert!(!h.join(".local/share/screen-side-switcher").exists());
        assert!(!h.join(".local/bin/screen-side").exists());
        assert!(!h.join(".local/bin/screen-side-gui").exists());
        assert!(!h
            .join(".local/share/applications/io.github.danieltyukov.ScreenSide.desktop")
            .exists());
        assert!(!h
            .join(".local/share/icons/hicolor/scalable/apps/io.github.danieltyukov.ScreenSide.svg")
            .exists());
        assert!(h.join(".local/bin/unrelated").exists());
        assert!(cleanup_v1(h).is_empty(), "a second run finds nothing");
    }

    #[test]
    fn leaves_a_2_0_install_alone() {
        let home = tempfile::tempdir().unwrap();
        let h = home.path();
        fs::create_dir_all(h.join(".local/bin")).unwrap();
        fs::write(h.join(".local/bin/screen-side"), b"\x7fELF binary").unwrap();
        fs::create_dir_all(h.join(".local/share/applications")).unwrap();
        fs::write(
            h.join(".local/share/applications/io.github.danieltyukov.ScreenSide.desktop"),
            "[Desktop Entry]\nExec=/usr/bin/screen-side-gui\n",
        )
        .unwrap();
        assert!(cleanup_v1(h).is_empty());
        assert!(h.join(".local/bin/screen-side").exists());
    }
}
