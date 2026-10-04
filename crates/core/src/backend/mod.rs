//! One backend per desktop. Each reads the screens into a [`State`] in its
//! own coordinate space and applies a [`Layout`]; everything in between is
//! shared.

pub mod ccd;
pub mod detect;
pub mod fake;
#[cfg(all(unix, not(target_os = "macos")))]
pub mod gnome;
pub mod kde;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod mutter;
pub mod quartz;
#[cfg(windows)]
pub mod windows;
pub mod wlroots;
pub mod x11;

use serde::{Deserialize, Serialize};

pub use detect::{choose, create, detect, Choice, Kind, Probe, SystemProbe};

use crate::layout::Origin;
use crate::model::{Layout, State};
use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyMode {
    /// Saved by the system, so it comes back after a reconnect.
    Persistent,
    /// Until the next reconnect.
    Temporary,
    /// Checked by the system without changing anything.
    Verify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    /// Can set which screen is primary.
    pub primary: bool,
    /// A temporary change differs from a saved one.
    pub temporary: bool,
    /// The system can check a layout without applying it.
    pub verify: bool,
    /// The system restores each set of screens' layout by itself.
    pub remembers: bool,
    pub origin: Origin,
    /// A saved change asks the person on screen to keep it and reverts
    /// without an answer (GNOME's "Keep these display settings?").
    #[serde(default)]
    pub confirms: bool,
}

/// The mode for a change nobody is watching: the watcher, the tray, a
/// hotkey. Where saving asks for confirmation, the change is temporary
/// instead, because an unanswered prompt would undo it.
pub fn unattended(caps: Capabilities) -> ApplyMode {
    if caps.confirms {
        ApplyMode::Temporary
    } else {
        ApplyMode::Persistent
    }
}

pub trait Backend: Send + Sync {
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> Capabilities;
    fn query(&self) -> Result<State, Error>;
    /// Applies `layout`. Re-reads whatever private state it needs and fails
    /// with [`Error::Changed`] if the enabled screens are not the ones the
    /// layout names.
    fn apply(&self, layout: &Layout, mode: ApplyMode) -> Result<(), Error>;
    /// Extra lines for `doctor`: tool versions, bus names.
    fn diagnostics(&self) -> Vec<(String, String)> {
        Vec::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    /// The system took the change.
    Done,
    /// The system checked the change and left the screens as they were.
    Checked,
    /// Computed only; this system has no way to check without applying.
    Computed,
}

/// Every apply first checks that the screens that are on are exactly the
/// ones the layout places; a screen plugged in or out since the query
/// would otherwise get a position meant for another arrangement.
pub fn ensure_same_screens(state: &State, layout: &Layout) -> Result<(), Error> {
    let mut on: Vec<&str> = state.enabled().map(|s| s.id.as_str()).collect();
    let mut named: Vec<&str> = layout.positions.iter().map(|p| p.id.as_str()).collect();
    on.sort_unstable();
    named.sort_unstable();
    if on == named {
        Ok(())
    } else {
        Err(Error::Changed)
    }
}

/// Applies with the rules every caller needs.
pub fn apply_checked(
    backend: &dyn Backend,
    layout: &Layout,
    mode: ApplyMode,
) -> Result<Applied, Error> {
    let caps = backend.capabilities();
    match mode {
        ApplyMode::Temporary if !caps.temporary && caps.remembers => {
            Err(Error::Unsupported(format!(
                "The {} backend always saves the arrangement, so a temporary change is not available here.",
                backend.name()
            )))
        }
        ApplyMode::Verify if !caps.verify => Ok(Applied::Computed),
        ApplyMode::Verify => backend.apply(layout, mode).map(|()| Applied::Checked),
        _ => backend.apply(layout, mode).map(|()| Applied::Done),
    }
}

/// Computes the layout for `arr` with this backend's origin rule and applies it.
pub fn arrange(
    backend: &dyn Backend,
    state: &State,
    arr: &crate::layout::Arrangement,
    mode: ApplyMode,
) -> Result<(Layout, Applied), Error> {
    let layout = crate::layout::compute(state, arr, backend.capabilities().origin)?;
    let applied = apply_checked(backend, &layout, mode)?;
    Ok((layout, applied))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Recorder {
        caps: Capabilities,
        calls: AtomicUsize,
    }

    impl Backend for Recorder {
        fn name(&self) -> &'static str {
            "recorder"
        }
        fn capabilities(&self) -> Capabilities {
            self.caps
        }
        fn query(&self) -> Result<State, Error> {
            Ok(State {
                backend: "recorder".into(),
                screens: vec![],
            })
        }
        fn apply(&self, _: &Layout, _: ApplyMode) -> Result<(), Error> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    fn recorder(temporary: bool, verify: bool, remembers: bool) -> Recorder {
        Recorder {
            caps: Capabilities {
                primary: true,
                temporary,
                verify,
                remembers,
                origin: Origin::TopLeft,
                confirms: false,
            },
            calls: AtomicUsize::new(0),
        }
    }

    fn layout() -> Layout {
        Layout {
            positions: vec![],
            primary: String::new(),
        }
    }

    #[test]
    fn temporary_is_refused_where_the_system_saves_anyway() {
        let kde_like = recorder(false, false, true);
        assert!(matches!(
            apply_checked(&kde_like, &layout(), ApplyMode::Temporary),
            Err(Error::Unsupported(_))
        ));
        assert_eq!(kde_like.calls.load(Ordering::SeqCst), 0);
        let wlroots_like = recorder(false, false, false);
        assert!(matches!(
            apply_checked(&wlroots_like, &layout(), ApplyMode::Temporary),
            Ok(Applied::Done)
        ));
    }

    #[test]
    fn verify_without_a_system_check_only_computes() {
        let r = recorder(true, false, true);
        assert!(matches!(
            apply_checked(&r, &layout(), ApplyMode::Verify),
            Ok(Applied::Computed)
        ));
        assert_eq!(r.calls.load(Ordering::SeqCst), 0);
        let v = recorder(true, true, true);
        assert!(matches!(
            apply_checked(&v, &layout(), ApplyMode::Verify),
            Ok(Applied::Checked)
        ));
        assert_eq!(v.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn persistent_applies() {
        let r = recorder(true, true, true);
        assert!(matches!(
            apply_checked(&r, &layout(), ApplyMode::Persistent),
            Ok(Applied::Done)
        ));
        assert_eq!(r.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn arrange_computes_with_the_backends_origin_and_applies() {
        use crate::layout::{baseline, Side};
        let fake = fake::Fake::in_memory(fake::FakeFile::sample());
        let state = fake.query().unwrap();
        let arr = baseline(&state).unwrap().move_all(Side::Right);
        let (layout, applied) = arrange(&fake, &state, &arr, ApplyMode::Persistent).unwrap();
        assert_eq!(applied, Applied::Done);
        assert_eq!(layout.position("HDMI-1").unwrap().x, 1280);
        assert_eq!(fake.query().unwrap().screen("HDMI-1").unwrap().rect.x, 1280);
    }

    #[test]
    fn unattended_changes_avoid_confirmation_prompts() {
        let mut caps = recorder(true, true, true).caps;
        assert_eq!(unattended(caps), ApplyMode::Persistent);
        caps.confirms = true;
        assert_eq!(unattended(caps), ApplyMode::Temporary);
    }

    fn at(ids: &[&str]) -> Layout {
        Layout {
            positions: ids
                .iter()
                .map(|id| crate::model::Position {
                    id: id.to_string(),
                    x: 0,
                    y: 0,
                })
                .collect(),
            primary: String::new(),
        }
    }

    #[test]
    fn a_layout_must_name_exactly_the_screens_that_are_on() {
        let state = fake::Fake::in_memory(fake::FakeFile::sample())
            .query()
            .unwrap();
        assert!(ensure_same_screens(&state, &at(&["HDMI-1", "eDP-1"])).is_ok());
        assert!(matches!(
            ensure_same_screens(&state, &at(&["eDP-1"])),
            Err(Error::Changed)
        ));
        assert!(matches!(
            ensure_same_screens(&state, &at(&["eDP-1", "HDMI-1", "DP-9"])),
            Err(Error::Changed)
        ));
    }

    #[test]
    fn windows_screens_changed_between_query_and_apply() {
        use crate::model::Rect;
        let path = |target: u32, source: u32, rect: Option<Rect>| ccd::CcdPath {
            adapter_low: 1,
            adapter_high: 0,
            source_id: source,
            target_id: target,
            technology: 5,
            source: rect,
            gdi_name: format!(r"\\.\DISPLAY{target}"),
            friendly_name: String::new(),
            edid: None,
        };
        let before = ccd::to_state(&[
            path(1, 0, Some(Rect::new(0, 0, 10, 10))),
            path(2, 1, Some(Rect::new(10, 0, 10, 10))),
        ]);
        let layout = Layout::from_state(&before);
        let unplugged = ccd::to_state(&[path(1, 0, Some(Rect::new(0, 0, 10, 10)))]);
        assert!(ensure_same_screens(&before, &layout).is_ok());
        assert!(matches!(
            ensure_same_screens(&unplugged, &layout),
            Err(Error::Changed)
        ));
    }

    #[test]
    fn macos_screens_changed_between_query_and_apply() {
        let display = |id: u32, mirror_of: u32| quartz::QuartzDisplay {
            id,
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            builtin: false,
            main: id == 1,
            vendor: 1,
            model: 1,
            serial: 0,
            pixels_wide: 100,
            mirror_of,
            name: None,
        };
        let before = quartz::to_state(&[display(1, 0), display(2, 0)]);
        let layout = Layout::from_state(&before);
        let mirrored = quartz::to_state(&[display(1, 0), display(2, 1)]);
        assert!(matches!(
            ensure_same_screens(&mirrored, &layout),
            Err(Error::Changed)
        ));
    }
}
