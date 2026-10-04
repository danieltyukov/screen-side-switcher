//! The Tauri shell around screen-side-core: one window, a tray menu, a
//! background watcher, autostart and a global shortcut. Every screen change
//! goes through core; this crate decides when and shows the result.

pub mod args;
mod autostart;
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

use crate::args::Action;

pub const MAIN_WINDOW: &str = "main";

type Detect = Box<dyn Fn() -> Result<Box<dyn Backend>, screen_side_core::Error> + Send + Sync>;

/// The backend, found on first use and kept. A session started at login can
/// come up before its display server is reachable, so a failed detection is
/// tried again the next time anything asks, instead of lasting all session.
pub struct BackendSlot {
    current: Mutex<Option<Arc<dyn Backend>>>,
    error: Mutex<Option<String>>,
    detect: Detect,
}

impl BackendSlot {
    pub fn new(detect: Detect) -> BackendSlot {
        let slot = BackendSlot {
            current: Mutex::new(None),
            error: Mutex::new(None),
            detect,
        };
        let _ = slot.get();
        slot
    }

    pub fn get(&self) -> Result<Arc<dyn Backend>, String> {
        let mut current = self.current.lock().unwrap();
        if let Some(backend) = current.as_ref() {
            return Ok(backend.clone());
        }
        match (self.detect)() {
            Ok(backend) => {
                let backend: Arc<dyn Backend> = Arc::from(backend);
                *current = Some(backend.clone());
                *self.error.lock().unwrap() = None;
                Ok(backend)
            }
            Err(e) => {
                *self.error.lock().unwrap() = Some(e.to_string());
                Err(e.to_string())
            }
        }
    }

    /// Why there is no backend, if there is none yet.
    pub fn error(&self) -> Option<String> {
        self.error.lock().unwrap().clone()
    }
}

/// What every command, the tray and the watcher share.
pub struct Shared {
    pub backend: BackendSlot,
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
        let backend = BackendSlot::new(Box::new(|| detect().map(|(backend, _)| backend)));
        let store =
            Store::open().unwrap_or_else(|_| Store::at(std::env::temp_dir().join("screen-side")));
        Shared {
            backend,
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

/// A 1.0 install in ~/.local shadows this app on PATH and in the menu; it
/// goes the first time 2.0 runs.
#[cfg(target_os = "linux")]
fn remove_v1() {
    if let Some(home) = std::env::var_os("HOME") {
        for path in screen_side_core::legacy::cleanup_v1(std::path::Path::new(&home)) {
            eprintln!(
                "screen-side: removed Screen Side 1.0 file {}",
                path.display()
            );
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn remove_v1() {}

fn start(background_launch: bool) {
    apply_webkit_workarounds();
    remove_v1();
    let shared = Shared::new();
    let settings = shared.store.settings().unwrap_or_default();

    let builder =
        tauri::Builder::default().plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app)
        }));
    // Start at login: a LaunchAgent on macOS; Linux and Windows are in autostart.rs.
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_plugin_autostart::init(
        tauri_plugin_autostart::MacosLauncher::LaunchAgent,
        Some(vec!["--background"]),
    ));
    builder
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
            backend: BackendSlot::new(Box::new(|| {
                Err(screen_side_core::Error::NoBackend("none in tests".into()))
            })),
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

    #[test]
    fn a_backend_missing_at_login_is_found_later() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let calls = Arc::new(AtomicUsize::new(0));
        let seen = calls.clone();
        let slot = BackendSlot::new(Box::new(move || {
            // The session bus is not up for the first try, as at login.
            if seen.fetch_add(1, Ordering::SeqCst) == 0 {
                Err(screen_side_core::Error::NoBackend("no session yet".into()))
            } else {
                Ok(Box::new(screen_side_core::backend::fake::Fake::in_memory(
                    screen_side_core::backend::fake::FakeFile::sample(),
                )))
            }
        }));
        assert_eq!(slot.error().as_deref(), Some("no session yet"));
        assert_eq!(slot.get().unwrap().name(), "fake");
        assert!(slot.error().is_none());
        slot.get().unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2, "found once, then kept");
    }
}
