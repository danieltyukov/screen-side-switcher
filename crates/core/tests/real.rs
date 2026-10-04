//! Talks to the real display system. Skipped unless SCREEN_SIDE_REAL=1.
//! CI sets it on the macOS and Windows runners. On a desktop it is safe to
//! run: the only apply it makes is a check, or, where the system cannot
//! check, a re-apply of the arrangement already in force.

use screen_side_core::backend::{apply_checked, detect, ApplyMode};
use screen_side_core::model::Layout;

#[test]
fn query_and_check_the_current_layout() {
    if std::env::var("SCREEN_SIDE_REAL").as_deref() != Ok("1") {
        eprintln!("skipped: set SCREEN_SIDE_REAL=1");
        return;
    }
    let (backend, choice) = detect().expect("a backend for this session");
    let state = backend.query().expect("query");
    eprintln!("{} ({}): {:#?}", backend.name(), choice.reason, state);
    assert!(state.enabled().count() >= 1, "at least one screen is on");
    let current = Layout::from_state(&state);
    let mode = if backend.capabilities().verify {
        ApplyMode::Verify
    } else {
        ApplyMode::Temporary
    };
    apply_checked(backend.as_ref(), &current, mode).expect("the current layout is accepted");
    let after = backend.query().expect("query again");
    assert!(Layout::from_state(&after).same_as(&current));
}
