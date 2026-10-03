//! One backend per desktop. Each reads the screens into a [`State`] in its
//! own coordinate space and applies a [`Layout`]; everything in between is
//! shared.

pub mod ccd;
pub mod detect;
pub mod fake;
#[cfg(all(unix, not(target_os = "macos")))]
pub mod gnome;
pub mod kde;
pub mod mutter;
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
}
