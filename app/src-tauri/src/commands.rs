//! One command per thing the window can do. Each is synchronous, which in
//! Tauri 2 means it runs on the main thread; on macOS that is where screen
//! names are available.

use std::fmt::Display;

use screen_side_core::backend::{arrange, unattended, ApplyMode, SystemProbe};
use screen_side_core::doctor;
use screen_side_core::layout::{baseline, Align, Arrangement, Side};
use screen_side_core::model::State as Screens;
use screen_side_core::run::{which, SystemRunner};
use screen_side_core::shortcut::{self, DEFAULT_GNOME_KEYS};
use screen_side_core::store::{capture, resolve};
use screen_side_core::Error;
use serde::{Deserialize, Deserializer};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;

use crate::view::{self, app_state, AppState, ViewInput};
use crate::{hotkey, tray, Shared};

fn text(e: impl Display) -> String {
    e.to_string()
}

fn cli_path() -> Option<String> {
    let beside = std::env::current_exe().ok().and_then(|exe| {
        let name = if cfg!(windows) {
            "screen-side.exe"
        } else {
            "screen-side"
        };
        let candidate = exe.with_file_name(name);
        candidate.is_file().then_some(candidate)
    });
    beside
        .or_else(|| which("screen-side"))
        .map(|p| p.to_string_lossy().into_owned())
}

/// The state the window draws, read fresh.
pub fn current(app: &AppHandle) -> AppState {
    let shared = app.state::<Shared>();
    let settings = shared.store.settings().unwrap_or_default();
    let installed = shortcut::is_installed(&SystemRunner, shared.desktop);
    let support = view::shortcut_support(shared.desktop, installed, &shared.toggle_command());
    let hotkey_error = shared.hotkey_error.lock().unwrap().clone();
    let base = |error: Option<String>| ViewInput {
        platform: view::platform(),
        backend: None,
        layouts: shared.store.layouts().map_err(text),
        settings: &settings,
        auto_start: app.autolaunch().is_enabled().unwrap_or(false),
        shortcut: support.clone(),
        cli_path: cli_path(),
        error: error.or_else(|| hotkey_error.clone()),
    };
    let Some(backend) = shared.backend.as_ref() else {
        return app_state(base(shared.detect_error.clone()));
    };
    match backend.query() {
        Ok(state) => app_state(ViewInput {
            backend: Some((backend.as_ref(), &state)),
            ..base(None)
        }),
        Err(e) => app_state(base(Some(e.to_string()))),
    }
}

/// Who asked for a change, which decides how it is applied.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum From {
    /// The window: someone is there to answer a "keep these settings?"
    /// prompt, so the change is saved.
    Window,
    /// The tray or a hotkey: no prompt may be left unanswered, so where
    /// saving asks for one (GNOME) the change is temporary.
    Quick,
}

/// Works out an arrangement from the screens in force and applies it.
/// Shared by the window, the tray and the hotkey.
pub fn change(
    app: &AppHandle,
    from: From,
    f: impl FnOnce(&Screens) -> Result<Arrangement, Error>,
) -> Result<(), String> {
    let shared = app.state::<Shared>();
    let backend = shared
        .backend
        .as_ref()
        .ok_or_else(|| shared.detect_error.clone().unwrap_or_default())?;
    let state = backend.query().map_err(text)?;
    let arr = f(&state).map_err(text)?;
    let mode = match from {
        From::Window => ApplyMode::Persistent,
        From::Quick => unattended(backend.capabilities()),
    };
    arrange(backend.as_ref(), &state, &arr, mode).map_err(text)?;
    tray::refresh(app);
    let _ = app.emit("state-changed", ());
    Ok(())
}

pub fn toggle_now(app: &AppHandle, from: From) -> Result<(), String> {
    change(app, from, |state| Ok(baseline(state)?.toggled()))
}

#[tauri::command]
pub fn get_state(app: AppHandle) -> AppState {
    current(&app)
}

#[tauri::command]
pub fn move_screen(app: AppHandle, screen: Option<String>, side: Side) -> Result<AppState, String> {
    change(&app, From::Window, |state| {
        let arr = baseline(state)?;
        Ok(match screen {
            Some(id) => arr.move_screen(&id, side)?,
            None => arr.move_all(side),
        })
    })?;
    Ok(current(&app))
}

#[tauri::command]
pub fn toggle(app: AppHandle) -> Result<AppState, String> {
    toggle_now(&app, From::Window)?;
    Ok(current(&app))
}

#[tauri::command]
pub fn set_align(app: AppHandle, align: Align) -> Result<AppState, String> {
    change(&app, From::Window, |state| {
        Ok(baseline(state)?.with_align(align))
    })?;
    Ok(current(&app))
}

#[tauri::command]
pub fn set_primary(
    app: AppHandle,
    shared: State<Shared>,
    screen: String,
) -> Result<AppState, String> {
    if !shared
        .backend
        .as_ref()
        .is_some_and(|b| b.capabilities().primary)
    {
        return Err("This desktop has no primary screen to set.".into());
    }
    change(&app, From::Window, |state| {
        Ok(baseline(state)?.with_primary(&screen))
    })?;
    Ok(current(&app))
}

#[tauri::command]
pub fn save_layout(
    app: AppHandle,
    shared: State<Shared>,
    name: String,
    auto: bool,
) -> Result<AppState, String> {
    let backend = shared
        .backend
        .as_ref()
        .ok_or("There are no screens to save.")?;
    let state = backend.query().map_err(text)?;
    let arr = baseline(&state).map_err(text)?;
    shared
        .store
        .put(capture(&name, &state, &arr, auto).map_err(text)?)
        .map_err(text)?;
    tray::refresh(&app);
    Ok(current(&app))
}

#[tauri::command]
pub fn apply_layout(
    app: AppHandle,
    shared: State<Shared>,
    name: String,
) -> Result<AppState, String> {
    let saved = shared.store.find(&name).map_err(text)?;
    change(&app, From::Window, |state| resolve(&saved, state))?;
    Ok(current(&app))
}

#[tauri::command]
pub fn forget_layout(
    app: AppHandle,
    shared: State<Shared>,
    name: String,
) -> Result<AppState, String> {
    shared.store.forget(&name).map_err(text)?;
    tray::refresh(&app);
    Ok(current(&app))
}

#[tauri::command]
pub fn set_layout_auto(
    app: AppHandle,
    shared: State<Shared>,
    name: String,
    auto: bool,
) -> Result<AppState, String> {
    shared.store.set_auto(&name, auto).map_err(text)?;
    Ok(current(&app))
}

/// `null` clears the shortcut; a missing field leaves it alone.
fn present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Option<String>>, D::Error> {
    Ok(Some(Option::deserialize(d)?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    background: Option<bool>,
    auto_start: Option<bool>,
    auto_apply: Option<bool>,
    #[serde(default, deserialize_with = "present")]
    shortcut: Option<Option<String>>,
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    shared: State<Shared>,
    patch: SettingsPatch,
) -> Result<AppState, String> {
    let mut settings = shared.store.settings().map_err(text)?;
    if let Some(start) = patch.auto_start {
        let manager = app.autolaunch();
        let done = if start {
            manager.enable()
        } else {
            manager.disable()
        };
        done.map_err(|e| format!("Could not change start at login: {e}"))?;
    }
    if let Some(background) = patch.background {
        settings.background = background;
        tray::set_enabled(&app, background)
            .map_err(|e| format!("Could not show the tray icon: {e}"))?;
    }
    if let Some(auto_apply) = patch.auto_apply {
        settings.auto_apply = auto_apply;
    }
    if let Some(shortcut) = patch.shortcut {
        if shared.app_managed_hotkey() {
            if let Err(e) = hotkey::apply(&app, shortcut.as_deref()) {
                // Put the previous one back so a failed change loses nothing.
                let _ = hotkey::apply(&app, settings.shortcut.as_deref());
                return Err(e);
            }
            *shared.hotkey_error.lock().unwrap() = None;
        }
        settings.shortcut = shortcut;
    }
    shared.store.put_settings(&settings).map_err(text)?;
    Ok(current(&app))
}

#[tauri::command]
pub fn install_shortcut(app: AppHandle, shared: State<Shared>) -> Result<AppState, String> {
    shortcut::install(
        &SystemRunner,
        shared.desktop,
        &shared.toggle_command(),
        DEFAULT_GNOME_KEYS,
    )
    .map_err(text)?;
    Ok(current(&app))
}

#[tauri::command]
pub fn remove_shortcut(app: AppHandle, shared: State<Shared>) -> Result<AppState, String> {
    shortcut::remove(&SystemRunner, shared.desktop).map_err(text)?;
    Ok(current(&app))
}

#[tauri::command]
pub fn diagnostics(shared: State<Shared>) -> String {
    doctor::report(&SystemProbe, env!("CARGO_PKG_VERSION"), Some(&shared.store)).to_string()
}
