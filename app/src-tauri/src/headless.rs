//! `--toggle` and `--apply NAME`: do the change and exit, without a window.
//! Desktop shortcuts run these, so they must be quick and quiet. A running
//! window notices the change on its next watcher tick.

use screen_side_core::backend::{arrange, detect, unattended};
use screen_side_core::layout::{baseline, Arrangement};
use screen_side_core::model::State;
use screen_side_core::store::{resolve, Store};
use screen_side_core::Error;

fn change(f: impl FnOnce(&State) -> Result<Arrangement, Error>) -> i32 {
    let result = detect().and_then(|(backend, _)| {
        let state = backend.query()?;
        let arr = f(&state)?;
        // A shortcut has nobody at a prompt, so where saving asks to confirm
        // (GNOME) the change is temporary rather than reverted.
        let mode = unattended(backend.capabilities());
        arrange(backend.as_ref(), &state, &arr, mode).map(|_| ())
    });
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("screen-side: {e}");
            e.exit_code()
        }
    }
}

pub fn toggle() -> i32 {
    change(|state| Ok(baseline(state)?.toggled()))
}

pub fn apply(name: &str) -> i32 {
    change(|state| {
        let saved = Store::open()?.find(name)?;
        resolve(&saved, state)
    })
}
