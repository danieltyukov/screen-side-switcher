//! The Tauri shell around screen-side-core: one window, a tray menu, a
//! background watcher, autostart and a global shortcut. Every screen change
//! goes through core; this crate decides when and shows the result.

pub mod args;
mod commands;
mod headless;
mod hotkey;
mod tray;
pub mod view;
mod watcher;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use screen_side_core::backend::{detect, Backend, SystemProbe};
use screen_side_core::shortcut::{self, Desktop};
use screen_side_core::store::Store;
use tauri::{AppHandle, Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

use crate::args::Action;

pub const MAIN_WINDOW: &str = "main";

/// What every command, the tray and the watcher share.
pub struct Shared {
    pub backend: Option<Arc<dyn Backend>>,
    pub detect_error: Option<String>,
    pub store: Store,
    pub desktop: Desktop,
    /// A global shortcut that could not be registered, shown in the window.
    pub hotkey_error: Mutex<Option<String>>,
    /// The latest failure of something with no window of its own (the tray,
    /// the hotkey, the watcher), shown once the next time the window draws.
    pub background_error: Mutex<Option<String>>,
}

impl Shared {
    fn new() -> Shared {
        let (backend, detect_error) = match detect() {
            Ok((backend, _)) => (Some(Arc::from(backend)), None),
            Err(e) => (None, Some(e.to_string())),
        };
        let store =
            Store::open().unwrap_or_else(|_| Store::at(std::env::temp_dir().join("screen-side")));
        Shared {
            backend,
            detect_error,
            store,
            desktop: shortcut::desktop(&SystemProbe),
            hotkey_error: Mutex::new(None),
            background_error: Mutex::new(None),
        }
    }

    /// Keeps a failure for the window; the newest one wins.
    pub fn note(&self, message: impl Into<String>) {
        *self.background_error.lock().unwrap() = Some(message.into());
    }

    /// The failure to show, once.
    pub fn take_notice(&self) -> Option<String> {
        self.background_error.lock().unwrap().take()
    }

    /// Whether this app registers the global shortcut itself.
    pub fn app_managed_hotkey(&self) -> bool {
        matches!(
            self.desktop,
            Desktop::Windows | Desktop::Macos | Desktop::OtherX11
        )
    }

    /// The command a desktop shortcut runs: this binary (or the AppImage) with --toggle.
    pub fn toggle_command(&self) -> Vec<String> {
        let exe = std::env::var_os("APPIMAGE")
            .map(PathBuf::from)
            .or_else(|| std::env::current_exe().ok())
            .unwrap_or_else(|| PathBuf::from("screen-side-gui"));
        vec![exe.to_string_lossy().into_owned(), "--toggle".into()]
    }
}

/// Brings the window back rather than opening another one.
pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// WebKitGTK on NVIDIA draws an empty white window unless explicit sync is
/// off. The packaged .desktop file sets this too; this covers the AppImage,
/// `tauri dev` and running the binary directly. A value someone set is kept.
#[cfg(target_os = "linux")]
fn apply_webkit_workarounds() {
    if std::env::var_os("__NV_DISABLE_EXPLICIT_SYNC").is_none() {
        // Safe here: nothing else is running yet.
        std::env::set_var("__NV_DISABLE_EXPLICIT_SYNC", "1");
    }
}

#[cfg(not(target_os = "linux"))]
fn apply_webkit_workarounds() {}

pub fn run() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args::parse(&args) {
        Action::Toggle => std::process::exit(headless::toggle()),
        Action::Apply(name) => std::process::exit(headless::apply(&name)),
        Action::Show => start(false),
        Action::Background => start(true),
    }
}

fn start(background_launch: bool) {
    apply_webkit_workarounds();
    let shared = Shared::new();
    let settings = shared.store.settings().unwrap_or_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app)
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(shared)
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::move_screen,
            commands::toggle,
            commands::set_align,
            commands::set_primary,
            commands::save_layout,
            commands::apply_layout,
            commands::forget_layout,
            commands::set_layout_auto,
            commands::update_settings,
            commands::install_shortcut,
            commands::remove_shortcut,
            commands::diagnostics,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            if settings.background {
                if let Err(e) = tray::set_enabled(&handle, true) {
                    eprintln!("screen-side: no tray icon: {e}");
                }
            }
            watcher::start(handle.clone());
            let shared = handle.state::<Shared>();
            if shared.app_managed_hotkey() {
                if let Err(e) = hotkey::apply(&handle, settings.shortcut.as_deref()) {
                    *shared.hotkey_error.lock().unwrap() = Some(e);
                }
            }
            // Starting hidden only makes sense with a tray to come back through.
            if !(background_launch && settings.background) {
                show_main(&handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let background = window
                    .state::<Shared>()
                    .store
                    .settings()
                    .map(|s| s.background)
                    .unwrap_or(false);
                if background {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("Screen Side could not start");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_background_failure_is_shown_once() {
        let dir = std::env::temp_dir().join("screen-side-shared-test");
        let shared = Shared {
            backend: None,
            detect_error: None,
            store: Store::at(&dir),
            desktop: Desktop::OtherX11,
            hotkey_error: Mutex::new(None),
            background_error: Mutex::new(None),
        };
        assert_eq!(shared.take_notice(), None);
        shared.note("Could not apply 'office': the screens changed");
        shared.note("A later problem");
        assert_eq!(shared.take_notice().as_deref(), Some("A later problem"));
        assert_eq!(shared.take_notice(), None);
    }
}
