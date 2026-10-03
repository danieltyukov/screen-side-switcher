//! Start at login. macOS uses the autostart plugin's LaunchAgent, whose
//! arguments are an array. Linux and Windows are written here, because the
//! plugin writes the program path unquoted: a Windows install lives under
//! "...\Screen Side\" and an AppImage may sit in a folder with spaces, and
//! neither would start. On Linux the AppImage's own path is used, not the
//! temporary mount it runs from.

use std::path::Path;
#[cfg(target_os = "linux")]
use std::path::PathBuf;

#[cfg(target_os = "macos")]
use tauri_plugin_autostart::ManagerExt;

use tauri::AppHandle;

const ARG: &str = "--background";

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
/// One argument for a desktop entry's Exec key, quoted per the Desktop
/// Entry Specification and then escaped as a string value.
pub fn exec_arg(arg: &str) -> String {
    let mut quoted = String::from("\"");
    for c in arg.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                quoted.push('\\');
                quoted.push(c);
            }
            '%' => quoted.push_str("%%"),
            _ => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted.replace('\\', "\\\\")
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn desktop_entry(program: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Screen Side\nComment=Starts Screen Side in the background\nExec={} {ARG}\nIcon=screen-side-gui\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        exec_arg(program)
    )
}

/// The Windows Run value: the path in quotes, then the argument.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn run_value(program: &str) -> String {
    format!("\"{program}\" {ARG}")
}

#[allow(dead_code)]
fn entry_path(config: &Path) -> std::path::PathBuf {
    config.join("autostart").join("screen-side.desktop")
}

#[allow(dead_code)]
pub fn is_enabled_in(config: &Path) -> bool {
    entry_path(config).is_file()
}

#[allow(dead_code)]
pub fn enable_in(config: &Path, program: &str) -> std::io::Result<()> {
    let path = entry_path(config);
    std::fs::create_dir_all(path.parent().expect("has a parent"))?;
    std::fs::write(path, desktop_entry(program))
}

#[allow(dead_code)]
pub fn disable_in(config: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(entry_path(config)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// The program to start: the AppImage itself when running from one.
#[allow(dead_code)]
fn program() -> String {
    std::env::var_os("APPIMAGE")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_exe().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "screen-side-gui".into())
}

#[cfg(target_os = "linux")]
fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"))
}

#[cfg(windows)]
mod registry {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const NAME: &str = "Screen Side";

    pub fn is_enabled() -> bool {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(RUN)
            .and_then(|key| key.get_value::<String, _>(NAME))
            .is_ok()
    }

    pub fn set(enabled: bool, value: &str) -> std::io::Result<()> {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN)?;
        if enabled {
            key.set_value(NAME, &value.to_string())
        } else {
            match key.delete_value(NAME) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            }
        }
    }
}

pub fn is_enabled(app: &AppHandle) -> bool {
    let _ = app;
    #[cfg(target_os = "linux")]
    return is_enabled_in(&config_dir());
    #[cfg(windows)]
    return registry::is_enabled();
    #[cfg(target_os = "macos")]
    return app.autolaunch().is_enabled().unwrap_or(false);
    #[allow(unreachable_code)]
    false
}

pub fn set(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let _ = app;
    let failed = |e: String| format!("Could not change start at login: {e}");
    #[cfg(target_os = "linux")]
    return if enabled {
        enable_in(&config_dir(), &program())
    } else {
        disable_in(&config_dir())
    }
    .map_err(|e| failed(e.to_string()));
    #[cfg(windows)]
    return registry::set(enabled, &run_value(&program())).map_err(|e| failed(e.to_string()));
    #[cfg(target_os = "macos")]
    return if enabled {
        app.autolaunch().enable()
    } else {
        app.autolaunch().disable()
    }
    .map_err(|e| failed(e.to_string()));
    #[allow(unreachable_code)]
    Err(failed("not available on this system".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_arguments_are_quoted_per_the_desktop_entry_spec() {
        assert_eq!(
            exec_arg("/usr/bin/screen-side-gui"),
            r#""/usr/bin/screen-side-gui""#
        );
        assert_eq!(
            exec_arg("/home/a b/My Apps/Screen Side.AppImage"),
            r#""/home/a b/My Apps/Screen Side.AppImage""#
        );
        // Quoting escapes " ` $ \ with a backslash, then the string escapes
        // every backslash again; % becomes %%.
        assert_eq!(exec_arg(r#"a"b$c`d\e%f"#), r#""a\\"b\\$c\\`d\\\\e%%f""#);
    }

    #[test]
    fn the_windows_run_value_quotes_the_path() {
        assert_eq!(
            run_value(r"C:\Users\Ann\AppData\Local\Screen Side\screen-side-gui.exe"),
            r#""C:\Users\Ann\AppData\Local\Screen Side\screen-side-gui.exe" --background"#
        );
    }

    #[test]
    fn the_linux_entry_starts_in_the_background() {
        let entry = desktop_entry("/home/a b/Screen Side.AppImage");
        assert!(
            entry.contains(r#"Exec="/home/a b/Screen Side.AppImage" --background"#),
            "{entry}"
        );
        assert!(entry.starts_with("[Desktop Entry]\n"));
    }

    #[test]
    fn enabling_and_disabling_on_linux() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_enabled_in(dir.path()));
        enable_in(dir.path(), "/usr/bin/screen-side-gui").unwrap();
        assert!(is_enabled_in(dir.path()));
        let text =
            std::fs::read_to_string(dir.path().join("autostart/screen-side.desktop")).unwrap();
        assert!(text.contains(r#"Exec="/usr/bin/screen-side-gui" --background"#));
        disable_in(dir.path()).unwrap();
        assert!(!is_enabled_in(dir.path()));
        disable_in(dir.path()).unwrap();
    }
}
