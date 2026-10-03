//! Noticing new screens and putting a saved layout back.
//!
//! Only a change in the set of connected screens triggers a saved layout,
//! never a change of positions, so the watcher's own apply cannot set it
//! off again. Errors are reported and the loop carries on.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::backend::{arrange, ApplyMode, Backend};
use crate::layout::compute;
use crate::model::{Layout, State};
use crate::store::{fingerprint, matches, resolve, Store};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The set of enabled screens changed (or this is the first look).
    ScreensChanged {
        screens: Vec<String>,
    },
    /// Same screens, new positions or primary.
    Moved,
    /// A saved layout was put back.
    Applied {
        name: String,
    },
    Failed {
        message: String,
    },
}

pub struct Watcher {
    last: Option<State>,
    settle: Duration,
}

impl Watcher {
    /// `settle` is how long to wait after new screens appear before applying,
    /// so the system has finished bringing them up.
    pub fn new(settle: Duration) -> Watcher {
        Watcher { last: None, settle }
    }

    pub fn tick(&mut self, backend: &dyn Backend, store: &Store, auto_apply: bool) -> Vec<Event> {
        let state = match backend.query() {
            Ok(state) => state,
            Err(e) => {
                return vec![Event::Failed {
                    message: e.to_string(),
                }]
            }
        };
        let previous = self.last.replace(state.clone());
        let screens = fingerprint(&state);
        let set_changed = previous.as_ref().is_none_or(|p| fingerprint(p) != screens);
        let mut events = Vec::new();
        if set_changed {
            events.push(Event::ScreensChanged {
                screens: screens.clone(),
            });
        } else if previous.as_ref() != Some(&state) {
            events.push(Event::Moved);
        }
        if set_changed && auto_apply {
            if let Some(event) = self.apply_saved(backend, store, &screens) {
                events.push(event);
            }
        }
        events
    }

    fn apply_saved(
        &mut self,
        backend: &dyn Backend,
        store: &Store,
        screens: &[String],
    ) -> Option<Event> {
        let layouts = match store.layouts() {
            Ok(layouts) => layouts,
            Err(e) => {
                return Some(Event::Failed {
                    message: e.to_string(),
                })
            }
        };
        let current = self.last.clone()?;
        let saved = layouts
            .into_iter()
            .find(|l| l.auto && matches(l, &current))?;
        std::thread::sleep(self.settle);
        let state = match backend.query() {
            Ok(state) if fingerprint(&state) == screens => state,
            Ok(_) => return None,
            Err(e) => {
                return Some(Event::Failed {
                    message: e.to_string(),
                })
            }
        };
        let result = resolve(&saved, &state).and_then(|arr| {
            let target = compute(&state, &arr, backend.capabilities().origin)?;
            if target.same_as(&Layout::from_state(&state)) {
                return Ok(false);
            }
            arrange(backend, &state, &arr, ApplyMode::Persistent).map(|_| true)
        });
        match result {
            Ok(false) => None,
            Ok(true) => {
                self.last = backend.query().ok();
                Some(Event::Applied { name: saved.name })
            }
            Err(e) => Some(Event::Failed {
                message: format!("Could not apply '{}': {e}", saved.name),
            }),
        }
    }
}

/// Checks every `interval` until `stop` is set, reporting what happened.
pub fn run(
    backend: &dyn Backend,
    store: &Store,
    interval: Duration,
    auto_apply: impl Fn() -> bool,
    stop: &AtomicBool,
    mut on_event: impl FnMut(Event),
) {
    let mut watcher = Watcher::new(Duration::from_secs(1));
    while !stop.load(Ordering::Relaxed) {
        for event in watcher.tick(backend, store, auto_apply()) {
            on_event(event);
        }
        std::thread::sleep(interval);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::fake::{Fake, FakeFile};
    use crate::backend::{ApplyMode, Capabilities};
    use crate::layout::{baseline, Side};
    use crate::model::Layout;
    use crate::store::capture;
    use crate::Error;
    use std::sync::atomic::AtomicBool;

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::at(dir.path().join("config"));
        (dir, store)
    }

    /// Saves "office": the sample with the monitor moved right.
    fn save_office(store: &Store, auto: bool) {
        let sample = Fake::in_memory(FakeFile::sample()).query().unwrap();
        let arr = baseline(&sample).unwrap().move_all(Side::Right);
        store
            .put(capture("office", &sample, &arr, auto).unwrap())
            .unwrap();
    }

    fn laptop_only() -> FakeFile {
        let mut file = FakeFile::sample();
        file.screens[1].enabled = false;
        file
    }

    fn watcher() -> Watcher {
        Watcher::new(Duration::ZERO)
    }

    fn applied(name: &str) -> Event {
        Event::Applied { name: name.into() }
    }

    fn is_screens_changed(e: &Event) -> bool {
        matches!(e, Event::ScreensChanged { .. })
    }

    #[test]
    fn first_tick_applies_a_matching_auto_layout() {
        let (_dir, store) = store();
        save_office(&store, true);
        let fake = Fake::in_memory(FakeFile::sample());
        let events = watcher().tick(&fake, &store, true);
        assert_eq!(events.len(), 2, "{events:?}");
        assert!(is_screens_changed(&events[0]));
        assert_eq!(events[1], applied("office"));
        assert_eq!(fake.query().unwrap().screen("HDMI-1").unwrap().rect.x, 1280);
    }

    #[test]
    fn nothing_happens_when_nothing_changes() {
        let (_dir, store) = store();
        let fake = Fake::in_memory(FakeFile::sample());
        let mut w = watcher();
        w.tick(&fake, &store, true);
        assert_eq!(w.tick(&fake, &store, true), vec![]);
    }

    #[test]
    fn a_move_is_reported_without_applying() {
        let (_dir, store) = store();
        save_office(&store, false);
        let fake = Fake::in_memory(FakeFile::sample());
        let mut w = watcher();
        w.tick(&fake, &store, true);
        let mut moved = FakeFile::sample();
        moved.screens[0].rect.y = 0;
        fake.replace(moved);
        assert_eq!(w.tick(&fake, &store, true), vec![Event::Moved]);
        assert_eq!(fake.query().unwrap().screen("eDP-1").unwrap().rect.y, 0);
    }

    #[test]
    fn a_hotplug_applies_the_layout_for_the_new_screens() {
        let (_dir, store) = store();
        save_office(&store, true);
        let fake = Fake::in_memory(laptop_only());
        let mut w = watcher();
        let first = w.tick(&fake, &store, true);
        assert_eq!(first.len(), 1);
        assert!(is_screens_changed(&first[0]));
        fake.replace(FakeFile::sample());
        let events = w.tick(&fake, &store, true);
        assert!(is_screens_changed(&events[0]));
        assert_eq!(events[1], applied("office"));
    }

    #[test]
    fn auto_off_or_not_auto_does_not_apply() {
        let (_dir, store) = store();
        save_office(&store, true);
        let fake = Fake::in_memory(FakeFile::sample());
        let events = watcher().tick(&fake, &store, false);
        assert_eq!(events.len(), 1);
        assert_eq!(fake.query().unwrap().screen("HDMI-1").unwrap().rect.x, 0);

        let (_dir2, manual) = self::store();
        save_office(&manual, false);
        let events = watcher().tick(&fake, &manual, true);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn an_applied_layout_does_not_apply_again() {
        let (_dir, store) = store();
        save_office(&store, true);
        let fake = Fake::in_memory(FakeFile::sample());
        let mut w = watcher();
        assert_eq!(w.tick(&fake, &store, true).len(), 2);
        assert_eq!(w.tick(&fake, &store, true), vec![]);
    }

    #[test]
    fn already_in_place_is_not_reapplied() {
        let (_dir, store) = store();
        let fake = Fake::in_memory(FakeFile::sample());
        let sample = fake.query().unwrap();
        let arr = baseline(&sample).unwrap();
        store
            .put(capture("office", &sample, &arr, true).unwrap())
            .unwrap();
        let events = watcher().tick(&fake, &store, true);
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(fake.load().unwrap().last_apply, None);
    }

    struct Flaky {
        fail: AtomicBool,
        inner: Fake,
    }

    impl Backend for Flaky {
        fn name(&self) -> &'static str {
            "flaky"
        }
        fn capabilities(&self) -> Capabilities {
            self.inner.capabilities()
        }
        fn query(&self) -> Result<crate::model::State, Error> {
            if self.fail.swap(false, Ordering::SeqCst) {
                return Err(Error::System("the bus went away".into()));
            }
            self.inner.query()
        }
        fn apply(&self, layout: &Layout, mode: ApplyMode) -> Result<(), Error> {
            self.inner.apply(layout, mode)
        }
    }

    #[test]
    fn errors_become_events() {
        let (_dir, store) = store();
        let flaky = Flaky {
            fail: AtomicBool::new(true),
            inner: Fake::in_memory(FakeFile::sample()),
        };
        let mut w = watcher();
        assert_eq!(
            w.tick(&flaky, &store, true),
            vec![Event::Failed {
                message: "the bus went away".into()
            }]
        );
        assert!(is_screens_changed(&w.tick(&flaky, &store, true)[0]));

        std::fs::create_dir_all(store.dir()).unwrap();
        std::fs::write(store.dir().join("layouts.json"), "{ not json").unwrap();
        let events = watcher().tick(&flaky, &store, true);
        assert!(
            matches!(&events[1], Event::Failed { message } if message.contains("layouts.json")),
            "{events:?}"
        );
    }
}
