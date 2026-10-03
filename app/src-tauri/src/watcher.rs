//! The background thread: notices screens coming and going, puts saved
//! layouts back, and tells the window and the tray to redraw.

use std::time::Duration;

use screen_side_core::store::Store;
use screen_side_core::watch::Watcher;
use tauri::{AppHandle, Emitter, Manager};

use crate::{tray, Shared};

const INTERVAL: Duration = Duration::from_secs(2);

pub fn start(app: AppHandle) {
    let shared = app.state::<Shared>();
    let Some(backend) = shared.backend.clone() else {
        return;
    };
    let store = Store::at(shared.store.dir());
    let _ = std::thread::Builder::new()
        .name("screen-watcher".into())
        .spawn(move || {
            let mut watcher = Watcher::new(Duration::from_secs(1));
            loop {
                let auto_apply = store.settings().map(|s| s.auto_apply).unwrap_or(true);
                let events = watcher.tick(backend.as_ref(), &store, auto_apply);
                if !events.is_empty() {
                    let _ = app.emit("state-changed", ());
                    tray::refresh(&app);
                }
                std::thread::sleep(INTERVAL);
            }
        });
}
