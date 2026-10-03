//! The tray (menu bar on macOS) menu: the four sides, swap, saved layouts,
//! open and quit. Rebuilt whenever the screens or layouts change, so the
//! checked side and the layouts on offer are always the ones in force.

use screen_side_core::layout::{baseline, infer, Side};
use screen_side_core::store::{matches, resolve};
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::commands::{change, toggle_now};
use crate::{show_main, Shared};

const ID: &str = "main";

pub fn set_enabled(app: &AppHandle, enabled: bool) -> tauri::Result<()> {
    if !enabled {
        app.remove_tray_by_id(ID);
        return Ok(());
    }
    if app.tray_by_id(ID).is_some() {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    let icon = Image::from_bytes(include_bytes!("../icons/tray-template.png"))?;
    #[cfg(not(target_os = "macos"))]
    let icon = Image::from_bytes(include_bytes!("../icons/tray.png"))?;
    TrayIconBuilder::with_id(ID)
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("Screen Side")
        .menu(&menu(app)?)
        .show_menu_on_left_click(true)
        .on_menu_event(on_menu)
        .build(app)?;
    Ok(())
}

/// Rebuilds the menu from the screens in force. Safe from any thread.
pub fn refresh(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(tray) = handle.tray_by_id(ID) {
            if let Ok(menu) = menu(&handle) {
                let _ = tray.set_menu(Some(menu));
            }
        }
    });
}

fn menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let shared = app.state::<Shared>();
    let state = shared.backend.as_ref().and_then(|b| b.query().ok());
    let can_move = state.as_ref().is_some_and(|s| s.enabled().count() >= 2);
    let side = state.as_ref().and_then(infer).and_then(|a| a.common_side());
    let item = |s: Side, label: &str| {
        CheckMenuItem::with_id(
            app,
            format!("side:{s}"),
            label,
            can_move,
            side == Some(s),
            None::<&str>,
        )
    };
    let layouts = shared.store.layouts().unwrap_or_default();
    let layout_items: Vec<MenuItem<Wry>> = if layouts.is_empty() {
        vec![MenuItem::with_id(
            app,
            "none",
            "No saved layouts",
            false,
            None::<&str>,
        )?]
    } else {
        layouts
            .iter()
            .map(|l| {
                let fits = state.as_ref().is_some_and(|s| matches(l, s));
                let label = if fits {
                    l.name.clone()
                } else {
                    format!("{} (other screens)", l.name)
                };
                MenuItem::with_id(app, format!("layout:{}", l.name), label, fits, None::<&str>)
            })
            .collect::<tauri::Result<_>>()?
    };
    let layout_refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = layout_items
        .iter()
        .map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>)
        .collect();
    let layouts_menu = Submenu::with_items(app, "Layouts", true, &layout_refs)?;

    Menu::with_items(
        app,
        &[
            &item(Side::Left, "Left")?,
            &item(Side::Right, "Right")?,
            &item(Side::Above, "Above")?,
            &item(Side::Below, "Below")?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "toggle", "Swap sides", can_move, None::<&str>)?,
            &layouts_menu,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "open", "Open Screen Side", true, None::<&str>)?,
            &MenuItem::with_id(app, "quit", "Quit Screen Side", true, None::<&str>)?,
        ],
    )
}

fn on_menu(app: &AppHandle, event: MenuEvent) {
    let id = event.id().as_ref().to_string();
    let result = match id.as_str() {
        "open" => {
            show_main(app);
            Ok(())
        }
        "quit" => {
            app.exit(0);
            Ok(())
        }
        "toggle" => toggle_now(app),
        _ => {
            if let Some(side) = id
                .strip_prefix("side:")
                .and_then(|s| s.parse::<Side>().ok())
            {
                change(app, |state| Ok(baseline(state)?.move_all(side)))
            } else if let Some(name) = id.strip_prefix("layout:") {
                let shared = app.state::<Shared>();
                match shared.store.find(name) {
                    Ok(saved) => change(app, |state| resolve(&saved, state)),
                    Err(e) => Err(e.to_string()),
                }
            } else {
                Ok(())
            }
        }
    };
    if let Err(e) = result {
        // The menu cannot show text, so the window does.
        let _ = app.emit("state-changed", ());
        show_main(app);
        eprintln!("screen-side: {e}");
    }
    refresh(app);
}
