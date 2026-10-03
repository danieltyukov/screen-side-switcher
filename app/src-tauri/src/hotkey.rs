//! The global toggle shortcut on Windows, macOS and X11. Wayland has no
//! global key grabs for applications, so there the desktop runs
//! `screen-side-gui --toggle` instead (see the shortcut module in core).

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::commands::{toggle_now, From};

/// Replaces whatever shortcut is registered with `keys`, or none.
pub fn apply(app: &AppHandle, keys: Option<&str>) -> Result<(), String> {
    let shortcuts = app.global_shortcut();
    let _ = shortcuts.unregister_all();
    let Some(keys) = keys else {
        return Ok(());
    };
    shortcuts
        .on_shortcut(keys, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                if let Err(e) = toggle_now(app, From::Quick) {
                    eprintln!("screen-side: {e}");
                }
            }
        })
        .map_err(|e| {
            format!(
                "{keys} could not be registered ({e}). Another app may use it; pick other keys."
            )
        })
}
