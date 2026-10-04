# Screen Side 2.0 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite Screen Side as a cross-platform Tauri app and CLI (Windows, macOS, GNOME, KDE, wlroots, X11) with several-screen support, saved layouts per desk, primary and end alignment, shortcut setup, `doctor` and JSON output, a project site, and the community files and CI of the maintainer's other repositories.

**Architecture:** A Rust workspace. `crates/core` holds the model, the pure layout maths, one backend per desktop behind a `Backend` trait, saved layouts, the watcher, shortcut setup and diagnostics, with no Tauri dependency. `crates/cli` is the `screen-side` command. `app/` is a React interface behind a TypeScript `Backend` seam with a mock, and `app/src-tauri` is the Tauri 2 shell (commands, tray, watcher thread, autostart, global shortcut). `site/` is a Vite page that shares the app's design tokens.

**Tech Stack:** Rust 1.85+ (edition 2021), clap 4, serde, thiserror 2, directories 6, humantime 2, zbus 5 (Linux), windows 0.61 (Windows), core-graphics 0.25 + objc2-app-kit 0.3 (macOS), Tauri 2 with single-instance, autostart, global-shortcut, opener and clipboard-manager plugins, React 19, TypeScript 5, Vite 8, Vitest, Testing Library, Playwright, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-10-04-screen-side-2-design.md`. Read it before starting any task; this plan argues from it.

## Global Constraints

- Version `2.0.0` everywhere: workspace `Cargo.toml`, `app/package.json`, `app/src-tauri/tauri.conf.json`, `site/package.json`.
- Identifier `io.github.danieltyukov.ScreenSide`; product name `Screen Side`; CLI binary `screen-side`; app binary `screen-side-gui`.
- Rust `rust-version = "1.85"`, edition 2021, MIT licence, `publish = false`.
- `cargo fmt --all --check` and `cargo clippy -p screen-side-core -p screen-side --all-targets -- -D warnings` stay clean after every task.
- Config directory: `SCREEN_SIDE_CONFIG_DIR`, else `ProjectDirs::from("io.github", "danieltyukov", "ScreenSide")`.
- Backend override: `SCREEN_SIDE_BACKEND` in `fake|gnome|kde|wlroots|x11|windows|macos`; fake state file `SCREEN_SIDE_FAKE_STATE`.
- CLI exit codes: 0 done; 1 the system or the layout maths refused; 2 usage error, unknown screen or layout, or a feature the backend lacks.
- JSON output carries `"schema": 1`.
- Writing style in every user-facing string, doc and commit: no emojis, no em dashes or en dashes as punctuation, plain direct prose.
- Commit messages: conventional (`feat:`, `fix:`, `test:`, `docs:`, `ci:`, `chore:`), describing the change only. No AI attribution, no session links, no `Co-Authored-By`. Never `--no-verify`.
- Commits authored by the global identity `danieltyukov <60662998+danieltyukov@users.noreply.github.com>`.
- No outward-facing action (push, repository settings, tags, releases) without the maintainer's explicit go.
- Before writing UI (Tasks 17, 18 and 21), invoke the `frontend-design:frontend-design` skill.

## Review Focus

1. A connected screen that is switched off (closed lid) must be listed as off, never moved, never counted in a layout's fingerprint, and never offered as `--screen`. Tests: Task 2 (compute ignores it), Task 3 (infer ignores it), Task 7 (status lists it as `off`; `--screen` on it is a usage error).
2. Fractional scale and rotation: sizes must equal what the compositor uses or the pointer meets a 1-pixel seam. Mutter and KWin round, wlroots truncates, rotation swaps before scaling. Tests: Task 9 (Mutter 1.5 and 1.25, transform 1), Task 10 (KDE rotation 2 and scale 1.5), Task 11 (wlroots 1.5 truncation and transform 90).
3. Screens changing between query and apply (a hotplug mid-command) must fail with "the screens changed" and apply nothing. Tests: Task 4 (fake), Task 9 (Mutter connectors differ), Task 13 (CCD ids differ).
4. Paths and names with spaces or quotes: an app under `Program Files`, an AppImage in `~/My Apps`, a layout named `Mum's desk`. Shortcut commands must be quoted for their target, and layout names must never reach a shell. Tests: Task 15 (quoting), Task 5 (name with quote round trip), Task 8 (CLI save and apply with such a name).
5. A corrupt `layouts.json`, or one written by a newer version, must be refused with a message saying which file and what to do, must never be overwritten, and must not break commands that do not need layouts. Tests: Task 5 (store), Task 8 (CLI `left` still works, `save` refuses, file unchanged).

---

## Phase 1: core and command line

### Task 1: Workspace, error type and model

**Files:**
- Delete: `screenside/`, `bin/`, `install.sh`, `uninstall.sh`, `data/io.github.danieltyukov.ScreenSide.desktop`
- Move: `data/io.github.danieltyukov.ScreenSide.svg` to `app/icon-source.svg`
- Create: `Cargo.toml`, `.gitignore` (replace), `crates/core/Cargo.toml`, `crates/core/src/lib.rs`, `crates/core/src/error.rs`, `crates/core/src/model.rs`

**Interfaces:**
- Produces: `screen_side_core::{Error, model::{Rect, Identity, Screen, State, Position, Layout}}` exactly as below.

- [ ] **Step 1: Remove the Python implementation and scaffold the workspace**

```bash
git rm -r -q screenside bin install.sh uninstall.sh data/io.github.danieltyukov.ScreenSide.desktop
mkdir -p app crates/core/src crates/cli/src
git mv data/io.github.danieltyukov.ScreenSide.svg app/icon-source.svg
```

`Cargo.toml`:

```toml
[workspace]
members = ["crates/core", "crates/cli"]
default-members = ["crates/core", "crates/cli"]
resolver = "2"

[workspace.package]
version = "2.0.0"
edition = "2021"
rust-version = "1.85"
license = "MIT"
repository = "https://github.com/danieltyukov/screen-side-switcher"

[profile.release]
codegen-units = 1
lto = true
opt-level = "s"
strip = true
```

`.gitignore`:

```
/target/
node_modules/
dist/
/app/src-tauri/gen/schemas/
/app/playwright-report/
/app/test-results/
/site/playwright-report/
/site/test-results/
*.tmp
.DS_Store
```

`crates/core/Cargo.toml`:

```toml
[package]
name = "screen-side-core"
description = "Monitor arrangement for Screen Side: layout maths, saved layouts and one backend per desktop"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
publish = false

[dependencies]
directories = "6"
humantime = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"

[dev-dependencies]
tempfile = "3"
```

`crates/core/src/lib.rs`:

```rust
//! Everything Screen Side knows about screens, independent of any window.
//!
//! The command line and the app are both thin layers over this crate: they
//! pick a backend, read a [`model::State`], turn a request into a
//! [`layout::Arrangement`], compute a [`model::Layout`] and hand it back.

pub mod error;
pub mod model;

pub use error::Error;
```

`crates/core/src/error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// No backend fits this session; the message says what is missing.
    #[error("{0}")]
    NoBackend(String),
    /// The backend cannot do what was asked (no primary on wlroots, say).
    #[error("{0}")]
    Unsupported(String),
    /// The screens were different when the change was applied.
    #[error("The screens changed while the arrangement was being applied. Try again.")]
    Changed,
    /// An external tool failed.
    #[error("{program} failed: {message}")]
    Tool { program: String, message: String },
    /// The system API refused or was unreachable.
    #[error("{0}")]
    System(String),
    /// A config file could not be read or written.
    #[error("{0}")]
    Config(String),
    /// The request names something that does not exist.
    #[error("{0}")]
    Usage(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Error {
    /// The command line's exit code for this error.
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Usage(_) | Error::Unsupported(_) => 2,
            _ => 1,
        }
    }
}
```

The `Layout(#[from] crate::layout::LayoutError)` variant is added in Task 2, once the layout module exists, so this task compiles on its own.

- [ ] **Step 2: Write the failing model tests**

Append to `crates/core/src/model.rs` (the file starts with the tests module so the test fails to compile first):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn screen(id: &str, x: i32, y: i32, w: i32, h: i32) -> Screen {
        Screen {
            id: id.into(),
            connector: id.into(),
            name: id.into(),
            identity: Identity::default(),
            builtin: false,
            enabled: true,
            primary: false,
            rect: Rect::new(x, y, w, h),
            scale: 1.0,
        }
    }

    #[test]
    fn overlap_needs_positive_area() {
        let a = Rect::new(0, 0, 100, 100);
        assert!(a.overlaps(&Rect::new(50, 50, 100, 100)));
        assert!(!a.overlaps(&Rect::new(100, 0, 100, 100)), "touching is not overlapping");
        assert!(!a.overlaps(&Rect::new(100, 100, 10, 10)), "a corner is not overlapping");
    }

    #[test]
    fn shared_edge_is_zero_at_a_corner() {
        let a = Rect::new(0, 0, 100, 100);
        assert_eq!(a.shared_edge(&Rect::new(100, 20, 50, 50)), 50);
        assert_eq!(a.shared_edge(&Rect::new(-50, 90, 50, 50)), 10);
        assert_eq!(a.shared_edge(&Rect::new(20, 100, 200, 10)), 80);
        assert_eq!(a.shared_edge(&Rect::new(100, 100, 10, 10)), 0);
        assert_eq!(a.shared_edge(&Rect::new(150, 0, 10, 10)), 0);
    }

    #[test]
    fn identity_key_falls_back_to_the_connector() {
        let id = Identity { vendor: " DEL ".into(), product: "0x41B5".into(), serial: "ABC".into() };
        assert_eq!(id.key("HDMI-1"), "del:0x41b5:abc");
        assert_eq!(Identity::default().key("HDMI-1"), "connector:hdmi-1");
    }

    #[test]
    fn listed_puts_enabled_screens_first_in_reading_order() {
        let mut off = screen("eDP-1", 0, 0, 0, 0);
        off.enabled = false;
        let state = State {
            backend: "fake".into(),
            screens: vec![off, screen("B", 1920, 0, 100, 100), screen("A", 0, 0, 1920, 1080)],
        };
        let order: Vec<&str> = state.listed().iter().map(|s| s.id.as_str()).collect();
        assert_eq!(order, ["A", "B", "eDP-1"]);
    }

    #[test]
    fn layout_from_state_keeps_current_positions() {
        let mut a = screen("A", 0, 0, 10, 10);
        a.primary = true;
        let state = State { backend: "fake".into(), screens: vec![a, screen("B", 10, 0, 10, 10)] };
        let layout = Layout::from_state(&state);
        assert_eq!(layout.primary, "A");
        assert_eq!(layout.position("B"), Some(&Position { id: "B".into(), x: 10, y: 0 }));
    }
}
```

- [ ] **Step 3: Run the tests to see them fail**

Run: `cargo test -p screen-side-core model`
Expected: compile errors, `Rect`, `Screen`, `State` not found.

- [ ] **Step 4: Implement the model above the tests module**

```rust
//! The picture every backend reports and every operation works on.
//!
//! Positions and sizes are in the backend's own coordinate space (logical
//! pixels, physical pixels or points). Backends convert before filling a
//! [`Rect`], so two screens that touch on the desktop touch here too.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self { x, y, width, height }
    }

    pub fn right(&self) -> i32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.height
    }

    /// True when the two share a positive area. Touching is not overlapping.
    pub fn overlaps(&self, other: &Rect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }

    /// Length of the edge the two share. Zero when they meet only at a
    /// corner or not at all, which is where the pointer cannot cross.
    pub fn shared_edge(&self, other: &Rect) -> i32 {
        if self.right() == other.x || other.right() == self.x {
            (self.bottom().min(other.bottom()) - self.y.max(other.y)).max(0)
        } else if self.bottom() == other.y || other.bottom() == self.y {
            (self.right().min(other.right()) - self.x.max(other.x)).max(0)
        } else {
            0
        }
    }
}

/// What a monitor says it is, for recognising it in a later session.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Identity {
    pub vendor: String,
    pub product: String,
    pub serial: String,
}

impl Identity {
    /// `vendor:product:serial`, lower-cased. A backend that reports neither
    /// vendor nor product gets `connector:<name>` instead.
    pub fn key(&self, connector: &str) -> String {
        let norm = |s: &str| s.trim().to_lowercase();
        let (vendor, product, serial) = (norm(&self.vendor), norm(&self.product), norm(&self.serial));
        if vendor.is_empty() && product.is_empty() {
            format!("connector:{}", norm(connector))
        } else {
            format!("{vendor}:{product}:{serial}")
        }
    }
}

fn one() -> f64 {
    1.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Screen {
    /// Stable within a session: connector, CCD target, CGDirectDisplayID.
    pub id: String,
    pub connector: String,
    pub name: String,
    #[serde(default)]
    pub identity: Identity,
    pub builtin: bool,
    /// Part of the desktop right now. A closed lid is connected but off.
    pub enabled: bool,
    pub primary: bool,
    pub rect: Rect,
    #[serde(default = "one")]
    pub scale: f64,
}

impl Screen {
    pub fn key(&self) -> String {
        self.identity.key(&self.connector)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub backend: String,
    pub screens: Vec<Screen>,
}

impl State {
    pub fn screen(&self, id: &str) -> Option<&Screen> {
        self.screens.iter().find(|s| s.id == id)
    }

    pub fn enabled(&self) -> impl Iterator<Item = &Screen> {
        self.screens.iter().filter(|s| s.enabled)
    }

    pub fn primary(&self) -> Option<&Screen> {
        self.enabled().find(|s| s.primary)
    }

    /// The order `status` numbers screens in: enabled ones left to right
    /// and top to bottom, then the switched-off ones by connector.
    pub fn listed(&self) -> Vec<&Screen> {
        let mut on: Vec<&Screen> = self.enabled().collect();
        on.sort_by_key(|s| (s.rect.x, s.rect.y, s.connector.clone()));
        let mut off: Vec<&Screen> = self.screens.iter().filter(|s| !s.enabled).collect();
        off.sort_by_key(|s| s.connector.clone());
        on.extend(off);
        on
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub id: String,
    pub x: i32,
    pub y: i32,
}

/// What a backend is asked to apply: a position for every enabled screen
/// and which one is primary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layout {
    pub positions: Vec<Position>,
    pub primary: String,
}

impl Layout {
    pub fn position(&self, id: &str) -> Option<&Position> {
        self.positions.iter().find(|p| p.id == id)
    }

    /// The layout already in force. Used to check an apply path without
    /// moving anything.
    pub fn from_state(state: &State) -> Layout {
        let primary = state
            .primary()
            .or_else(|| state.enabled().next())
            .map(|s| s.id.clone())
            .unwrap_or_default();
        Layout {
            positions: state
                .enabled()
                .map(|s| Position { id: s.id.clone(), x: s.rect.x, y: s.rect.y })
                .collect(),
            primary,
        }
    }
}
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test -p screen-side-core model`
Expected: 5 passed.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "chore: replace the Python package with a Rust workspace and core model"
```

### Task 2: Layout maths, compute

**Files:**
- Create: `crates/core/src/layout.rs`
- Modify: `crates/core/src/lib.rs` (add `pub mod layout;`), `crates/core/src/error.rs` (add the `Layout` variant shown in Task 1)

**Interfaces:**
- Consumes: `model::{Rect, Screen, State, Position, Layout}`.
- Produces: `layout::{Side, Align, Origin, Placement, Arrangement, LayoutError, anchor, compute}`:
  - `Side::{Left, Right, Above, Below}`, `Side::ALL`, `Side::horizontal(self) -> bool`, `Side::mirrored(self) -> Side`, `Side::as_str(self) -> &'static str`, `FromStr`, `Display`, serde lowercase.
  - `Align::{Start, Center, End}` (default `Center`), `FromStr` accepting `start|top|left`, `center|centre|middle`, `end|bottom|right`, `Align::label(self, horizontal: bool) -> &'static str`, serde lowercase.
  - `Origin::{TopLeft, Primary}`, serde snake_case.
  - `Arrangement { anchor: String, placements: Vec<Placement>, align: Align, aligned: bool, primary: String }`.
  - `pub fn anchor(state: &State) -> Option<&Screen>`.
  - `pub fn compute(state: &State, arr: &Arrangement, origin: Origin) -> Result<Layout, LayoutError>`.

- [ ] **Step 1: Write the failing tests at the bottom of `layout.rs`**

```rust
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::{Identity, Rect, Screen, State};

    pub(crate) fn screen(id: &str, w: i32, h: i32) -> Screen {
        Screen {
            id: id.into(),
            connector: id.into(),
            name: id.into(),
            identity: Identity::default(),
            builtin: false,
            enabled: true,
            primary: false,
            rect: Rect::new(0, 0, w, h),
            scale: 1.0,
        }
    }

    pub(crate) fn laptop(w: i32, h: i32) -> Screen {
        Screen { builtin: true, primary: true, ..screen("eDP-1", w, h) }
    }

    pub(crate) fn state(screens: Vec<Screen>) -> State {
        State { backend: "fake".into(), screens }
    }

    fn arr(placements: &[(&str, Side)], align: Align) -> Arrangement {
        Arrangement {
            anchor: "eDP-1".into(),
            placements: placements
                .iter()
                .map(|(id, side)| Placement { screen: id.to_string(), side: *side })
                .collect(),
            align,
            aligned: true,
            primary: "eDP-1".into(),
        }
    }

    fn at(layout: &Layout, id: &str) -> (i32, i32) {
        let p = layout.position(id).expect("screen in layout");
        (p.x, p.y)
    }

    #[test]
    fn one_screen_left_centred_matches_v1() {
        // 1.0 put a 1440-high monitor left of an 800-high laptop at (0,0)
        // and the laptop at (2560,320).
        let s = state(vec![laptop(1280, 800), screen("HDMI-1", 2560, 1440)]);
        let l = compute(&s, &arr(&[("HDMI-1", Side::Left)], Align::Center), Origin::TopLeft).unwrap();
        assert_eq!(at(&l, "HDMI-1"), (0, 0));
        assert_eq!(at(&l, "eDP-1"), (2560, 320));
        assert_eq!(l.primary, "eDP-1");
    }

    #[test]
    fn each_side_and_alignment() {
        let s = state(vec![laptop(1280, 800), screen("HDMI-1", 1920, 1080)]);
        let cases = [
            (Side::Right, Align::Start, (0, 0), (1280, 0)),
            (Side::Right, Align::End, (0, 280), (1280, 0)),
            (Side::Above, Align::Center, (320, 1080), (0, 0)),
            (Side::Below, Align::Start, (0, 0), (0, 800)),
            (Side::Below, Align::End, (640, 0), (0, 800)),
        ];
        for (side, align, laptop_at, external_at) in cases {
            let l = compute(&s, &arr(&[("HDMI-1", side)], align), Origin::TopLeft).unwrap();
            assert_eq!(at(&l, "eDP-1"), laptop_at, "{side} {align}");
            assert_eq!(at(&l, "HDMI-1"), external_at, "{side} {align}");
        }
    }

    #[test]
    fn odd_differences_round_down() {
        // floor((801 - 1440) / 2) = -320, so the laptop sits 320 down.
        let s = state(vec![laptop(1280, 801), screen("HDMI-1", 2560, 1440)]);
        let l = compute(&s, &arr(&[("HDMI-1", Side::Right)], Align::Center), Origin::TopLeft).unwrap();
        assert_eq!(at(&l, "eDP-1"), (0, 320));
    }

    #[test]
    fn chains_go_outward_in_order() {
        let s = state(vec![laptop(1280, 800), screen("A", 1920, 1080), screen("B", 2560, 1440)]);
        let l = compute(&s, &arr(&[("A", Side::Left), ("B", Side::Left)], Align::Start), Origin::TopLeft).unwrap();
        assert_eq!(at(&l, "B"), (0, 0));
        assert_eq!(at(&l, "A"), (2560, 0));
        assert_eq!(at(&l, "eDP-1"), (4480, 0));
    }

    #[test]
    fn mixed_sides() {
        let s = state(vec![laptop(1280, 800), screen("A", 1920, 1080), screen("B", 1280, 1024)]);
        let l = compute(&s, &arr(&[("A", Side::Left), ("B", Side::Above)], Align::Start), Origin::TopLeft).unwrap();
        assert_eq!(at(&l, "A"), (0, 1024));
        assert_eq!(at(&l, "B"), (1920, 0));
        assert_eq!(at(&l, "eDP-1"), (1920, 1024));
    }

    #[test]
    fn perpendicular_overlap_is_refused() {
        let s = state(vec![laptop(1280, 800), screen("A", 2560, 1440), screen("B", 2560, 1440)]);
        let err = compute(&s, &arr(&[("A", Side::Left), ("B", Side::Above)], Align::Center), Origin::TopLeft)
            .unwrap_err();
        assert_eq!(err, LayoutError::Overlap("A".into(), "B".into()));
        assert!(err.to_string().contains("would overlap"));
        // Start alignment keeps them apart.
        assert!(compute(&s, &arr(&[("A", Side::Left), ("B", Side::Above)], Align::Start), Origin::TopLeft).is_ok());
    }

    #[test]
    fn primary_origin_puts_the_primary_at_zero() {
        let s = state(vec![laptop(1280, 800), screen("HDMI-1", 2560, 1440)]);
        let mut a = arr(&[("HDMI-1", Side::Left)], Align::Center);
        let l = compute(&s, &a, Origin::Primary).unwrap();
        assert_eq!(at(&l, "eDP-1"), (0, 0));
        assert_eq!(at(&l, "HDMI-1"), (-2560, -320));
        a.primary = "HDMI-1".into();
        let l = compute(&s, &a, Origin::Primary).unwrap();
        assert_eq!(at(&l, "HDMI-1"), (0, 0));
        assert_eq!(at(&l, "eDP-1"), (2560, 320));
    }

    #[test]
    fn switched_off_screens_are_left_alone() {
        let mut lid = laptop(1280, 800);
        lid.enabled = false;
        lid.primary = false;
        let mut a = screen("A", 1920, 1080);
        a.primary = true;
        let s = state(vec![lid, a, screen("B", 1920, 1080)]);
        let mut arrangement = arr(&[("B", Side::Right)], Align::Center);
        arrangement.anchor = "A".into();
        arrangement.primary = "A".into();
        let l = compute(&s, &arrangement, Origin::TopLeft).unwrap();
        assert_eq!(l.positions.len(), 2);
        assert!(l.position("eDP-1").is_none());
        // Naming the switched-off screen is refused.
        let mut bad = arrangement.clone();
        bad.placements.push(Placement { screen: "eDP-1".into(), side: Side::Left });
        assert_eq!(compute(&s, &bad, Origin::TopLeft).unwrap_err(), LayoutError::UnknownScreen("eDP-1".into()));
    }

    #[test]
    fn every_enabled_screen_needs_a_place() {
        let s = state(vec![laptop(1280, 800), screen("A", 1920, 1080), screen("B", 1920, 1080)]);
        let err = compute(&s, &arr(&[("A", Side::Left)], Align::Center), Origin::TopLeft).unwrap_err();
        assert_eq!(err, LayoutError::MissingScreen("B".into()));
    }

    #[test]
    fn a_screen_without_a_resolution_is_refused() {
        let s = state(vec![laptop(1280, 800), screen("A", 0, 0)]);
        let err = compute(&s, &arr(&[("A", Side::Left)], Align::Center), Origin::TopLeft).unwrap_err();
        assert_eq!(err, LayoutError::NoResolution("A".into()));
    }

    #[test]
    fn connectivity_finds_an_island() {
        let rects = [Rect::new(0, 0, 10, 10), Rect::new(10, 0, 10, 10), Rect::new(30, 0, 10, 10)];
        assert_eq!(first_unreachable(&rects), Some(2));
        assert_eq!(first_unreachable(&rects[..2]), None);
        let corner = [Rect::new(0, 0, 10, 10), Rect::new(10, 10, 10, 10)];
        assert_eq!(first_unreachable(&corner), Some(1));
    }

    #[test]
    fn parsing_and_labels() {
        assert_eq!("LEFT".parse::<Side>().unwrap(), Side::Left);
        assert!("sideways".parse::<Side>().is_err());
        assert_eq!("centre".parse::<Align>().unwrap(), Align::Center);
        assert_eq!("bottom".parse::<Align>().unwrap(), Align::End);
        assert_eq!(Align::Start.label(true), "top");
        assert_eq!(Align::End.label(false), "right");
        assert_eq!(Side::Above.mirrored(), Side::Below);
        assert_eq!(serde_json::to_string(&Side::Below).unwrap(), "\"below\"");
        assert_eq!(serde_json::to_string(&Origin::TopLeft).unwrap(), "\"top_left\"");
    }
}
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p screen-side-core layout`
Expected: compile errors for the missing items.

- [ ] **Step 3: Implement `compute` and its types above the tests**

```rust
//! Turns "put this screen on the left" into coordinates the system accepts.
//!
//! Systems refuse arrangements whose screens overlap, and the pointer cannot
//! cross where two screens meet only at a corner. So screens are placed with
//! exact adjacency along one axis and a deliberate alignment on the other,
//! and anything that would overlap is refused rather than nudged into a gap.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::model::{Layout, Position, Rect, Screen, State};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
    Above,
    Below,
}

impl Side {
    pub const ALL: [Side; 4] = [Side::Left, Side::Right, Side::Above, Side::Below];

    pub fn horizontal(self) -> bool {
        matches!(self, Side::Left | Side::Right)
    }

    pub fn mirrored(self) -> Side {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
            Side::Above => Side::Below,
            Side::Below => Side::Above,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Right => "right",
            Side::Above => "above",
            Side::Below => "below",
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Side {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "left" => Ok(Side::Left),
            "right" => Ok(Side::Right),
            "above" | "up" | "top" => Ok(Side::Above),
            "below" | "down" | "bottom" => Ok(Side::Below),
            other => Err(format!("unknown side '{other}', expected left, right, above or below")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Start,
    #[default]
    Center,
    End,
}

impl Align {
    /// How the alignment reads for screens side by side (`horizontal`) or
    /// stacked.
    pub fn label(self, horizontal: bool) -> &'static str {
        match (self, horizontal) {
            (Align::Start, true) => "top",
            (Align::Start, false) => "left",
            (Align::Center, _) => "centred",
            (Align::End, true) => "bottom",
            (Align::End, false) => "right",
        }
    }
}

impl fmt::Display for Align {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Align::Start => "start",
            Align::Center => "center",
            Align::End => "end",
        })
    }
}

impl FromStr for Align {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "start" | "top" | "left" => Ok(Align::Start),
            "center" | "centre" | "middle" => Ok(Align::Center),
            "end" | "bottom" | "right" => Ok(Align::End),
            other => Err(format!("unknown alignment '{other}', expected start, center or end")),
        }
    }
}

/// Where the finished layout is anchored. Windows and macOS define the
/// primary display as the one at (0, 0); everything else wants the layout
/// to start at (0, 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    TopLeft,
    Primary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    pub screen: String,
    pub side: Side,
}

/// The intent behind a layout: which side each screen is on, nearest first
/// within a side, how they line up, and which is primary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Arrangement {
    pub anchor: String,
    pub placements: Vec<Placement>,
    pub align: Align,
    /// False when the screens in force match no alignment ("custom").
    pub aligned: bool,
    pub primary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    #[error("No screen is switched on.")]
    NoScreens,
    #[error("Only one screen is switched on, so there is nothing to arrange.")]
    OnlyOneScreen,
    #[error("{0} is not connected or is switched off.")]
    UnknownScreen(String),
    #[error("{0} has no place in the arrangement.")]
    MissingScreen(String),
    #[error("{0} reports no usable resolution.")]
    NoResolution(String),
    #[error("{0} and {1} would overlap. Put one of them on another side, or use another alignment.")]
    Overlap(String, String),
    #[error("{0} would not touch the other screens, so the pointer could not reach it.")]
    Disconnected(String),
    #[error("Pick an external screen to move. The built-in screen stays where it is.")]
    AnchorSelected,
}

/// The screen everything is arranged around: the built-in one if it is on,
/// else the primary, else the first enabled screen.
pub fn anchor(state: &State) -> Option<&Screen> {
    state
        .enabled()
        .find(|s| s.builtin)
        .or_else(|| state.primary())
        .or_else(|| state.enabled().next())
}

fn offset(align: Align, anchor_len: i32, len: i32) -> i32 {
    match align {
        Align::Start => 0,
        Align::End => anchor_len - len,
        Align::Center => (anchor_len - len).div_euclid(2),
    }
}

/// Index of the first rectangle that cannot be reached from the first one by
/// crossing shared edges, if any.
pub(crate) fn first_unreachable(rects: &[Rect]) -> Option<usize> {
    if rects.is_empty() {
        return None;
    }
    let mut reached = vec![false; rects.len()];
    reached[0] = true;
    let mut stack = vec![0];
    while let Some(i) = stack.pop() {
        for j in 0..rects.len() {
            if !reached[j] && rects[i].shared_edge(&rects[j]) > 0 {
                reached[j] = true;
                stack.push(j);
            }
        }
    }
    reached.iter().position(|r| !r)
}

pub fn compute(state: &State, arr: &Arrangement, origin: Origin) -> Result<Layout, LayoutError> {
    if state.enabled().next().is_none() {
        return Err(LayoutError::NoScreens);
    }
    let find = |id: &str| {
        state
            .screen(id)
            .filter(|s| s.enabled)
            .ok_or_else(|| LayoutError::UnknownScreen(state.screen(id).map_or(id, |s| &s.name).to_string()))
    };
    let anchor = find(&arr.anchor)?;
    find(&arr.primary)?;

    let mut placements: Vec<(&Screen, Side)> = Vec::new();
    for p in &arr.placements {
        let s = find(&p.screen)?;
        if s.id != anchor.id && !placements.iter().any(|(q, _)| q.id == s.id) {
            placements.push((s, p.side));
        }
    }
    for s in state.enabled() {
        if s.id != anchor.id && !placements.iter().any(|(q, _)| q.id == s.id) {
            return Err(LayoutError::MissingScreen(s.name.clone()));
        }
    }
    for s in std::iter::once(anchor).chain(placements.iter().map(|(s, _)| *s)) {
        if s.rect.width <= 0 || s.rect.height <= 0 {
            return Err(LayoutError::NoResolution(s.name.clone()));
        }
    }

    let (aw, ah) = (anchor.rect.width, anchor.rect.height);
    let mut placed: Vec<(&Screen, Rect)> = vec![(anchor, Rect::new(0, 0, aw, ah))];
    for side in Side::ALL {
        let mut cursor = 0;
        for (s, _) in placements.iter().filter(|(_, sd)| *sd == side) {
            let (w, h) = (s.rect.width, s.rect.height);
            let rect = match side {
                Side::Left => {
                    cursor += w;
                    Rect::new(-cursor, offset(arr.align, ah, h), w, h)
                }
                Side::Right => {
                    let r = Rect::new(aw + cursor, offset(arr.align, ah, h), w, h);
                    cursor += w;
                    r
                }
                Side::Above => {
                    cursor += h;
                    Rect::new(offset(arr.align, aw, w), -cursor, w, h)
                }
                Side::Below => {
                    let r = Rect::new(offset(arr.align, aw, w), ah + cursor, w, h);
                    cursor += h;
                    r
                }
            };
            placed.push((s, rect));
        }
    }

    for (i, (a, ra)) in placed.iter().enumerate() {
        for (b, rb) in &placed[i + 1..] {
            if ra.overlaps(rb) {
                return Err(LayoutError::Overlap(a.name.clone(), b.name.clone()));
            }
        }
    }
    let rects: Vec<Rect> = placed.iter().map(|(_, r)| *r).collect();
    if let Some(i) = first_unreachable(&rects) {
        return Err(LayoutError::Disconnected(placed[i].0.name.clone()));
    }

    let (dx, dy) = match origin {
        Origin::TopLeft => (
            -rects.iter().map(|r| r.x).min().unwrap_or(0),
            -rects.iter().map(|r| r.y).min().unwrap_or(0),
        ),
        Origin::Primary => {
            let r = placed
                .iter()
                .find(|(s, _)| s.id == arr.primary)
                .map(|(_, r)| *r)
                .unwrap_or_default();
            (-r.x, -r.y)
        }
    };
    Ok(Layout {
        positions: placed
            .iter()
            .map(|(s, r)| Position { id: s.id.clone(), x: r.x + dx, y: r.y + dy })
            .collect(),
        primary: arr.primary.clone(),
    })
}
```

Add `pub mod layout;` to `lib.rs` and this variant as the first one in `Error`:

```rust
    /// The layout maths refused the request.
    #[error(transparent)]
    Layout(#[from] crate::layout::LayoutError),
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p screen-side-core layout`
Expected: 12 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/core
git commit -m "feat(core): compute layouts for several screens with start, centre and end alignment"
```

### Task 3: Layout maths, infer and operations

**Files:**
- Modify: `crates/core/src/layout.rs`

**Interfaces:**
- Produces: `pub fn infer(state: &State) -> Option<Arrangement>`, `pub fn baseline(state: &State) -> Result<Arrangement, LayoutError>`, and on `Arrangement`: `side_of(&self, id: &str) -> Option<Side>`, `move_all(&self, side: Side) -> Arrangement`, `move_screen(&self, id: &str, side: Side) -> Result<Arrangement, LayoutError>`, `toggled(&self) -> Arrangement`, `with_align(&self, align: Align) -> Arrangement`, `with_primary(&self, id: &str) -> Arrangement`, `common_side(&self) -> Option<Side>`.

- [ ] **Step 1: Write the failing tests (append inside `tests`)**

```rust
    fn placed(mut s: Screen, x: i32, y: i32) -> Screen {
        s.rect.x = x;
        s.rect.y = y;
        s
    }

    #[test]
    fn infer_reads_back_what_compute_wrote() {
        let base = state(vec![laptop(1280, 800), screen("A", 1920, 1080), screen("B", 2560, 1440)]);
        for align in [Align::Start, Align::Center, Align::End] {
            let wanted = arr(&[("A", Side::Left), ("B", Side::Left)], align);
            let layout = compute(&base, &wanted, Origin::TopLeft).unwrap();
            let mut moved = base.clone();
            for s in &mut moved.screens {
                let p = layout.position(&s.id).unwrap();
                s.rect.x = p.x;
                s.rect.y = p.y;
            }
            assert_eq!(infer(&moved).unwrap(), wanted, "{align}");
        }
    }

    #[test]
    fn infer_takes_the_larger_gap_for_a_diagonal_screen() {
        let s = state(vec![placed(laptop(1280, 800), 0, 0), placed(screen("A", 100, 100), 2000, 900)]);
        assert_eq!(infer(&s).unwrap().side_of("A"), Some(Side::Right));
    }

    #[test]
    fn infer_gives_up_when_a_screen_overlaps_the_anchor() {
        let s = state(vec![placed(laptop(1280, 800), 0, 0), placed(screen("A", 1920, 1080), 600, 0)]);
        assert_eq!(infer(&s), None);
    }

    #[test]
    fn infer_reports_custom_alignment() {
        let s = state(vec![placed(laptop(1280, 800), 0, 100), placed(screen("A", 1920, 1080), 1280, 0)]);
        let a = infer(&s).unwrap();
        assert!(!a.aligned);
        assert_eq!(a.align, Align::Center);
    }

    #[test]
    fn infer_tolerates_one_pixel_of_centring() {
        // 1.0 centred against the taller screen, which can differ by one.
        let s = state(vec![placed(laptop(1280, 801), 2560, 319), placed(screen("A", 2560, 1440), 0, 0)]);
        let a = infer(&s).unwrap();
        assert!(a.aligned);
        assert_eq!(a.align, Align::Center);
    }

    #[test]
    fn infer_ignores_switched_off_screens() {
        let mut lid = laptop(1280, 800);
        lid.enabled = false;
        let mut a = placed(screen("A", 1920, 1080), 0, 0);
        a.primary = true;
        let s = state(vec![lid, a, placed(screen("B", 1920, 1080), 1920, 0)]);
        let inferred = infer(&s).unwrap();
        assert_eq!(inferred.anchor, "A");
        assert_eq!(inferred.placements, vec![Placement { screen: "B".into(), side: Side::Right }]);
    }

    #[test]
    fn baseline_needs_two_screens_and_falls_back_to_the_right() {
        assert_eq!(baseline(&state(vec![laptop(1280, 800)])).unwrap_err(), LayoutError::OnlyOneScreen);
        let overlapping = state(vec![placed(laptop(1280, 800), 0, 0), placed(screen("A", 1920, 1080), 600, 0)]);
        let b = baseline(&overlapping).unwrap();
        assert_eq!(b.placements, vec![Placement { screen: "A".into(), side: Side::Right }]);
    }

    #[test]
    fn move_all_keeps_the_nearest_screen_nearest() {
        let a = arr(&[("A", Side::Right), ("B", Side::Right), ("C", Side::Above)], Align::Center);
        let moved = a.move_all(Side::Left);
        let order: Vec<(&str, Side)> = moved.placements.iter().map(|p| (p.screen.as_str(), p.side)).collect();
        assert_eq!(order, [("A", Side::Left), ("C", Side::Left), ("B", Side::Left)]);
    }

    #[test]
    fn move_screen_moves_one_and_puts_it_last() {
        let a = arr(&[("A", Side::Left), ("B", Side::Right), ("C", Side::Right)], Align::Center);
        let moved = a.move_screen("A", Side::Right).unwrap();
        assert_eq!(moved.side_of("A"), Some(Side::Right));
        assert_eq!(moved.placements.last().unwrap().screen, "A");
        assert_eq!(a.move_screen("B", Side::Right).unwrap(), a, "same side is a no-op");
        assert_eq!(a.move_screen("Z", Side::Left).unwrap_err(), LayoutError::UnknownScreen("Z".into()));
    }

    #[test]
    fn moving_the_anchor_moves_the_only_other_screen_the_other_way() {
        let one = arr(&[("A", Side::Right)], Align::Center);
        assert_eq!(one.move_screen("eDP-1", Side::Right).unwrap().side_of("A"), Some(Side::Left));
        let two = arr(&[("A", Side::Right), ("B", Side::Left)], Align::Center);
        assert_eq!(two.move_screen("eDP-1", Side::Left).unwrap_err(), LayoutError::AnchorSelected);
    }

    #[test]
    fn toggle_mirrors_every_side() {
        let a = arr(&[("A", Side::Left), ("B", Side::Above)], Align::Center);
        let t = a.toggled();
        assert_eq!(t.side_of("A"), Some(Side::Right));
        assert_eq!(t.side_of("B"), Some(Side::Below));
        assert_eq!(t.toggled(), a);
    }

    #[test]
    fn common_side_only_when_all_agree() {
        assert_eq!(arr(&[("A", Side::Left), ("B", Side::Left)], Align::Center).common_side(), Some(Side::Left));
        assert_eq!(arr(&[("A", Side::Left), ("B", Side::Above)], Align::Center).common_side(), None);
    }
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p screen-side-core layout`
Expected: compile errors for `infer`, `baseline` and the methods.

- [ ] **Step 3: Implement**

```rust
/// The arrangement in force, or None when a screen overlaps the anchor (a
/// layout made by hand that no side describes).
pub fn infer(state: &State) -> Option<Arrangement> {
    let anchor = anchor(state)?;
    let a = anchor.rect;
    let mut found: Vec<(Side, i32, &Screen)> = Vec::new();
    for s in state.enabled().filter(|s| s.id != anchor.id) {
        let r = s.rect;
        let gaps = [
            (Side::Left, a.x - r.right()),
            (Side::Right, r.x - a.right()),
            (Side::Above, a.y - r.bottom()),
            (Side::Below, r.y - a.bottom()),
        ];
        let (side, gap) = gaps.into_iter().filter(|(_, g)| *g >= 0).max_by_key(|(_, g)| *g)?;
        found.push((side, gap, s));
    }
    found.sort_by_key(|(side, gap, s)| {
        (Side::ALL.iter().position(|x| x == side), *gap, s.connector.clone())
    });

    let matches = |align: Align| {
        found.iter().all(|(side, _, s)| {
            let (anchor_len, len, actual) = if side.horizontal() {
                (a.height, s.rect.height, s.rect.y - a.y)
            } else {
                (a.width, s.rect.width, s.rect.x - a.x)
            };
            (actual - offset(align, anchor_len, len)).abs() <= 1
        })
    };
    let align = [Align::Center, Align::Start, Align::End].into_iter().find(|al| matches(*al));

    Some(Arrangement {
        anchor: anchor.id.clone(),
        placements: found
            .iter()
            .map(|(side, _, s)| Placement { screen: s.id.clone(), side: *side })
            .collect(),
        align: align.unwrap_or_default(),
        aligned: align.is_some(),
        primary: state.primary().map_or_else(|| anchor.id.clone(), |s| s.id.clone()),
    })
}

/// The arrangement operations start from: the one in force, or every other
/// screen to the right of the anchor in left-to-right order.
pub fn baseline(state: &State) -> Result<Arrangement, LayoutError> {
    match state.enabled().count() {
        0 => return Err(LayoutError::NoScreens),
        1 => return Err(LayoutError::OnlyOneScreen),
        _ => {}
    }
    if let Some(arr) = infer(state) {
        return Ok(arr);
    }
    let anchor = anchor(state).ok_or(LayoutError::NoScreens)?;
    let mut others: Vec<&Screen> = state.enabled().filter(|s| s.id != anchor.id).collect();
    others.sort_by_key(|s| (s.rect.x, s.rect.y));
    Ok(Arrangement {
        anchor: anchor.id.clone(),
        placements: others
            .iter()
            .map(|s| Placement { screen: s.id.clone(), side: Side::Right })
            .collect(),
        align: Align::Center,
        aligned: true,
        primary: state.primary().map_or_else(|| anchor.id.clone(), |s| s.id.clone()),
    })
}

impl Arrangement {
    pub fn side_of(&self, id: &str) -> Option<Side> {
        self.placements.iter().find(|p| p.screen == id).map(|p| p.side)
    }

    /// The side every other screen is on, if they share one.
    pub fn common_side(&self) -> Option<Side> {
        let first = self.placements.first()?.side;
        self.placements.iter().all(|p| p.side == first).then_some(first)
    }

    /// Every screen to `side`. Screens nearest the anchor stay nearest, so
    /// moving a whole row is a mirror image rather than a reshuffle.
    pub fn move_all(&self, side: Side) -> Arrangement {
        let mut ranked: Vec<(usize, usize, &Placement)> = self
            .placements
            .iter()
            .enumerate()
            .map(|(i, p)| (self.placements[..i].iter().filter(|q| q.side == p.side).count(), i, p))
            .collect();
        ranked.sort_by_key(|(rank, i, _)| (*rank, *i));
        Arrangement {
            placements: ranked
                .into_iter()
                .map(|(_, _, p)| Placement { screen: p.screen.clone(), side })
                .collect(),
            aligned: true,
            ..self.clone()
        }
    }

    /// One screen to `side`, as the farthest on that side. Choosing the
    /// anchor with exactly one other screen means "put the anchor there",
    /// which moves the other screen to the opposite side.
    pub fn move_screen(&self, id: &str, side: Side) -> Result<Arrangement, LayoutError> {
        if id == self.anchor {
            return match self.placements.as_slice() {
                [only] => self.move_screen(&only.screen.clone(), side.mirrored()),
                _ => Err(LayoutError::AnchorSelected),
            };
        }
        let current = self.side_of(id).ok_or_else(|| LayoutError::UnknownScreen(id.to_string()))?;
        if current == side {
            return Ok(self.clone());
        }
        let mut placements: Vec<Placement> = self.placements.iter().filter(|p| p.screen != id).cloned().collect();
        placements.push(Placement { screen: id.to_string(), side });
        Ok(Arrangement { placements, aligned: true, ..self.clone() })
    }

    /// Left with right and above with below, for every screen.
    pub fn toggled(&self) -> Arrangement {
        Arrangement {
            placements: self
                .placements
                .iter()
                .map(|p| Placement { screen: p.screen.clone(), side: p.side.mirrored() })
                .collect(),
            aligned: true,
            ..self.clone()
        }
    }

    pub fn with_align(&self, align: Align) -> Arrangement {
        Arrangement { align, aligned: true, ..self.clone() }
    }

    pub fn with_primary(&self, id: &str) -> Arrangement {
        Arrangement { primary: id.to_string(), ..self.clone() }
    }
}
```

The toggle test expects `t.toggled() == a`; `a` was built with `aligned: true`, so the round trip holds.

- [ ] **Step 4: Run all core tests**

Run: `cargo test -p screen-side-core`
Expected: all pass (5 model + 24 layout).

- [ ] **Step 5: Commit**

```bash
git add crates/core
git commit -m "feat(core): read the arrangement in force and move, toggle, align and set primary"
```

### Task 4: Backend trait, runner, detection and the fake backend

**Files:**
- Create: `crates/core/src/backend/mod.rs`, `crates/core/src/backend/fake.rs`, `crates/core/src/backend/detect.rs`, `crates/core/src/run.rs`
- Modify: `crates/core/src/lib.rs` (`pub mod backend; pub mod run;`)

**Interfaces:**
- Consumes: model, layout.
- Produces:
  - `backend::{Backend, ApplyMode, Capabilities, Applied, apply_checked, Kind, Choice, Probe, SystemProbe, choose, create, detect}`.
  - `trait Backend: Send + Sync { fn name(&self) -> &'static str; fn capabilities(&self) -> Capabilities; fn query(&self) -> Result<State, Error>; fn apply(&self, layout: &Layout, mode: ApplyMode) -> Result<(), Error>; fn diagnostics(&self) -> Vec<(String, String)> { Vec::new() } }`
  - `ApplyMode::{Persistent, Temporary, Verify}` (serde snake_case).
  - `Capabilities { primary, temporary, verify, remembers: bool, origin: Origin }`.
  - `enum Applied { Done, Checked, Computed }`.
  - `pub fn apply_checked(backend: &dyn Backend, layout: &Layout, mode: ApplyMode) -> Result<Applied, Error>`.
  - `Kind::{Fake, Gnome, Kde, Wlroots, X11, Windows, Macos}` with `Kind::as_str`, `FromStr`.
  - `Choice { kind: Kind, reason: String, bus_name: Option<String> }`.
  - `trait Probe { fn os(&self) -> &str; fn var(&self, name: &str) -> Option<String>; fn on_path(&self, program: &str) -> bool; fn bus_has_owner(&self, name: &str) -> bool; }`
  - `pub fn choose(probe: &dyn Probe) -> Result<Choice, Error>`, `pub fn create(choice: &Choice) -> Result<Box<dyn Backend>, Error>`, `pub fn detect() -> Result<(Box<dyn Backend>, Choice), Error>`.
  - `backend::fake::{Fake, FakeFile}`: `Fake::from_env() -> Result<Fake, Error>`, `Fake::at(path: PathBuf) -> Fake`, `Fake::in_memory(file: FakeFile) -> Fake`, `Fake::replace(&self, file: FakeFile)` (tests), `FakeFile::sample() -> FakeFile`, `FakeFile { capabilities, screens, last_apply: Option<ApplyMode> }`.
  - `run::{Runner, SystemRunner, which, on_path}`: `trait Runner: Send + Sync { fn run(&self, program: &str, args: &[String]) -> Result<String, Error>; }`; `pub fn which(program: &str) -> Option<PathBuf>`; `pub fn on_path(program: &str) -> bool` (is `which(..).is_some()`). Test helper `run::testing::Scripted` behind `#[cfg(test)]`: `Scripted::new(responses: Vec<(&str, Result<&str, &str>)>)` returning outputs in order and recording `calls() -> Vec<Vec<String>>` (program first).

- [ ] **Step 1: Write the failing tests**

In `backend/mod.rs` tests: `apply_checked` refuses `Temporary` when `!temporary && remembers` (`Error::Unsupported`), returns `Computed` for `Verify` when `!verify` without calling `apply`, returns `Checked` for `Verify` when `verify`, returns `Done` otherwise. Use a test `Recorder` backend that counts `apply` calls:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Recorder { caps: Capabilities, calls: AtomicUsize }
    impl Backend for Recorder {
        fn name(&self) -> &'static str { "recorder" }
        fn capabilities(&self) -> Capabilities { self.caps }
        fn query(&self) -> Result<State, Error> { Ok(State { backend: "recorder".into(), screens: vec![] }) }
        fn apply(&self, _: &Layout, _: ApplyMode) -> Result<(), Error> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
    fn caps(temporary: bool, verify: bool, remembers: bool) -> Capabilities {
        Capabilities { primary: true, temporary, verify, remembers, origin: Origin::TopLeft }
    }
    fn layout() -> Layout { Layout { positions: vec![], primary: String::new() } }

    #[test]
    fn temporary_is_refused_where_the_system_saves_anyway() {
        let r = Recorder { caps: caps(false, false, true), calls: AtomicUsize::new(0) };
        assert!(matches!(apply_checked(&r, &layout(), ApplyMode::Temporary), Err(Error::Unsupported(_))));
        let wl = Recorder { caps: caps(false, false, false), calls: AtomicUsize::new(0) };
        assert!(matches!(apply_checked(&wl, &layout(), ApplyMode::Temporary), Ok(Applied::Done)));
    }

    #[test]
    fn verify_without_a_system_check_only_computes() {
        let r = Recorder { caps: caps(true, false, true), calls: AtomicUsize::new(0) };
        assert!(matches!(apply_checked(&r, &layout(), ApplyMode::Verify), Ok(Applied::Computed)));
        assert_eq!(r.calls.load(Ordering::SeqCst), 0);
        let v = Recorder { caps: caps(true, true, true), calls: AtomicUsize::new(0) };
        assert!(matches!(apply_checked(&v, &layout(), ApplyMode::Verify), Ok(Applied::Checked)));
        assert_eq!(v.calls.load(Ordering::SeqCst), 1);
    }
}
```

In `detect.rs` tests, a `FakeProbe { os, vars: HashMap, path: Vec<&str>, bus: Vec<&str> }`:
- `SCREEN_SIDE_BACKEND=kde` picks `Kind::Kde` whatever else is present; `SCREEN_SIDE_BACKEND=nope` is `Error::Usage`.
- os `windows` picks Windows; os `macos` picks Macos.
- linux with bus owner `org.gnome.Mutter.DisplayConfig` picks Gnome with `bus_name` set, even with `XDG_CURRENT_DESKTOP=KDE` absent; with only `org.cinnamon.Muffin.DisplayConfig` picks Gnome with that bus name.
- linux, `XDG_CURRENT_DESKTOP=KDE`, `kscreen-doctor` on path: Kde.
- linux, `XDG_CURRENT_DESKTOP=KDE`, no `kscreen-doctor`, `WAYLAND_DISPLAY` set, no `wlr-randr`: `NoBackend` whose message contains `kscreen-doctor`.
- linux, `WAYLAND_DISPLAY=wayland-1`, `wlr-randr` on path: Wlroots.
- linux, `WAYLAND_DISPLAY` set, no `wlr-randr`: `NoBackend` mentioning `wlr-randr`.
- linux, `DISPLAY=:0`, no `WAYLAND_DISPLAY`, `xrandr` on path: X11.
- linux, `WAYLAND_DISPLAY` and `DISPLAY` both set, `xrandr` on path, no `wlr-randr`: NOT X11 (XWayland); `NoBackend`.
- linux, nothing: `NoBackend` mentioning "no graphical session".

In `fake.rs` tests:
- `FakeFile::sample()` has two enabled screens, the built-in primary, origin TopLeft.
- `Fake::at(tmp)` with no file yet writes the sample on first query.
- `apply` with `Persistent` writes the new positions and primary, `last_apply == Some(Persistent)`.
- `apply` with `Verify` leaves the file byte-identical.
- `apply` whose layout names a screen that is not enabled returns `Error::Changed` and leaves the file unchanged.
- with `capabilities.primary == false`, `apply` leaves primary flags alone.

In `run.rs` tests: `Scripted` returns outputs in order and records calls; `SystemRunner.run("definitely-not-a-program-xyz", &[])` is `Error::Tool`; `on_path("sh")` is true on unix (`#[cfg(unix)]`).

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p screen-side-core backend run`
Expected: compile errors.

- [ ] **Step 3: Implement `run.rs`**

```rust
//! Running the desktop's own tools (kscreen-doctor, wlr-randr, xrandr,
//! gsettings). Backends take a `Runner` so tests can script the output and
//! check the exact command lines.

use std::path::Path;
use std::process::Command;

use crate::Error;

pub trait Runner: Send + Sync {
    /// Runs `program` with `args` and returns its standard output. A
    /// non-zero exit is an error carrying standard error.
    fn run(&self, program: &str, args: &[String]) -> Result<String, Error>;
}

pub struct SystemRunner;

impl Runner for SystemRunner {
    fn run(&self, program: &str, args: &[String]) -> Result<String, Error> {
        let out = Command::new(program).args(args).output().map_err(|e| Error::Tool {
            program: program.to_string(),
            message: e.to_string(),
        })?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(Error::Tool {
                program: program.to_string(),
                message: if stderr.is_empty() { format!("exited with {}", out.status) } else { stderr },
            });
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

/// True when `program` is an executable file in a directory on PATH.
pub fn on_path(program: &str) -> bool {
    which(program).is_some()
}

/// Where `program` is on PATH. The app shows this for the CLI.
pub fn which(program: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT".into())
            .split(';')
            .map(|e| e.to_string())
            .chain(std::iter::once(String::new()))
            .collect()
    } else {
        vec![String::new()]
    };
    std::env::split_paths(&path).find_map(|dir| {
        exts.iter().map(|ext| dir.join(format!("{program}{ext}"))).find(|p| is_executable(p))
    })
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata().map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
pub(crate) mod testing {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use super::Runner;
    use crate::Error;

    /// Replies with canned output in order and remembers every call.
    pub struct Scripted {
        replies: Mutex<VecDeque<(String, Result<String, String>)>>,
        calls: Mutex<Vec<Vec<String>>>,
    }

    impl Scripted {
        pub fn new(replies: Vec<(&str, Result<&str, &str>)>) -> Self {
            Scripted {
                replies: Mutex::new(
                    replies
                        .into_iter()
                        .map(|(p, r)| (p.to_string(), r.map(str::to_string).map_err(str::to_string)))
                        .collect(),
                ),
                calls: Mutex::new(Vec::new()),
            }
        }

        pub fn calls(&self) -> Vec<Vec<String>> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl Runner for Scripted {
        fn run(&self, program: &str, args: &[String]) -> Result<String, Error> {
            let mut call = vec![program.to_string()];
            call.extend(args.iter().cloned());
            self.calls.lock().unwrap().push(call);
            let (expected, reply) = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| panic!("unexpected call to {program}"));
            assert_eq!(expected, program, "called the wrong program");
            reply.map_err(|message| Error::Tool { program: program.to_string(), message })
        }
    }
}
```

- [ ] **Step 4: Implement `backend/mod.rs`**

```rust
//! One backend per desktop. Each reads the screens into a [`State`] in its
//! own coordinate space and applies a [`Layout`]; everything in between is
//! shared.

pub mod detect;
pub mod fake;

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
pub fn apply_checked(backend: &dyn Backend, layout: &Layout, mode: ApplyMode) -> Result<Applied, Error> {
    let caps = backend.capabilities();
    match mode {
        ApplyMode::Temporary if !caps.temporary && caps.remembers => Err(Error::Unsupported(format!(
            "The {} backend always saves the arrangement, so a temporary change is not available here.",
            backend.name()
        ))),
        ApplyMode::Verify if !caps.verify => Ok(Applied::Computed),
        ApplyMode::Verify => backend.apply(layout, mode).map(|()| Applied::Checked),
        _ => backend.apply(layout, mode).map(|()| Applied::Done),
    }
}
```

- [ ] **Step 5: Implement `backend/fake.rs`**

```rust
//! A backend made of a JSON file. For tests, CI runners with no screens,
//! and contributors with a single monitor.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::{ApplyMode, Backend, Capabilities};
use crate::layout::Origin;
use crate::model::{Identity, Layout, Rect, Screen, State};
use crate::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FakeFile {
    pub capabilities: Capabilities,
    pub screens: Vec<Screen>,
    #[serde(default)]
    pub last_apply: Option<ApplyMode>,
}

impl FakeFile {
    /// A 1280x800 laptop at scale 1.5 to the right of a 2560x1440 monitor.
    pub fn sample() -> FakeFile {
        FakeFile {
            capabilities: Capabilities { primary: true, temporary: true, verify: true, remembers: true, origin: Origin::TopLeft },
            screens: vec![
                Screen {
                    id: "eDP-1".into(),
                    connector: "eDP-1".into(),
                    name: "Built-in display".into(),
                    identity: Identity { vendor: "BOE".into(), product: "0x095f".into(), serial: String::new() },
                    builtin: true,
                    enabled: true,
                    primary: true,
                    rect: Rect::new(2560, 320, 1280, 800),
                    scale: 1.5,
                },
                Screen {
                    id: "HDMI-1".into(),
                    connector: "HDMI-1".into(),
                    name: "DELL U2723QE".into(),
                    identity: Identity { vendor: "DEL".into(), product: "0x41b5".into(), serial: "FAKE0001".into() },
                    builtin: false,
                    enabled: true,
                    primary: false,
                    rect: Rect::new(0, 0, 2560, 1440),
                    scale: 1.0,
                },
            ],
            last_apply: None,
        }
    }
}

pub struct Fake {
    path: Option<PathBuf>,
    memory: Mutex<FakeFile>,
}

impl Fake {
    /// `SCREEN_SIDE_FAKE_STATE` if set, else the sample in memory.
    pub fn from_env() -> Result<Fake, Error> {
        Ok(match std::env::var_os("SCREEN_SIDE_FAKE_STATE") {
            Some(path) => Fake::at(PathBuf::from(path)),
            None => Fake::in_memory(FakeFile::sample()),
        })
    }

    pub fn at(path: PathBuf) -> Fake {
        Fake { path: Some(path), memory: Mutex::new(FakeFile::sample()) }
    }

    pub fn in_memory(file: FakeFile) -> Fake {
        Fake { path: None, memory: Mutex::new(file) }
    }

    /// Swaps the screens, as a hotplug would. Tests only.
    pub fn replace(&self, file: FakeFile) {
        *self.memory.lock().unwrap() = file;
    }

    fn load(&self) -> Result<FakeFile, Error> {
        let Some(path) = &self.path else { return Ok(self.memory.lock().unwrap().clone()) };
        if !path.exists() {
            let sample = FakeFile::sample();
            self.store(&sample)?;
            return Ok(sample);
        }
        let text = std::fs::read_to_string(path)?;
        serde_json::from_str(&text).map_err(|e| Error::Config(format!("{}: {e}", path.display())))
    }

    fn store(&self, file: &FakeFile) -> Result<(), Error> {
        match &self.path {
            Some(path) => {
                let text = serde_json::to_string_pretty(file).expect("serialisable");
                std::fs::write(path, text + "\n")?;
            }
            None => *self.memory.lock().unwrap() = file.clone(),
        }
        Ok(())
    }
}

impl Backend for Fake {
    fn name(&self) -> &'static str {
        "fake"
    }

    fn capabilities(&self) -> Capabilities {
        self.load().map(|f| f.capabilities).unwrap_or(FakeFile::sample().capabilities)
    }

    fn query(&self) -> Result<State, Error> {
        Ok(State { backend: "fake".into(), screens: self.load()?.screens })
    }

    fn apply(&self, layout: &Layout, mode: ApplyMode) -> Result<(), Error> {
        let mut file = self.load()?;
        let mut enabled: Vec<&str> = file.screens.iter().filter(|s| s.enabled).map(|s| s.id.as_str()).collect();
        let mut named: Vec<&str> = layout.positions.iter().map(|p| p.id.as_str()).collect();
        enabled.sort_unstable();
        named.sort_unstable();
        if enabled != named {
            return Err(Error::Changed);
        }
        if mode == ApplyMode::Verify {
            return Ok(());
        }
        let primary_supported = file.capabilities.primary;
        for s in &mut file.screens {
            if let Some(p) = layout.position(&s.id) {
                s.rect.x = p.x;
                s.rect.y = p.y;
            }
            if primary_supported {
                s.primary = s.enabled && s.id == layout.primary;
            }
        }
        file.last_apply = Some(mode);
        self.store(&file)
    }
}
```

- [ ] **Step 6: Implement `backend/detect.rs`**

```rust
//! Picking the backend for this session.

use std::str::FromStr;

use super::Backend;
use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Fake,
    Gnome,
    Kde,
    Wlroots,
    X11,
    Windows,
    Macos,
}

impl Kind {
    pub const ALL: [Kind; 7] = [Kind::Fake, Kind::Gnome, Kind::Kde, Kind::Wlroots, Kind::X11, Kind::Windows, Kind::Macos];

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Fake => "fake",
            Kind::Gnome => "gnome",
            Kind::Kde => "kde",
            Kind::Wlroots => "wlroots",
            Kind::X11 => "x11",
            Kind::Windows => "windows",
            Kind::Macos => "macos",
        }
    }
}

impl FromStr for Kind {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Error> {
        Kind::ALL
            .into_iter()
            .find(|k| k.as_str() == s.trim().to_lowercase())
            .ok_or_else(|| Error::Usage(format!(
                "SCREEN_SIDE_BACKEND={s} is not a backend. Use one of: fake, gnome, kde, wlroots, x11, windows, macos."
            )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub kind: Kind,
    /// Why this one, in a sentence for `doctor`.
    pub reason: String,
    /// The Mutter-compatible bus name, for the gnome backend.
    pub bus_name: Option<String>,
}

pub trait Probe {
    /// `linux`, `windows`, `macos`, `freebsd`, ...: `std::env::consts::OS`.
    fn os(&self) -> &str;
    fn var(&self, name: &str) -> Option<String>;
    fn on_path(&self, program: &str) -> bool;
    fn bus_has_owner(&self, name: &str) -> bool;
}

pub struct SystemProbe;

impl Probe for SystemProbe {
    fn os(&self) -> &str {
        std::env::consts::OS
    }
    fn var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok().filter(|v| !v.is_empty())
    }
    fn on_path(&self, program: &str) -> bool {
        crate::run::on_path(program)
    }
    fn bus_has_owner(&self, name: &str) -> bool {
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            super::gnome::bus_has_owner(name)
        }
        #[cfg(not(all(unix, not(target_os = "macos"))))]
        {
            let _ = name;
            false
        }
    }
}

pub const MUTTER: &str = "org.gnome.Mutter.DisplayConfig";
pub const MUFFIN: &str = "org.cinnamon.Muffin.DisplayConfig";

pub fn choose(probe: &dyn Probe) -> Result<Choice, Error> {
    let pick = |kind: Kind, reason: &str| Choice { kind, reason: reason.to_string(), bus_name: None };
    if let Some(name) = probe.var("SCREEN_SIDE_BACKEND") {
        let kind: Kind = name.parse()?;
        let bus_name = (kind == Kind::Gnome).then(|| {
            if probe.bus_has_owner(MUFFIN) && !probe.bus_has_owner(MUTTER) { MUFFIN } else { MUTTER }.to_string()
        });
        return Ok(Choice { kind, reason: "chosen by SCREEN_SIDE_BACKEND".into(), bus_name });
    }
    match probe.os() {
        "windows" => return Ok(pick(Kind::Windows, "Windows display configuration API")),
        "macos" => return Ok(pick(Kind::Macos, "Quartz Display Services")),
        _ => {}
    }
    for bus in [MUTTER, MUFFIN] {
        if probe.bus_has_owner(bus) {
            return Ok(Choice {
                kind: Kind::Gnome,
                reason: format!("{bus} is on the session bus"),
                bus_name: Some(bus.to_string()),
            });
        }
    }
    let desktop = probe.var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_uppercase();
    let wayland = probe.var("WAYLAND_DISPLAY").is_some();
    let x11 = !wayland && probe.var("DISPLAY").is_some();
    if desktop.split(':').any(|d| d == "KDE") {
        if probe.on_path("kscreen-doctor") {
            return Ok(pick(Kind::Kde, "KDE Plasma with kscreen-doctor"));
        }
        return Err(Error::NoBackend(
            "This is KDE Plasma, but kscreen-doctor is not installed. Install the libkscreen package (it provides kscreen-doctor).".into(),
        ));
    }
    if wayland {
        if probe.on_path("wlr-randr") {
            return Ok(pick(Kind::Wlroots, "a Wayland session with wlr-randr"));
        }
        return Err(Error::NoBackend(
            "This is a Wayland session that is neither GNOME nor KDE. Install wlr-randr, which Sway, Hyprland, niri, river and labwc work with.".into(),
        ));
    }
    if x11 {
        if probe.on_path("xrandr") {
            return Ok(pick(Kind::X11, "an X11 session with xrandr"));
        }
        return Err(Error::NoBackend("This is an X11 session without xrandr. Install xrandr (x11-xserver-utils on Debian and Ubuntu).".into()));
    }
    Err(Error::NoBackend(
        "There is no graphical session here (neither WAYLAND_DISPLAY nor DISPLAY is set), so there are no screens to arrange.".into(),
    ))
}

pub fn create(choice: &Choice) -> Result<Box<dyn Backend>, Error> {
    let elsewhere = |what: &str| Error::Unsupported(format!("The {what} backend is not available on this operating system."));
    match choice.kind {
        Kind::Fake => Ok(Box::new(super::fake::Fake::from_env()?)),
        #[cfg(all(unix, not(target_os = "macos")))]
        Kind::Gnome => Ok(Box::new(super::gnome::Gnome::connect(choice.bus_name.as_deref().unwrap_or(MUTTER))?)),
        #[cfg(all(unix, not(target_os = "macos")))]
        Kind::Kde => Ok(Box::new(super::kde::Kde::new(Box::new(crate::run::SystemRunner)))),
        #[cfg(all(unix, not(target_os = "macos")))]
        Kind::Wlroots => Ok(Box::new(super::wlroots::Wlroots::new(Box::new(crate::run::SystemRunner)))),
        #[cfg(all(unix, not(target_os = "macos")))]
        Kind::X11 => Ok(Box::new(super::x11::X11::new(Box::new(crate::run::SystemRunner)))),
        #[cfg(windows)]
        Kind::Windows => Ok(Box::new(super::windows::Windows)),
        #[cfg(target_os = "macos")]
        Kind::Macos => Ok(Box::new(super::macos::Macos)),
        #[allow(unreachable_patterns)]
        other => Err(elsewhere(other.as_str())),
    }
}

pub fn detect() -> Result<(Box<dyn Backend>, Choice), Error> {
    let choice = choose(&SystemProbe)?;
    Ok((create(&choice)?, choice))
}
```

Until Tasks 9 to 14 add the real backends, gate their arms behind the modules existing: in this task write `create` with only the `Fake` arm and the fallback arm, and each backend task adds its arm. Likewise `SystemProbe::bus_has_owner` returns `false` until Task 9.

- [ ] **Step 7: Run all core tests**

Run: `cargo test -p screen-side-core`
Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add crates/core
git commit -m "feat(core): add the backend trait, detection and a file-backed fake backend"
```

### Task 5: Saved layouts and settings

**Files:**
- Create: `crates/core/src/store.rs`
- Modify: `crates/core/src/lib.rs` (`pub mod store;`)

**Interfaces:**
- Consumes: model, layout, `Error`.
- Produces:
  - `SavedLayout { name, screens: Vec<String>, anchor: String, placements: Vec<SavedPlacement>, align: Align, primary: String, auto: bool, saved: String }`, `SavedPlacement { screen: String, side: Side }`.
  - `Settings { background: bool, auto_apply: bool, shortcut: Option<String> }` with `Default` (background `cfg!(any(windows, target_os = "macos"))`, auto_apply true, shortcut None).
  - `pub fn fingerprint(state: &State) -> Vec<String>`, `pub fn keyed(state: &State) -> Vec<(String, String)>` (suffixed key, id), `pub fn capture(name: &str, state: &State, arr: &Arrangement, auto: bool) -> Result<SavedLayout, Error>`, `pub fn resolve(saved: &SavedLayout, state: &State) -> Result<Arrangement, Error>`, `pub fn matches(saved: &SavedLayout, state: &State) -> bool`, `SavedLayout::summary(&self) -> String`.
  - `Store::open() -> Result<Store, Error>`, `Store::at(dir: impl Into<PathBuf>) -> Store`, `dir(&self) -> &Path`, `layouts(&self) -> Result<Vec<SavedLayout>, Error>`, `find(&self, name: &str) -> Result<SavedLayout, Error>`, `put(&self, layout: SavedLayout) -> Result<(), Error>`, `forget(&self, name: &str) -> Result<(), Error>`, `set_auto(&self, name: &str, auto: bool) -> Result<(), Error>`, `settings(&self) -> Result<Settings, Error>`, `put_settings(&self, s: &Settings) -> Result<(), Error>`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::tests::{laptop, screen, state};
    use crate::layout::{Align, Placement, Side};
    use crate::model::Identity;

    fn monitor(id: &str, vendor: &str, product: &str, serial: &str) -> crate::model::Screen {
        let mut s = screen(id, 1920, 1080);
        s.identity = Identity { vendor: vendor.into(), product: product.into(), serial: serial.into() };
        s
    }

    fn desk() -> State {
        let mut l = laptop(1280, 800);
        l.identity = Identity { vendor: "BOE".into(), product: "0x095f".into(), serial: String::new() };
        state(vec![l, monitor("HDMI-1", "DEL", "0x41b5", "ABC")])
    }

    fn left_of() -> Arrangement {
        Arrangement {
            anchor: "eDP-1".into(),
            placements: vec![Placement { screen: "HDMI-1".into(), side: Side::Left }],
            align: Align::End,
            aligned: true,
            primary: "HDMI-1".into(),
        }
    }
```

Tests:

- `capture_and_resolve_round_trip`: capture "office" from `desk()` and `left_of`; `screens == ["boe:0x095f:", "del:0x41b5:abc"]`, anchor key `boe:0x095f:`, primary key `del:0x41b5:abc`; `resolve` on `desk()` returns the same Arrangement; `matches` is true.
- `resolve_on_other_screens_is_a_usage_error`: a state with a different monitor serial; `matches` false; `resolve` is `Error::Usage` whose message contains `office` and `other screens`.
- `identical_monitors_resolve_in_connector_order`: two `DEL 0x41b5` without serials on `DP-1` and `DP-2`, placements DP-1 left and DP-2 right; captured placement keys are `del:0x41b5:` and `del:0x41b5:#2`; `fingerprint` lists `del:0x41b5:` twice; resolve maps back to DP-1 left and DP-2 right.
- `fingerprint_ignores_switched_off_screens`: disabling eDP-1 removes its key.
- `names_are_checked`: `capture("", ...)` and a 65-character name are `Usage`; `"  Mum's desk  "` is stored as `Mum's desk`.
- `store_round_trip_and_replace_by_name` (tempdir): put "office", put "Office" with `auto: true` replaces it in place (one layout, auto true), `find("OFFICE")` works, `forget("office")` empties it, `forget("nope")` is `Usage` listing nothing; `set_auto` flips the flag.
- `missing_file_means_no_layouts`.
- `corrupt_file_is_refused_and_kept`: write `{ not json` to `layouts.json`; `layouts()` is `Error::Config` whose message contains the path and "fix or remove"; `put(...)` also fails and the file still reads `{ not json`.
- `newer_version_is_refused_and_kept`: write `{"version": 2, "layouts": []}`; `layouts()` is `Error::Config` containing "newer Screen Side"; `put` fails; file unchanged.
- `writes_are_atomic`: after `put`, no `*.tmp` file remains in the directory.
- `settings_default_and_round_trip`: missing file gives `Settings::default()`; put and read back `shortcut: Some("<Super><Alt>s")`.
- `summary_describes_the_layout`: `"1 screen left, aligned bottom"` style text, exactly `"DEL 0x41b5 left of the built-in screen, bottom aligned"` is not required; assert it contains `left` and `bottom`.

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p screen-side-core store`
Expected: compile errors.

- [ ] **Step 3: Implement `store.rs`**

```rust
//! Saved layouts and app settings, in the OS config directory.
//!
//! A layout is saved as intent (sides, order, alignment, primary) against
//! the identities of the screens connected when it was saved, never as
//! coordinates, so it survives a resolution or scale change.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::layout::{Align, Arrangement, Placement, Side};
use crate::model::State;
use crate::Error;

const LAYOUTS: &str = "layouts.json";
const SETTINGS: &str = "settings.json";
const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedPlacement {
    pub screen: String,
    pub side: Side,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedLayout {
    pub name: String,
    pub screens: Vec<String>,
    pub anchor: String,
    pub placements: Vec<SavedPlacement>,
    pub align: Align,
    pub primary: String,
    #[serde(default)]
    pub auto: bool,
    #[serde(default)]
    pub saved: String,
}

impl SavedLayout {
    /// One line for lists: "2 screens: 1 left, 1 above; centred".
    pub fn summary(&self) -> String {
        let mut counts: Vec<(Side, usize)> = Vec::new();
        for p in &self.placements {
            match counts.iter_mut().find(|(s, _)| *s == p.side) {
                Some((_, n)) => *n += 1,
                None => counts.push((p.side, 1)),
            }
        }
        let sides: Vec<String> = counts.iter().map(|(s, n)| format!("{n} {s}")).collect();
        let horizontal = self.placements.first().map_or(true, |p| p.side.horizontal());
        format!("{} screens: {}; {}", self.screens.len(), sides.join(", "), self.align.label(horizontal))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub background: bool,
    pub auto_apply: bool,
    pub shortcut: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            background: cfg!(any(windows, target_os = "macos")),
            auto_apply: true,
            shortcut: None,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct LayoutsFile {
    version: u32,
    layouts: Vec<SavedLayout>,
}

#[derive(Serialize, Deserialize)]
struct SettingsFile {
    version: u32,
    #[serde(flatten)]
    settings: Settings,
}

/// Sorted identity keys of the enabled screens: which desk this is.
pub fn fingerprint(state: &State) -> Vec<String> {
    let mut keys: Vec<String> = state.enabled().map(|s| s.key()).collect();
    keys.sort();
    keys
}

/// Each enabled screen's key, made unique: the second identical monitor
/// (by connector) gets `#2`, the third `#3`.
pub fn keyed(state: &State) -> Vec<(String, String)> {
    let mut screens: Vec<_> = state.enabled().collect();
    screens.sort_by(|a, b| a.connector.cmp(&b.connector));
    let mut seen: HashMap<String, usize> = HashMap::new();
    screens
        .into_iter()
        .map(|s| {
            let key = s.key();
            let n = seen.entry(key.clone()).or_insert(0);
            *n += 1;
            let unique = if *n == 1 { key } else { format!("{key}#{n}") };
            (unique, s.id.clone())
        })
        .collect()
}

fn clean_name(name: &str) -> Result<String, Error> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) {
        return Err(Error::Usage("A layout name needs 1 to 64 characters and no control characters.".into()));
    }
    Ok(name.to_string())
}

pub fn capture(name: &str, state: &State, arr: &Arrangement, auto: bool) -> Result<SavedLayout, Error> {
    let name = clean_name(name)?;
    let keys = keyed(state);
    let key_of = |id: &str| {
        keys.iter()
            .find(|(_, i)| i == id)
            .map(|(k, _)| k.clone())
            .ok_or_else(|| Error::Usage(format!("{id} is not switched on.")))
    };
    Ok(SavedLayout {
        name,
        screens: fingerprint(state),
        anchor: key_of(&arr.anchor)?,
        placements: arr
            .placements
            .iter()
            .map(|p| Ok(SavedPlacement { screen: key_of(&p.screen)?, side: p.side }))
            .collect::<Result<_, Error>>()?,
        align: arr.align,
        primary: key_of(&arr.primary)?,
        auto,
        saved: humantime::format_rfc3339_seconds(SystemTime::now()).to_string(),
    })
}

pub fn matches(saved: &SavedLayout, state: &State) -> bool {
    saved.screens == fingerprint(state)
}

pub fn resolve(saved: &SavedLayout, state: &State) -> Result<Arrangement, Error> {
    if !matches(saved, state) {
        return Err(Error::Usage(format!(
            "'{}' was saved for other screens than the ones connected now.",
            saved.name
        )));
    }
    let keys = keyed(state);
    let id_of = |key: &str| {
        keys.iter()
            .find(|(k, _)| k == key)
            .map(|(_, id)| id.clone())
            .ok_or_else(|| Error::Usage(format!("'{}' names a screen that is not connected.", saved.name)))
    };
    Ok(Arrangement {
        anchor: id_of(&saved.anchor)?,
        placements: saved
            .placements
            .iter()
            .map(|p| Ok(Placement { screen: id_of(&p.screen)?, side: p.side }))
            .collect::<Result<_, Error>>()?,
        align: saved.align,
        aligned: true,
        primary: id_of(&saved.primary)?,
    })
}

pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn open() -> Result<Store, Error> {
        if let Some(dir) = std::env::var_os("SCREEN_SIDE_CONFIG_DIR") {
            return Ok(Store::at(PathBuf::from(dir)));
        }
        let dirs = directories::ProjectDirs::from("io.github", "danieltyukov", "ScreenSide")
            .ok_or_else(|| Error::Config("There is no home directory to keep settings in.".into()))?;
        Ok(Store::at(dirs.config_dir()))
    }

    pub fn at(dir: impl Into<PathBuf>) -> Store {
        Store { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn read<T: for<'de> Deserialize<'de>>(&self, file: &str, version: impl Fn(&T) -> u32) -> Result<Option<T>, Error> {
        let path = self.dir.join(file);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let value: T = serde_json::from_str(&text).map_err(|e| {
            Error::Config(format!("{} could not be read ({e}). Fix or remove it; it has not been changed.", path.display()))
        })?;
        if version(&value) > VERSION {
            return Err(Error::Config(format!(
                "{} was written by a newer Screen Side. Update Screen Side to use it; it has not been changed.",
                path.display()
            )));
        }
        Ok(Some(value))
    }

    fn write(&self, file: &str, text: String) -> Result<(), Error> {
        std::fs::create_dir_all(&self.dir)?;
        let path = self.dir.join(file);
        let tmp = self.dir.join(format!("{file}.tmp"));
        std::fs::write(&tmp, text + "\n")?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    pub fn layouts(&self) -> Result<Vec<SavedLayout>, Error> {
        Ok(self.read::<LayoutsFile>(LAYOUTS, |f| f.version)?.map(|f| f.layouts).unwrap_or_default())
    }

    fn put_all(&self, layouts: Vec<SavedLayout>) -> Result<(), Error> {
        let text = serde_json::to_string_pretty(&LayoutsFile { version: VERSION, layouts }).expect("serialisable");
        self.write(LAYOUTS, text)
    }

    pub fn find(&self, name: &str) -> Result<SavedLayout, Error> {
        let layouts = self.layouts()?;
        layouts
            .iter()
            .find(|l| l.name.eq_ignore_ascii_case(name.trim()))
            .cloned()
            .ok_or_else(|| {
                let names: Vec<&str> = layouts.iter().map(|l| l.name.as_str()).collect();
                Error::Usage(if names.is_empty() {
                    format!("There is no saved layout called '{}'. No layouts are saved yet.", name.trim())
                } else {
                    format!("There is no saved layout called '{}'. Saved: {}.", name.trim(), names.join(", "))
                })
            })
    }

    pub fn put(&self, layout: SavedLayout) -> Result<(), Error> {
        let mut layouts = self.layouts()?;
        match layouts.iter_mut().find(|l| l.name.eq_ignore_ascii_case(&layout.name)) {
            Some(existing) => *existing = layout,
            None => layouts.push(layout),
        }
        self.put_all(layouts)
    }

    pub fn forget(&self, name: &str) -> Result<(), Error> {
        let found = self.find(name)?;
        let layouts = self.layouts()?.into_iter().filter(|l| l.name != found.name).collect();
        self.put_all(layouts)
    }

    pub fn set_auto(&self, name: &str, auto: bool) -> Result<(), Error> {
        let mut found = self.find(name)?;
        found.auto = auto;
        self.put(found)
    }

    pub fn settings(&self) -> Result<Settings, Error> {
        Ok(self.read::<SettingsFile>(SETTINGS, |f| f.version)?.map(|f| f.settings).unwrap_or_default())
    }

    pub fn put_settings(&self, settings: &Settings) -> Result<(), Error> {
        let text = serde_json::to_string_pretty(&SettingsFile { version: VERSION, settings: settings.clone() })
            .expect("serialisable");
        self.write(SETTINGS, text)
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p screen-side-core store`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/core
git commit -m "feat(core): save layouts per set of screens and keep app settings"
```

### Task 6: Watcher

**Files:**
- Create: `crates/core/src/watch.rs`
- Modify: `crates/core/src/lib.rs` (`pub mod watch;`), `crates/core/src/model.rs` (`Layout::same_as`), `crates/core/src/backend/mod.rs` (add `pub fn arrange(backend: &dyn Backend, state: &State, arr: &Arrangement, mode: ApplyMode) -> Result<(Layout, Applied), Error>` that computes with `backend.capabilities().origin` and calls `apply_checked`)

**Interfaces:**
- Consumes: `Backend`, `Store`, `store::{fingerprint, matches, resolve}`, `layout::compute`.
- Produces: `watch::{Event, Watcher}`; `Event::{ScreensChanged { screens: Vec<String> }, Moved, Applied { name: String }, Failed { message: String }}`; `Watcher::new(settle: Duration) -> Watcher`; `Watcher::tick(&mut self, backend: &dyn Backend, store: &Store, auto_apply: bool) -> Vec<Event>`; `pub fn run(backend: &dyn Backend, store: &Store, interval: Duration, auto_apply: impl Fn() -> bool, stop: &AtomicBool, on_event: impl FnMut(Event))`.

- [ ] **Step 1: Write the failing tests** with a `Fake::in_memory` and a tempdir `Store`:
- `first_tick_applies_a_matching_auto_layout`: the sample has HDMI-1 left of the laptop. Save an auto layout "office" captured from the sample with `baseline(&sample).move_all(Side::Right)`; the first tick returns `[ScreensChanged{..}, Applied{name: "office"}]` and the fake now has HDMI-1 to the right.
- `nothing_happens_when_nothing_changes`: second tick on the same state returns `[]`.
- `a_move_is_reported_without_applying`: change HDMI-1's x in the fake; tick returns `[Moved]` and the fake keeps the manual position.
- `a_hotplug_applies_the_layout_for_the_new_screens`: start with laptop only (one screen), tick, then replace with the sample; tick returns `ScreensChanged` then `Applied`.
- `auto_off_or_not_auto_does_not_apply`: same hotplug with `auto_apply == false` gives only `ScreensChanged`; with the layout's `auto: false` too.
- `an_applied_layout_does_not_apply_again`: after `Applied`, the next tick returns `[]` (its own apply did not loop).
- `already_in_place_is_not_reapplied`: when the auto layout equals the current positions, first tick gives `ScreensChanged` only and the fake's `last_apply` stays `None`.
- `errors_become_events`: a backend whose `query` fails yields `[Failed{..}]`, and the next tick still works; a corrupt `layouts.json` yields `Failed` mentioning the file.

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p screen-side-core watch`
Expected: compile errors.

- [ ] **Step 3: Implement**

```rust
//! Noticing new screens and putting a saved layout back.
//!
//! Only a change in the set of connected screens triggers a saved layout,
//! never a change of positions, so the watcher's own apply cannot set it
//! off again. Errors are reported and the loop carries on.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::backend::{arrange, ApplyMode, Backend};
use crate::layout::compute;
use crate::model::State;
use crate::store::{fingerprint, matches, resolve, Store};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    ScreensChanged { screens: Vec<String> },
    Moved,
    Applied { name: String },
    Failed { message: String },
}

pub struct Watcher {
    last: Option<State>,
    settle: Duration,
}

impl Watcher {
    pub fn new(settle: Duration) -> Watcher {
        Watcher { last: None, settle }
    }

    pub fn tick(&mut self, backend: &dyn Backend, store: &Store, auto_apply: bool) -> Vec<Event> {
        let state = match backend.query() {
            Ok(state) => state,
            Err(e) => return vec![Event::Failed { message: e.to_string() }],
        };
        let previous = self.last.replace(state.clone());
        let screens = fingerprint(&state);
        let set_changed = previous.as_ref().map_or(true, |p| fingerprint(p) != screens);
        let mut events = Vec::new();
        if set_changed {
            events.push(Event::ScreensChanged { screens: screens.clone() });
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

    fn apply_saved(&mut self, backend: &dyn Backend, store: &Store, screens: &[String]) -> Option<Event> {
        let layouts = match store.layouts() {
            Ok(layouts) => layouts,
            Err(e) => return Some(Event::Failed { message: e.to_string() }),
        };
        let current = self.last.clone()?;
        let saved = layouts.into_iter().find(|l| l.auto && matches(l, &current))?;
        std::thread::sleep(self.settle);
        let state = match backend.query() {
            Ok(state) if fingerprint(&state) == screens => state,
            Ok(_) => return None,
            Err(e) => return Some(Event::Failed { message: e.to_string() }),
        };
        let result = resolve(&saved, &state).and_then(|arr| {
            let target = compute(&state, &arr, backend.capabilities().origin)?;
            if target.same_as(&crate::model::Layout::from_state(&state)) {
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
            Err(e) => Some(Event::Failed { message: format!("Could not apply '{}': {e}", saved.name) }),
        }
    }
}

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
```

`Layout::from_state` lists positions in `state.enabled()` order and `compute` lists the anchor first, so the comparison ignores order. Add to `model.rs`, with a test that two layouts listing the same positions in different orders are the same and a different primary is not:

```rust
impl Layout {
    /// Same positions and primary, in any order.
    pub fn same_as(&self, other: &Layout) -> bool {
        let sorted = |l: &Layout| {
            let mut p = l.positions.clone();
            p.sort_by(|a, b| a.id.cmp(&b.id));
            p
        };
        self.primary == other.primary && sorted(self) == sorted(other)
    }
}
```

`arrange` in `backend/mod.rs`:

```rust
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
```

- [ ] **Step 4: Run all core tests** (`cargo test -p screen-side-core`); expected all pass.
- [ ] **Step 5: Commit** `feat(core): watch for new screens and apply their saved layout`.

### Task 7: Command line, status and moves

**Files:**
- Create: `crates/cli/Cargo.toml`, `crates/cli/src/main.rs`, `crates/cli/src/output.rs`, `crates/cli/src/select.rs`, `crates/cli/tests/cli.rs`, `crates/cli/tests/common/mod.rs`

**Interfaces:**
- Consumes: core (`detect`, `arrange`, `baseline`, `infer`, `Store`, `store::{matches}`, `Error::exit_code`).
- Produces: the `screen-side` binary; `select::resolve_screen(state: &State, selector: &str) -> Result<String, Error>`; `output::{status_text, status_json}`.

`crates/cli/Cargo.toml`:

```toml
[package]
name = "screen-side"
description = "Put your external screen left, right, above or below the built-in one"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
publish = false

[[bin]]
name = "screen-side"
path = "src/main.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
screen-side-core = { path = "../core" }
serde_json = "1"

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"
```

- [ ] **Step 1: Write the failing integration tests**

`tests/common/mod.rs`:

```rust
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use screen_side_core::backend::fake::FakeFile;
use tempfile::TempDir;

pub struct Env {
    pub dir: TempDir,
}

impl Env {
    pub fn new(file: FakeFile) -> Env {
        let dir = tempfile::tempdir().unwrap();
        let env = Env { dir };
        env.write(&file);
        env
    }

    pub fn state_path(&self) -> PathBuf {
        self.dir.path().join("state.json")
    }

    pub fn config(&self) -> PathBuf {
        self.dir.path().join("config")
    }

    pub fn write(&self, file: &FakeFile) {
        std::fs::write(self.state_path(), serde_json::to_string_pretty(file).unwrap()).unwrap();
    }

    pub fn read(&self) -> FakeFile {
        serde_json::from_str(&std::fs::read_to_string(self.state_path()).unwrap()).unwrap()
    }

    pub fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("screen-side").unwrap();
        cmd.env("SCREEN_SIDE_BACKEND", "fake")
            .env("SCREEN_SIDE_FAKE_STATE", self.state_path())
            .env("SCREEN_SIDE_CONFIG_DIR", self.config());
        cmd
    }
}

pub fn at(file: &FakeFile, id: &str) -> (i32, i32) {
    let s = file.screens.iter().find(|s| s.id == id).unwrap();
    (s.rect.x, s.rect.y)
}

#[allow(dead_code)]
pub fn exists(path: &Path) -> bool {
    path.exists()
}
```

`tests/cli.rs` cases (each a `#[test]`):
- `status_describes_the_sample`: stdout contains `External screen is left of the built-in screen.`, `Alignment: centred.`, `HDMI-1`, `DELL U2723QE`, `2560x1440`, `primary`, `Backend: fake`.
- `status_json_has_schema_and_numbers`: `--json` parses; `schema == 1`; `screens[0].number == 1`; `arrangement.placements[0].side == "left"`; no `serial` anywhere in the output.
- `right_moves_the_external_screen`: `right` exits 0, stdout contains `right of the built-in screen`; state has HDMI-1 at x 1280 and eDP-1 at x 0; `last_apply == Some(Persistent)`.
- `temporary_and_dry_run`: `left --temporary` gives `last_apply == Some(Temporary)`; `right --dry-run` leaves the file byte-identical and prints `Dry run` and `checked`.
- `toggle_flips`: two toggles return to the starting positions.
- `align_end_and_primary`: `align end` puts both bottoms level; `primary 1` (HDMI-1 is number 1 in the sample, being leftmost) makes HDMI-1 primary in the file and the laptop not primary.
- `screen_selector`: with a third screen `DP-1 "DELL P2422H"` (1920x1080 at 3840,320, right of the laptop): `left --screen dell` exits 2 and stderr lists both DELL screens; `left --screen DP-1` moves only DP-1 (HDMI-1 keeps its side); `left --screen 9` exits 2.
- `switched_off_screen_is_listed_and_refused`: disable eDP-1 (and make HDMI-1 primary): status lists `eDP-1` with `off`; `left --screen eDP-1` exits 2 with `switched off`.
- `one_screen_is_a_clear_error`: only HDMI-1 enabled: `left` exits 1, stderr `Only one screen is switched on`.
- `overlap_is_refused_without_changes`: laptop 1280x800, A 2560x1440 left of it, B 2560x1440 right of it; `above --screen B --align center` exits 1, stderr contains `would overlap`, file unchanged.
- `primary_unsupported`: capabilities.primary false: `primary 1` exits 2 with `no primary screen`.
- `temporary_refused_where_the_system_saves`: capabilities `temporary: false, remembers: true`: `left --temporary` exits 2.
- `version_and_help`: `--version` prints `2.0.0`; `--help` lists `toggle`, `save`, `doctor`.

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p screen-side`
Expected: compile failure, no binary.

- [ ] **Step 3: Implement `select.rs`**

```rust
use screen_side_core::model::State;
use screen_side_core::Error;

/// A screen by its number in `status`, its connector, or part of its name.
pub fn resolve_screen(state: &State, selector: &str) -> Result<String, Error> {
    let listed = state.listed();
    let wanted = selector.trim();
    let chosen = if let Ok(n) = wanted.parse::<usize>() {
        listed.get(n.wrapping_sub(1)).copied().ok_or_else(|| {
            Error::Usage(format!("There is no screen {n}. Run screen-side status to see the numbers."))
        })?
    } else if let Some(s) = listed.iter().find(|s| s.connector.eq_ignore_ascii_case(wanted)) {
        s
    } else {
        let lower = wanted.to_lowercase();
        let hits: Vec<_> = listed
            .iter()
            .enumerate()
            .filter(|(_, s)| s.name.to_lowercase().contains(&lower))
            .collect();
        match hits.as_slice() {
            [(_, s)] => s,
            [] => return Err(Error::Usage(format!("No screen matches '{wanted}'. Run screen-side status to see them."))),
            many => {
                let names: Vec<String> = many.iter().map(|(i, s)| format!("{} {} {}", i + 1, s.connector, s.name)).collect();
                return Err(Error::Usage(format!(
                    "'{wanted}' matches {} screens: {}. Use the number or the connector.",
                    many.len(),
                    names.join(", ")
                )));
            }
        }
    };
    if !chosen.enabled {
        return Err(Error::Usage(format!("{} ({}) is switched off.", chosen.name, chosen.connector)));
    }
    Ok(chosen.id.clone())
}
```

- [ ] **Step 4: Implement `output.rs`**

`status_text(state, arrangement: Option<&Arrangement>, choice_reason: &str, backend_name: &str, layout: Option<&str>) -> String` builds:
- Headline: with an arrangement whose anchor is built-in and one placement: `External screen is {side} of the built-in screen.`; anchor not built-in: `{name} is {side} of {anchor name}.`; several placements: one clause per screen joined by `; ` ("DELL U2723QE is left; LG HDR 4K is above"); no arrangement: `The arrangement is not a simple side-by-side layout.`; one enabled screen: `Only one screen is switched on.`
- `Alignment: {label}.` appended when there are placements, `custom` when `!aligned`.
- One line per listed screen: `  {n:>2}  {connector:<10} {name:<24} {w}x{h:<10} at {x},{y:<8} scale {scale}` plus `  primary` or `  off`, using `{:g}` for scale.
- `Backend: {backend_name} ({choice_reason}).` and, when `layout` is `Some`, ` Saved layout in force: {name}.`

`status_json(...)` returns `serde_json::Value` with `schema`, `backend`, `capabilities`, `screens` (number, id, connector, name, builtin, enabled, primary, x, y, width, height, scale; no identity), `arrangement` (or null), `layout` (or null).

`moved_text(state, arr, side, applied)`: `"{name} is now {side} of the built-in screen."` plus the pointer sentence for one screen (`The pointer leaves the {edge} edge of the built-in screen and enters the {opposite} edge of {name}.`); for dry runs prefix `Dry run: ` and suffix `(checked by the system)` for `Applied::Checked` or `(computed; this system cannot check without applying)` for `Computed`, and print the positions.

- [ ] **Step 5: Implement `main.rs`**

```rust
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use screen_side_core::backend::{arrange, detect, ApplyMode, Applied, Backend, Choice};
use screen_side_core::layout::{baseline, infer, Align, Arrangement, Side};
use screen_side_core::model::State;
use screen_side_core::store::{matches, Store};
use screen_side_core::Error;

mod output;
mod select;

#[derive(Parser)]
#[command(name = "screen-side", version, about = "Put your external screen left, right, above or below the built-in one.")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Print machine-readable JSON.
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Args, Clone, Copy)]
struct ApplyArgs {
    /// Do not save the change; it reverts on the next reconnect.
    #[arg(long)]
    temporary: bool,
    /// Work out the change and check it without applying it.
    #[arg(long)]
    dry_run: bool,
}

impl ApplyArgs {
    fn mode(self) -> ApplyMode {
        if self.dry_run {
            ApplyMode::Verify
        } else if self.temporary {
            ApplyMode::Temporary
        } else {
            ApplyMode::Persistent
        }
    }
}

#[derive(Args)]
struct MoveArgs {
    /// Which screen to move: its number from status, its connector, or part of its name.
    #[arg(long)]
    screen: Option<String>,
    /// How the screens line up on the other axis: start, center or end.
    #[arg(long)]
    align: Option<Align>,
    #[command(flatten)]
    apply: ApplyArgs,
}

#[derive(Subcommand)]
enum Command {
    /// Show the screens and how they are arranged (the default).
    Status,
    /// Put the external screen left of the built-in one.
    Left(MoveArgs),
    /// Put the external screen right of the built-in one.
    Right(MoveArgs),
    /// Put the external screen above the built-in one.
    Above(MoveArgs),
    /// Put the external screen below the built-in one.
    Below(MoveArgs),
    /// Swap left with right and above with below. Meant for a keyboard shortcut.
    Toggle(ApplyArgs),
    /// Make a screen the primary one.
    Primary { screen: String, #[command(flatten)] apply: ApplyArgs },
    /// Line the screens up at the start, centre or end of the shared edge.
    Align { align: Align, #[command(flatten)] apply: ApplyArgs },
    // save, apply, layouts, forget, watch: Task 8. shortcut: Task 15. doctor: Task 16.
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("screen-side: {e}");
            ExitCode::from(e.exit_code() as u8)
        }
    }
}

struct Session {
    backend: Box<dyn Backend>,
    choice: Choice,
    store: Store,
    state: State,
}

impl Session {
    fn open() -> Result<Session, Error> {
        let (backend, choice) = detect()?;
        let state = backend.query()?;
        Ok(Session { backend, choice, store: Store::open()?, state })
    }

    /// The saved layout for these screens that the screens are in, if any.
    /// A broken layouts file never stops a status or a move.
    fn layout_in_force(&self) -> Option<String> {
        let layouts = self.store.layouts().ok()?;
        let arr = infer(&self.state)?;
        layouts
            .into_iter()
            .filter(|l| matches(l, &self.state))
            .find(|l| screen_side_core::store::resolve(l, &self.state).ok().as_ref() == Some(&arr))
            .map(|l| l.name)
    }
}

fn run(cli: Cli) -> Result<(), Error> {
    let session = Session::open()?;
    match cli.command.unwrap_or(Command::Status) {
        Command::Status => print_status(&session, cli.json),
        Command::Left(m) => move_to(&session, Side::Left, m, cli.json),
        Command::Right(m) => move_to(&session, Side::Right, m, cli.json),
        Command::Above(m) => move_to(&session, Side::Above, m, cli.json),
        Command::Below(m) => move_to(&session, Side::Below, m, cli.json),
        Command::Toggle(a) => {
            let arr = baseline(&session.state)?.toggled();
            finish(&session, &arr, a.mode(), cli.json)
        }
        Command::Primary { screen, apply } => {
            if !session.backend.capabilities().primary {
                return Err(Error::Unsupported(format!(
                    "The {} backend has no primary screen to set.",
                    session.backend.name()
                )));
            }
            let id = select::resolve_screen(&session.state, &screen)?;
            let arr = baseline(&session.state)?.with_primary(&id);
            finish(&session, &arr, apply.mode(), cli.json)
        }
        Command::Align { align, apply } => {
            let arr = baseline(&session.state)?.with_align(align);
            finish(&session, &arr, apply.mode(), cli.json)
        }
    }
}

fn move_to(session: &Session, side: Side, args: MoveArgs, json: bool) -> Result<(), Error> {
    let mut arr = baseline(&session.state)?;
    if let Some(align) = args.align {
        arr = arr.with_align(align);
    }
    arr = match &args.screen {
        Some(sel) => arr.move_screen(&select::resolve_screen(&session.state, sel)?, side)?,
        None => arr.move_all(side),
    };
    finish(session, &arr, args.apply.mode(), json)
}

fn finish(session: &Session, arr: &Arrangement, mode: ApplyMode, json: bool) -> Result<(), Error> {
    let (layout, applied) = arrange(session.backend.as_ref(), &session.state, arr, mode)?;
    if json {
        let value = if mode == ApplyMode::Verify {
            output::dry_run_json(&layout, applied)
        } else {
            let state = session.backend.query()?;
            output::status_json(&state, infer(&state).as_ref(), session.backend.as_ref(), None)
        };
        println!("{}", serde_json::to_string_pretty(&value).expect("serialisable"));
    } else {
        print!("{}", output::moved_text(&session.state, arr, &layout, applied));
    }
    Ok(())
}

fn print_status(session: &Session, json: bool) -> Result<(), Error> {
    let arr = infer(&session.state);
    let layout = session.layout_in_force();
    if json {
        let value = output::status_json(&session.state, arr.as_ref(), session.backend.as_ref(), layout.as_deref());
        println!("{}", serde_json::to_string_pretty(&value).expect("serialisable"));
    } else {
        print!(
            "{}",
            output::status_text(&session.state, arr.as_ref(), session.backend.name(), &session.choice.reason, layout.as_deref())
        );
    }
    Ok(())
}
```

Note `ApplyMode::Verify` paired with `Applied::Done` cannot happen; `Applied` is used only for the dry-run wording. `Applied` import is used by `output`.

- [ ] **Step 6: Run the CLI tests and the whole suite**

Run: `cargo test -p screen-side && cargo clippy -p screen-side-core -p screen-side --all-targets -- -D warnings`
Expected: all pass, no warnings.

- [ ] **Step 7: Commit** `feat(cli): add status, sides, toggle, primary and align with JSON and dry runs`.

### Task 8: Command line, saved layouts and watch

**Files:**
- Modify: `crates/cli/src/main.rs`, `crates/cli/src/output.rs`, `crates/cli/tests/cli.rs`

**Interfaces:**
- Consumes: `store::{capture, resolve, Store}`, `watch::run`.
- Produces: subcommands `save NAME [--auto]`, `apply NAME [--temporary] [--dry-run]`, `layouts [--json]`, `forget NAME`, `watch [--interval SECONDS]`.

- [ ] **Step 1: Write the failing tests**
- `save_apply_layouts_forget`: `save office --auto` exits 0 and prints `Saved 'office'`; `right`; `apply office` puts HDMI-1 back on the left; `layouts` lists `office`, `auto`, and `these screens`; `layouts --json` has `schema 1` and `layouts[0].matches == true`; `status` ends with `Saved layout in force: office.`; `forget office` then `layouts` prints `No layouts are saved yet.`
- `apply_for_other_screens_is_refused`: save, change HDMI-1's identity serial in the state file, `apply office` exits 2 with `other screens`.
- `layout_name_with_a_quote`: `save "Mum's desk"` and `apply "mum's desk"` both exit 0.
- `corrupt_layouts_file`: write `{ not json` to `config/layouts.json`; `left` exits 0; `status` exits 0 without a saved-layout line; `save x` exits 1 naming the file; the file still reads `{ not json`.
- `watch_rejects_a_bad_interval`: `watch --interval 0` exits 2.
- `watch` itself loops forever, so its behaviour is covered by the core tests in Task 6; here only check that `watch --help` mentions saved layouts.

- [ ] **Step 2: Run to see them fail** (`cargo test -p screen-side`).

- [ ] **Step 3: Implement.** Add to `Command`:

```rust
    /// Save the current arrangement for the screens connected now.
    Save {
        name: String,
        /// Apply it automatically whenever these screens are connected.
        #[arg(long)]
        auto: bool,
    },
    /// Apply a saved layout.
    Apply { name: String, #[command(flatten)] apply: ApplyArgs },
    /// List saved layouts.
    Layouts,
    /// Delete a saved layout.
    Forget { name: String },
    /// Keep running and apply saved layouts marked --auto when their screens are connected.
    Watch {
        /// Seconds between checks.
        #[arg(long, default_value_t = 2.0)]
        interval: f64,
    },
```

Handlers:
- `Save`: `let arr = baseline(&state)?; let saved = capture(&name, &state, &arr, auto)?; store.put(saved)?;` print `Saved '{name}' for these screens.` plus ` It will be applied whenever they are connected.` with `--auto`.
- `Apply`: `let saved = store.find(&name)?; let arr = resolve(&saved, &state)?; finish(...)`.
- `Layouts`: text lines `  {name:<20} {summary}  {"auto" or ""}  {"these screens" or "other screens"}`; JSON `{ schema, layouts: [{name, auto, matches, screens: n, summary, saved}] }`; empty prints `No layouts are saved yet. Save one with: screen-side save NAME`.
- `Forget`: `store.forget(&name)?`, print `Forgot '{name}'.`
- `Watch`: validate `interval >= 0.5` else `Error::Usage("--interval must be at least 0.5 seconds.")`; print `Watching for screen changes. Press Ctrl+C to stop.`; `watch::run(backend, &store, Duration::from_secs_f64(interval), || true, &AtomicBool::new(false), |e| println!("{}", output::event_text(&e)))` where `event_text` prints `Screens changed: {n} connected.`, `Applied '{name}'.`, `Problem: {message}` and nothing for `Moved`.

The session currently loads state before the command; `Layouts` and `Forget` do not need a backend. Make `Session::open` lazy: open the store first and only detect a backend for commands that need one, so `layouts` and `forget` work over SSH or on a headless machine.

- [ ] **Step 4: Run** `cargo test -p screen-side` and clippy; expected all pass.
- [ ] **Step 5: Commit** `feat(cli): save, apply, list and forget layouts, and watch for screen changes`.

---

## Phase 2: real backends, shortcut and doctor

Each backend is split in two: a pure part that converts the system's raw data to a `State` and a `Layout` back to the system's request (compiled and tested on every OS), and a thin part that talks to the system (compiled only where it can run). The pure parts are where the tests live.

### Task 9: GNOME (Mutter) backend

**Files:**
- Create: `crates/core/src/backend/mutter.rs` (pure), `crates/core/src/backend/gnome.rs` (zbus), `crates/core/tests/fixtures/mutter/{laptop-left,fractional,physical,lid-closed,mirror}.json`, `crates/core/tests/real.rs`
- Modify: `crates/core/Cargo.toml`, `crates/core/src/backend/mod.rs`, `crates/core/src/backend/detect.rs` (Gnome arm, `bus_has_owner`)

**Interfaces:**
- Produces: `mutter::{MutterState, MutterMonitor, MutterMode, MutterLogical, ApplyLogical, LAYOUT_LOGICAL, LAYOUT_PHYSICAL}`, `MutterState::to_state(&self) -> State`, `MutterState::apply_config(&self, layout: &Layout) -> Result<Vec<ApplyLogical>, Error>`; `gnome::{Gnome, bus_has_owner}`, `Gnome::connect(bus: &str) -> Result<Gnome, Error>`.

Cargo:

```toml
[target.'cfg(all(unix, not(target_os = "macos")))'.dependencies]
zbus = "5"
```

- [ ] **Step 1: Write the fixtures.** Each is a serialised `MutterState`. `laptop-left.json`:

```json
{
  "serial": 7,
  "layout_mode": 1,
  "supports_changing_layout_mode": false,
  "monitors": [
    { "connector": "eDP-1", "vendor": "BOE", "product": "0x095f", "serial": "", "display_name": "Built-in display", "builtin": true,
      "modes": [ { "id": "2560x1600@60.000", "width": 2560, "height": 1600, "current": true, "preferred": true } ] },
    { "connector": "HDMI-1", "vendor": "DEL", "product": "DELL U2723QE", "serial": "ABC123", "display_name": "Dell Inc. 27\"", "builtin": false,
      "modes": [ { "id": "2560x1440@59.951", "width": 2560, "height": 1440, "current": true, "preferred": true },
                 { "id": "1920x1080@60.000", "width": 1920, "height": 1080, "current": false, "preferred": false } ] }
  ],
  "logical": [
    { "x": 2560, "y": 320, "scale": 2.0, "transform": 0, "primary": true, "connectors": ["eDP-1"] },
    { "x": 0, "y": 0, "scale": 1.0, "transform": 0, "primary": false, "connectors": ["HDMI-1"] }
  ]
}
```

`fractional.json`: eDP-1 2560x1600 at scale 1.5 (expect 1707x1067), HDMI-1 1920x1080 at scale 1.25 (expect 1536x864), DP-1 1920x1080 transform 1 at scale 1 (expect 1080x1920), `layout_mode` 1. `physical.json`: same as `laptop-left` with `layout_mode` 2 (expect eDP-1 2560x1600). `lid-closed.json`: eDP-1 listed with modes but in no logical monitor. `mirror.json`: one logical monitor with `["eDP-1", "HDMI-1"]`.

- [ ] **Step 2: Write the failing tests in `mutter.rs`** that load each fixture with `include_str!("../../tests/fixtures/mutter/<name>.json")`:
- `laptop_left`: two enabled screens; eDP-1 rect `(2560, 320, 1280, 800)`, built-in, primary, name `Built-in display`, identity key `boe:0x095f:`; HDMI-1 rect `(0, 0, 2560, 1440)`, name `Dell Inc. 27"`.
- `fractional_sizes_round_like_mutter`: the three sizes above.
- `physical_mode_ignores_scale`: eDP-1 width 2560.
- `lid_closed_is_listed_but_off`: eDP-1 `enabled == false`, rect zero.
- `mirror_lists_the_second_as_off`.
- `apply_config_keeps_scale_transform_and_modes`: from `laptop-left`, a layout with HDMI-1 at (1280,0), eDP-1 at (0,0) primary eDP-1 gives two `ApplyLogical` with the right positions, `scale` 2.0 and 1.0, mode ids `2560x1600@60.000` and `2560x1440@59.951`, primary only on eDP-1.
- `apply_config_keeps_mirrors_together`: from `mirror`, a single-position layout yields one logical with both connectors.
- `apply_config_spots_changed_screens`: a layout naming `DP-9`, or missing HDMI-1, is `Error::Changed`.

- [ ] **Step 3: Run to see them fail** (`cargo test -p screen-side-core mutter`).

- [ ] **Step 4: Implement `mutter.rs`**

```rust
//! Mutter's DisplayConfig state as plain data, and the conversions to and
//! from it. No D-Bus here, so this compiles and is tested everywhere.

use serde::{Deserialize, Serialize};

use crate::model::{Identity, Layout, Rect, Screen, State};
use crate::Error;

/// Positions in scaled pixels: a 2560 wide panel at scale 2 is 1280 wide.
pub const LAYOUT_LOGICAL: u32 = 1;
/// Positions in device pixels: the same panel is 2560 wide.
pub const LAYOUT_PHYSICAL: u32 = 2;
/// Transforms that swap width and height (90 and 270, flipped or not).
const ROTATED: [u32; 4] = [1, 3, 5, 7];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutterMode { pub id: String, pub width: i32, pub height: i32, pub current: bool, pub preferred: bool }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutterMonitor {
    pub connector: String,
    pub vendor: String,
    pub product: String,
    pub serial: String,
    pub display_name: String,
    pub builtin: bool,
    pub modes: Vec<MutterMode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutterLogical { pub x: i32, pub y: i32, pub scale: f64, pub transform: u32, pub primary: bool, pub connectors: Vec<String> }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutterState {
    pub serial: u32,
    pub layout_mode: u32,
    pub supports_changing_layout_mode: bool,
    pub monitors: Vec<MutterMonitor>,
    pub logical: Vec<MutterLogical>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplyLogical {
    pub x: i32,
    pub y: i32,
    pub scale: f64,
    pub transform: u32,
    pub primary: bool,
    /// (connector, mode id)
    pub monitors: Vec<(String, String)>,
}

impl MutterState {
    fn monitor(&self, connector: &str) -> Option<&MutterMonitor> {
        self.monitors.iter().find(|m| m.connector == connector)
    }

    fn mode(&self, connector: &str) -> Option<&MutterMode> {
        let m = self.monitor(connector)?;
        m.modes.iter().find(|m| m.current).or_else(|| m.modes.iter().find(|m| m.preferred))
    }

    /// The size this logical monitor occupies in the active coordinate space.
    fn extent(&self, logical: &MutterLogical) -> (i32, i32) {
        let Some(mode) = logical.connectors.first().and_then(|c| self.mode(c)) else { return (0, 0) };
        let (mut w, mut h) = (mode.width, mode.height);
        if ROTATED.contains(&logical.transform) {
            std::mem::swap(&mut w, &mut h);
        }
        if self.layout_mode == LAYOUT_LOGICAL && logical.scale > 0.0 {
            // Rounded the way Mutter rounds, or adjacent monitors leave a
            // one-pixel seam that the pointer cannot cross.
            w = (w as f64 / logical.scale).round() as i32;
            h = (h as f64 / logical.scale).round() as i32;
        }
        (w, h)
    }

    pub fn to_state(&self) -> State {
        let screens = self
            .monitors
            .iter()
            .map(|m| {
                let logical = self.logical.iter().find(|l| l.connectors.contains(&m.connector));
                let leads = logical.is_some_and(|l| l.connectors.first() == Some(&m.connector));
                let (rect, primary, scale) = match logical {
                    Some(l) if leads => {
                        let (w, h) = self.extent(l);
                        (Rect::new(l.x, l.y, w, h), l.primary, l.scale)
                    }
                    _ => (Rect::default(), false, 1.0),
                };
                let name = if !m.display_name.trim().is_empty() {
                    m.display_name.trim().to_string()
                } else if m.builtin {
                    "Built-in display".to_string()
                } else {
                    format!("{} {}", m.vendor, m.product).trim().to_string()
                };
                Screen {
                    id: m.connector.clone(),
                    connector: m.connector.clone(),
                    name,
                    identity: Identity { vendor: m.vendor.clone(), product: m.product.clone(), serial: m.serial.clone() },
                    builtin: m.builtin,
                    enabled: leads,
                    primary,
                    rect,
                    scale,
                }
            })
            .collect();
        State { backend: "gnome".into(), screens }
    }

    pub fn apply_config(&self, layout: &Layout) -> Result<Vec<ApplyLogical>, Error> {
        let mut leads: Vec<&str> = self.logical.iter().filter_map(|l| l.connectors.first().map(String::as_str)).collect();
        let mut named: Vec<&str> = layout.positions.iter().map(|p| p.id.as_str()).collect();
        leads.sort_unstable();
        named.sort_unstable();
        if leads != named {
            return Err(Error::Changed);
        }
        self.logical
            .iter()
            .map(|l| {
                let lead = &l.connectors[0];
                let p = layout.position(lead).ok_or(Error::Changed)?;
                let monitors = l
                    .connectors
                    .iter()
                    .map(|c| {
                        let mode = self.mode(c).ok_or_else(|| Error::System(format!("{c} reports no usable mode.")))?;
                        Ok((c.clone(), mode.id.clone()))
                    })
                    .collect::<Result<_, Error>>()?;
                Ok(ApplyLogical {
                    x: p.x,
                    y: p.y,
                    scale: l.scale,
                    transform: l.transform,
                    primary: *lead == layout.primary,
                    monitors,
                })
            })
            .collect()
    }
}
```

- [ ] **Step 5: Implement `gnome.rs`** (gated `#[cfg(all(unix, not(target_os = "macos")))]` in `backend/mod.rs`; `mutter` is not gated)

```rust
//! GNOME and other Mutter desktops over the session bus. Cinnamon's Muffin
//! offers the same interface under another name.

use std::collections::HashMap;

use zbus::blocking::Connection;
use zbus::zvariant::{OwnedValue, Value};

use super::mutter::{MutterLogical, MutterMode, MutterMonitor, MutterState};
use super::{ApplyMode, Backend, Capabilities};
use crate::layout::Origin;
use crate::model::{Layout, State};
use crate::Error;

type Spec = (String, String, String, String);
type RawMode = (String, i32, i32, f64, f64, Vec<f64>, HashMap<String, OwnedValue>);
type RawMonitor = (Spec, Vec<RawMode>, HashMap<String, OwnedValue>);
type RawLogical = (i32, i32, f64, u32, bool, Vec<Spec>, HashMap<String, OwnedValue>);
type RawState = (u32, Vec<RawMonitor>, Vec<RawLogical>, HashMap<String, OwnedValue>);
type ApplyMonitor<'a> = (String, String, HashMap<&'a str, Value<'a>>);
type ApplyLogicalRaw<'a> = (i32, i32, f64, u32, bool, Vec<ApplyMonitor<'a>>);

pub fn bus_has_owner(name: &str) -> bool {
    let Ok(conn) = Connection::session() else { return false };
    let Ok(proxy) = zbus::blocking::fdo::DBusProxy::new(&conn) else { return false };
    let Ok(bus_name) = zbus::names::BusName::try_from(name) else { return false };
    proxy.name_has_owner(bus_name).unwrap_or(false)
}

fn flag(props: &HashMap<String, OwnedValue>, key: &str) -> Option<bool> {
    props.get(key).and_then(|v| bool::try_from(v).ok())
}

fn text(props: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    props.get(key).and_then(|v| <&str>::try_from(v).ok()).map(str::to_string)
}

fn builtin_connector(connector: &str) -> bool {
    let c = connector.to_uppercase();
    c.starts_with("EDP") || c.starts_with("LVDS") || c.starts_with("DSI")
}

pub struct Gnome {
    conn: Connection,
    bus: String,
    path: String,
}

impl Gnome {
    pub fn connect(bus: &str) -> Result<Gnome, Error> {
        let conn = Connection::session().map_err(|e| Error::System(format!("Cannot reach the session bus: {e}")))?;
        Ok(Gnome { conn, bus: bus.to_string(), path: format!("/{}", bus.replace('.', "/")) })
    }

    fn raw(&self) -> Result<MutterState, Error> {
        let reply = self
            .conn
            .call_method(Some(self.bus.as_str()), self.path.as_str(), Some(self.bus.as_str()), "GetCurrentState", &())
            .map_err(|e| Error::System(format!("{} did not answer: {e}", self.bus)))?;
        let (serial, monitors, logical, props): RawState =
            reply.body().deserialize().map_err(|e| Error::System(format!("Unexpected reply from {}: {e}", self.bus)))?;
        Ok(MutterState {
            serial,
            layout_mode: props.get("layout-mode").and_then(|v| u32::try_from(v).ok()).unwrap_or(super::mutter::LAYOUT_LOGICAL),
            supports_changing_layout_mode: flag(&props, "supports-changing-layout-mode").unwrap_or(false),
            monitors: monitors
                .into_iter()
                .map(|((connector, vendor, product, serial), modes, mprops)| MutterMonitor {
                    builtin: flag(&mprops, "is-builtin").unwrap_or_else(|| builtin_connector(&connector)),
                    display_name: text(&mprops, "display-name").unwrap_or_default(),
                    modes: modes
                        .into_iter()
                        .map(|(id, width, height, _refresh, _pref_scale, _scales, p)| MutterMode {
                            id,
                            width,
                            height,
                            current: flag(&p, "is-current").unwrap_or(false),
                            preferred: flag(&p, "is-preferred").unwrap_or(false),
                        })
                        .collect(),
                    connector,
                    vendor,
                    product,
                    serial,
                })
                .collect(),
            logical: logical
                .into_iter()
                .map(|(x, y, scale, transform, primary, specs, _)| MutterLogical {
                    x,
                    y,
                    scale,
                    transform,
                    primary,
                    connectors: specs.into_iter().map(|s| s.0).collect(),
                })
                .collect(),
        })
    }
}

impl Backend for Gnome {
    fn name(&self) -> &'static str {
        "gnome"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities { primary: true, temporary: true, verify: true, remembers: true, origin: Origin::TopLeft }
    }

    fn query(&self) -> Result<State, Error> {
        self.raw().map(|raw| raw.to_state())
    }

    fn apply(&self, layout: &Layout, mode: ApplyMode) -> Result<(), Error> {
        let raw = self.raw()?;
        let config = raw.apply_config(layout)?;
        let method: u32 = match mode {
            ApplyMode::Verify => 0,
            ApplyMode::Temporary => 1,
            ApplyMode::Persistent => 2,
        };
        let logical: Vec<ApplyLogicalRaw> = config
            .into_iter()
            .map(|l| {
                let monitors = l.monitors.into_iter().map(|(c, m)| (c, m, HashMap::new())).collect();
                (l.x, l.y, l.scale, l.transform, l.primary, monitors)
            })
            .collect();
        let mut props: HashMap<&str, Value> = HashMap::new();
        if raw.supports_changing_layout_mode {
            props.insert("layout-mode", Value::from(raw.layout_mode));
        }
        self.conn
            .call_method(
                Some(self.bus.as_str()),
                self.path.as_str(),
                Some(self.bus.as_str()),
                "ApplyMonitorsConfig",
                &(raw.serial, method, logical, props),
            )
            .map(|_| ())
            .map_err(|e| Error::System(format!("Mutter refused the arrangement: {e}")))
    }

    fn diagnostics(&self) -> Vec<(String, String)> {
        let mut lines = vec![("bus".to_string(), self.bus.clone())];
        if let Ok(raw) = self.raw() {
            let mode = if raw.layout_mode == super::mutter::LAYOUT_PHYSICAL { "physical" } else { "logical" };
            lines.push(("layout mode".into(), mode.into()));
        }
        lines
    }
}
```

Add the `Gnome` arm to `create` and the real `bus_has_owner` to `SystemProbe`.

- [ ] **Step 6: Add `crates/core/tests/real.rs`**, which only runs with `SCREEN_SIDE_REAL=1`:

```rust
//! Talks to the real display system. Skipped unless SCREEN_SIDE_REAL=1.
//! CI sets it on the macOS and Windows runners; on a desktop it is safe to
//! run, because the only apply it makes is a check (or, where the system
//! cannot check, a re-apply of the arrangement already in force).

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
    let mode = if backend.capabilities().verify { ApplyMode::Verify } else { ApplyMode::Temporary };
    apply_checked(backend.as_ref(), &current, mode).expect("the current layout is accepted");
    let after = backend.query().expect("query again");
    assert_eq!(Layout::from_state(&after).positions.len(), current.positions.len());
}
```

- [ ] **Step 7: Run the tests, then the real check on this GNOME session**

Run: `cargo test -p screen-side-core && SCREEN_SIDE_REAL=1 cargo test -p screen-side-core --test real -- --nocapture`
Expected: all pass; the real test prints this machine's screens from Mutter and Mutter accepts the verify.

Run: `cargo run -q -p screen-side -- status` and `cargo run -q -p screen-side -- status --json`
Expected: this machine's monitors, backend `gnome`.

- [ ] **Step 8: Commit** `feat(core): add the GNOME backend over Mutter DisplayConfig`.

### Task 10: KDE Plasma backend

**Files:**
- Create: `crates/core/src/backend/kde.rs`, `crates/core/tests/fixtures/kde/{plasma6,plasma5-rotated}.json`
- Modify: `backend/mod.rs`, `backend/detect.rs` (Kde arm)

**Interfaces:**
- Produces: `kde::{Kde, parse, apply_args}`: `pub fn parse(json: &str) -> Result<State, Error>`, `pub fn apply_args(json: &str, layout: &Layout) -> Result<Vec<String>, Error>`, `Kde::new(runner: Box<dyn Runner>) -> Kde`.

Format, from libkscreen's `ConfigSerializer` (`kscreen-doctor --json`): `{"outputs":[{"id":1,"name":"eDP-1","type":7,"enabled":true,"connected":true,"priority":1,"pos":{"x":0,"y":0},"scale":1.5,"rotation":1,"currentModeId":"3","replicationSource":0,"modes":[{"id":"3","name":"2560x1600@60","size":{"width":2560,"height":1600},"refreshRate":60.0}]}],"screen":{...}}`. Type 7 is `Panel`. Rotation flags: 1 none, 2 left, 4 inverted, 8 right. Plasma 5 has `"primary": true` instead of `priority`. A non-zero `replicationSource` means the output mirrors another.

- [ ] **Step 1: Write the fixtures** by hand in that format. `plasma6.json`: eDP-1 (type 7, 2560x1600, scale 1.5, priority 1, pos 2560,0) and HDMI-A-1 (type 6, 2560x1440, scale 1, priority 2, pos 0,0), plus a disconnected DP-1 (`connected: false`). `plasma5-rotated.json`: eDP-1 with `"primary": true`, no `priority`, and DP-2 at rotation 2 with mode 1920x1080 and scale 1.

- [ ] **Step 2: Write the failing tests**:
- `plasma6`: two screens (disconnected DP-1 absent); eDP-1 built-in, primary, rect `(2560, 0, 1707, 1067)` (2560/1.5 rounded, 1600/1.5 rounded); HDMI-A-1 `(0, 0, 2560, 1440)`; identity key `connector:hdmi-a-1`; names `Built-in display` and `HDMI-A-1`.
- `rotation_swaps_before_scaling`: DP-2 is 1080x1920.
- `plasma5_primary_flag`: eDP-1 primary.
- `apply_args_plasma6`: positions and `output.eDP-1.priority.1` in this exact list: `["output.HDMI-A-1.position.0,0", "output.eDP-1.position.2560,0", "output.eDP-1.priority.1"]` (positions in output order, primary last).
- `apply_args_plasma5`: ends with `output.eDP-1.primary`.
- `apply_args_changed`: a layout naming `DP-9` is `Error::Changed`.
- `backend_runs_kscreen_doctor` with `Scripted`: `query` calls `kscreen-doctor --json`; `apply` calls `kscreen-doctor --json` then `kscreen-doctor <args>`; `apply` with `Verify` is never called by `apply_checked` because `verify` is false (assert via `apply_checked` returning `Computed` and only zero calls).
- `bad_json_is_a_tool_error_naming_kscreen_doctor`.

- [ ] **Step 3: Run to see them fail.**

- [ ] **Step 4: Implement**

```rust
//! KDE Plasma (Wayland and X11) through kscreen-doctor, which ships with
//! libkscreen. KScreen saves every change itself, so there is no temporary
//! mode, and its JSON carries no EDID, so screens are known by connector.

use serde::Deserialize;

use super::{ApplyMode, Backend, Capabilities};
use crate::layout::Origin;
use crate::model::{Identity, Layout, Rect, Screen, State};
use crate::run::Runner;
use crate::Error;

const PANEL: i64 = 7;
const ROTATION_LEFT: i64 = 2;
const ROTATION_RIGHT: i64 = 8;

#[derive(Deserialize)]
struct Config {
    outputs: Vec<Output>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Output {
    name: String,
    #[serde(rename = "type", default)]
    kind: i64,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    connected: bool,
    priority: Option<u32>,
    primary: Option<bool>,
    #[serde(default)]
    pos: Point,
    #[serde(default = "one")]
    scale: f64,
    #[serde(default = "one_i")]
    rotation: i64,
    #[serde(default)]
    current_mode_id: String,
    #[serde(default)]
    replication_source: i64,
    #[serde(default)]
    modes: Vec<Mode>,
}

#[derive(Deserialize, Default)]
struct Point {
    x: i32,
    y: i32,
}

#[derive(Deserialize)]
struct Mode {
    id: String,
    size: Size,
}

#[derive(Deserialize)]
struct Size {
    width: i32,
    height: i32,
}

fn one() -> f64 {
    1.0
}
fn one_i() -> i64 {
    1
}

fn load(json: &str) -> Result<Config, Error> {
    serde_json::from_str(json).map_err(|e| Error::Tool {
        program: "kscreen-doctor".into(),
        message: format!("unexpected --json output ({e})"),
    })
}

fn builtin(o: &Output) -> bool {
    let c = o.name.to_uppercase();
    o.kind == PANEL || c.starts_with("EDP") || c.starts_with("LVDS") || c.starts_with("DSI")
}

pub fn parse(json: &str) -> Result<State, Error> {
    let config = load(json)?;
    let screens = config
        .outputs
        .iter()
        .filter(|o| o.connected)
        .map(|o| {
            let enabled = o.enabled && o.replication_source == 0;
            let rect = match o.modes.iter().find(|m| m.id == o.current_mode_id) {
                Some(m) if enabled => {
                    let (mut w, mut h) = (m.size.width, m.size.height);
                    if o.rotation == ROTATION_LEFT || o.rotation == ROTATION_RIGHT {
                        std::mem::swap(&mut w, &mut h);
                    }
                    let scale = if o.scale > 0.0 { o.scale } else { 1.0 };
                    Rect::new(o.pos.x, o.pos.y, (w as f64 / scale).round() as i32, (h as f64 / scale).round() as i32)
                }
                _ => Rect::default(),
            };
            Screen {
                id: o.name.clone(),
                connector: o.name.clone(),
                name: if builtin(o) { "Built-in display".into() } else { o.name.clone() },
                identity: Identity::default(),
                builtin: builtin(o),
                enabled,
                primary: enabled && (o.priority == Some(1) || o.primary == Some(true)),
                rect,
                scale: o.scale,
            }
        })
        .collect();
    Ok(State { backend: "kde".into(), screens })
}

pub fn apply_args(json: &str, layout: &Layout) -> Result<Vec<String>, Error> {
    let state = parse(json)?;
    let mut on: Vec<&str> = state.enabled().map(|s| s.id.as_str()).collect();
    let mut named: Vec<&str> = layout.positions.iter().map(|p| p.id.as_str()).collect();
    on.sort_unstable();
    named.sort_unstable();
    if on != named {
        return Err(Error::Changed);
    }
    let plasma6 = load(json)?.outputs.iter().any(|o| o.priority.is_some());
    let mut args: Vec<String> = state
        .enabled()
        .filter_map(|s| layout.position(&s.id))
        .map(|p| format!("output.{}.position.{},{}", p.id, p.x, p.y))
        .collect();
    args.push(if plasma6 {
        format!("output.{}.priority.1", layout.primary)
    } else {
        format!("output.{}.primary", layout.primary)
    });
    Ok(args)
}

pub struct Kde {
    runner: Box<dyn Runner>,
}

impl Kde {
    pub fn new(runner: Box<dyn Runner>) -> Kde {
        Kde { runner }
    }

    fn json(&self) -> Result<String, Error> {
        self.runner.run("kscreen-doctor", &["--json".to_string()])
    }
}

impl Backend for Kde {
    fn name(&self) -> &'static str {
        "kde"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities { primary: true, temporary: false, verify: false, remembers: true, origin: Origin::TopLeft }
    }

    fn query(&self) -> Result<State, Error> {
        parse(&self.json()?)
    }

    fn apply(&self, layout: &Layout, _mode: ApplyMode) -> Result<(), Error> {
        let args = apply_args(&self.json()?, layout)?;
        self.runner.run("kscreen-doctor", &args).map(|_| ())
    }
}
```

The `positions` order in `apply_args` follows `state.enabled()`, which is the JSON's output order; the test's expected list matches the fixture's order (HDMI-A-1 listed first in `plasma6.json`).

- [ ] **Step 5: Run the tests**; expected pass. **Step 6: Commit** `feat(core): add the KDE Plasma backend through kscreen-doctor`.

### Task 11: wlroots backend (Sway, Hyprland, niri, river, labwc)

**Files:**
- Create: `crates/core/src/backend/wlroots.rs`, `crates/core/tests/fixtures/wlroots/{sway,rotated}.json`
- Modify: `backend/mod.rs`, `backend/detect.rs`

**Interfaces:**
- Produces: `wlroots::{Wlroots, parse, apply_args}` with `parse(json: &str) -> Result<State, Error>`, `apply_args(json: &str, layout: &Layout, verify: bool) -> Result<Vec<String>, Error>`.

Format (wlr-randr 0.3+, `print_state_json`): an array of heads `{"name","description","make","model","serial","physical_size":{"width","height"},"enabled","modes":[{"width","height","refresh","preferred","current"}]}`; enabled heads also have `"position":{"x","y"}`, `"transform"` (`normal`, `90`, `180`, `270`, `flipped`, `flipped-90`, `flipped-180`, `flipped-270`), `"scale"`, `"adaptive_sync"`. `make`, `model`, `serial` may be `null`. `--dryrun` tests a configuration without applying it; several `--output` groups form one atomic configuration.

- [ ] **Step 1: Fixtures.** `sway.json`: eDP-1 (make "BOE", model "0x095F", serial null, 2560x1600 current, scale 1.5, position 0,0) and DP-1 (make "Dell Inc.", model "DELL U2723QE", serial "ABC123", 3840x2160 current, scale 1.5, position 1706,0), plus HDMI-A-1 disabled (no position). `rotated.json`: DP-2 1920x1080 transform "90" scale 1.

- [ ] **Step 2: Failing tests**:
- `sizes_truncate_like_wlroots`: eDP-1 width `2560 / 1.5 = 1706.67` becomes 1706 and height 1066; DP-1 becomes 2560x1440; eDP-1 built-in; identity `boe:0x095f:`; DP-1 name `Dell Inc. DELL U2723QE`; HDMI-A-1 listed and off.
- `transform_swaps`: DP-2 is 1080x1920.
- `nobody_is_primary`: no screen has `primary`.
- `apply_args`: exactly `["--output", "eDP-1", "--pos", "2560,0", "--output", "DP-1", "--pos", "0,0"]`; with `verify` the list ends with `--dryrun`.
- `apply_args_changed`.
- Scripted backend: `query` runs `wlr-randr --json`; `apply_checked(..., Verify)` runs `wlr-randr --json` then the args with `--dryrun`; capabilities `primary: false, temporary: false, verify: true, remembers: false`.

- [ ] **Step 3: Run to see them fail.**

- [ ] **Step 4: Implement** in the same shape as `kde.rs`:

```rust
//! wlroots compositors (Sway, Hyprland, niri, river, labwc, Wayfire) through
//! wlr-randr, which speaks wlr-output-management. Nothing persists: the
//! compositor forgets on reconnect, which saved layouts and `watch` cover.

use serde::Deserialize;

use super::{ApplyMode, Backend, Capabilities};
use crate::layout::Origin;
use crate::model::{Identity, Layout, Rect, Screen, State};
use crate::run::Runner;
use crate::Error;

#[derive(Deserialize)]
struct Head {
    name: String,
    make: Option<String>,
    model: Option<String>,
    serial: Option<String>,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    modes: Vec<Mode>,
    position: Option<Point>,
    transform: Option<String>,
    scale: Option<f64>,
}

#[derive(Deserialize)]
struct Mode {
    width: i32,
    height: i32,
    #[serde(default)]
    current: bool,
}

#[derive(Deserialize)]
struct Point {
    x: i32,
    y: i32,
}

fn heads(json: &str) -> Result<Vec<Head>, Error> {
    serde_json::from_str(json).map_err(|e| Error::Tool {
        program: "wlr-randr".into(),
        message: format!("unexpected --json output ({e}); wlr-randr 0.3 or newer is needed"),
    })
}

pub fn parse(json: &str) -> Result<State, Error> {
    let screens = heads(json)?
        .into_iter()
        .map(|h| {
            let scale = h.scale.filter(|s| *s > 0.0).unwrap_or(1.0);
            let rect = match (h.enabled, h.position.as_ref(), h.modes.iter().find(|m| m.current)) {
                (true, Some(p), Some(m)) => {
                    let (mut w, mut h2) = (m.width, m.height);
                    if matches!(h.transform.as_deref(), Some("90" | "270" | "flipped-90" | "flipped-270")) {
                        std::mem::swap(&mut w, &mut h2);
                    }
                    // wlroots truncates (wlr_output_effective_resolution).
                    Rect::new(p.x, p.y, (w as f64 / scale) as i32, (h2 as f64 / scale) as i32)
                }
                _ => Rect::default(),
            };
            let upper = h.name.to_uppercase();
            let builtin = upper.starts_with("EDP") || upper.starts_with("LVDS") || upper.starts_with("DSI");
            let make = h.make.clone().unwrap_or_default();
            let model = h.model.clone().unwrap_or_default();
            let name = if builtin {
                "Built-in display".to_string()
            } else {
                let joined = format!("{make} {model}").trim().to_string();
                if joined.is_empty() { h.name.clone() } else { joined }
            };
            Screen {
                id: h.name.clone(),
                connector: h.name.clone(),
                name,
                identity: Identity { vendor: make, product: model, serial: h.serial.clone().unwrap_or_default() },
                builtin,
                enabled: h.enabled && rect.width > 0,
                primary: false,
                rect,
                scale,
            }
        })
        .collect();
    Ok(State { backend: "wlroots".into(), screens })
}

pub fn apply_args(json: &str, layout: &Layout, verify: bool) -> Result<Vec<String>, Error> {
    let state = parse(json)?;
    let mut on: Vec<&str> = state.enabled().map(|s| s.id.as_str()).collect();
    let mut named: Vec<&str> = layout.positions.iter().map(|p| p.id.as_str()).collect();
    on.sort_unstable();
    named.sort_unstable();
    if on != named {
        return Err(Error::Changed);
    }
    let mut args = Vec::new();
    for s in state.enabled() {
        let p = layout.position(&s.id).ok_or(Error::Changed)?;
        args.extend(["--output".to_string(), s.id.clone(), "--pos".to_string(), format!("{},{}", p.x, p.y)]);
    }
    if verify {
        args.push("--dryrun".into());
    }
    Ok(args)
}

pub struct Wlroots {
    runner: Box<dyn Runner>,
}

impl Wlroots {
    pub fn new(runner: Box<dyn Runner>) -> Wlroots {
        Wlroots { runner }
    }
    fn json(&self) -> Result<String, Error> {
        self.runner.run("wlr-randr", &["--json".to_string()])
    }
}

impl Backend for Wlroots {
    fn name(&self) -> &'static str {
        "wlroots"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities { primary: false, temporary: false, verify: true, remembers: false, origin: Origin::TopLeft }
    }
    fn query(&self) -> Result<State, Error> {
        parse(&self.json()?)
    }
    fn apply(&self, layout: &Layout, mode: ApplyMode) -> Result<(), Error> {
        let args = apply_args(&self.json()?, layout, mode == ApplyMode::Verify)?;
        self.runner.run("wlr-randr", &args).map(|_| ())
    }
    fn diagnostics(&self) -> Vec<(String, String)> {
        ["SWAYSOCK", "HYPRLAND_INSTANCE_SIGNATURE", "NIRI_SOCKET"]
            .iter()
            .filter(|v| std::env::var_os(v).is_some())
            .map(|v| ("compositor".to_string(), v.to_string()))
            .collect()
    }
}
```

The test's expected `apply_args` order (`eDP-1` then `DP-1`) follows the fixture's head order.

- [ ] **Step 5: Run; Step 6: Commit** `feat(core): add the wlroots backend through wlr-randr`.

### Task 12: X11 backend

**Files:**
- Create: `crates/core/src/backend/x11.rs`, `crates/core/src/edid.rs`
- Modify: `lib.rs` (`pub mod edid;`), `backend/mod.rs`, `backend/detect.rs`

**Interfaces:**
- Produces: `edid::{Edid, parse_edid}` with `Edid { vendor: String, product: String, serial: String, name: String }` and `pub fn parse_edid(bytes: &[u8]) -> Option<Edid>`; `x11::{X11, parse, apply_args}`.

- [ ] **Step 1: Failing EDID tests** in `edid.rs` with a builder:

```rust
#[cfg(test)]
pub(crate) fn build(vendor: &str, product: u16, serial_number: u32, serial_text: Option<&str>, name: &str) -> Vec<u8> {
    let mut b = vec![0u8; 128];
    b[..8].copy_from_slice(&[0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00]);
    let letters: Vec<u16> = vendor.bytes().map(|c| (c - b'A' + 1) as u16).collect();
    let id = (letters[0] << 10) | (letters[1] << 5) | letters[2];
    b[8..10].copy_from_slice(&id.to_be_bytes());
    b[10..12].copy_from_slice(&product.to_le_bytes());
    b[12..16].copy_from_slice(&serial_number.to_le_bytes());
    let mut put = |slot: usize, tag: u8, text: &str| {
        let at = 54 + slot * 18;
        b[at + 3] = tag;
        let mut field = [b' '; 13];
        let bytes = text.as_bytes();
        field[..bytes.len()].copy_from_slice(bytes);
        if bytes.len() < 13 {
            field[bytes.len()] = 0x0a;
        }
        b[at + 5..at + 18].copy_from_slice(&field);
    };
    put(1, 0xfc, name);
    if let Some(s) = serial_text {
        put(2, 0xff, s);
    }
    b
}
```

Tests: `DEL`, `0x41b5`, serial text `ABC123`, name `DELL U2723QE` decode to `vendor "DEL"`, `product "0x41b5"`, `serial "ABC123"`, `name "DELL U2723QE"`; with no serial text the numeric serial `12345678` decodes to `"12345678"`; numeric 0 and no text gives `""`; a 64-byte slice or a bad header gives `None`.

- [ ] **Step 2: Implement `edid.rs`**

```rust
//! Just enough of EDID to recognise a monitor: maker, product, serial, name.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edid {
    pub vendor: String,
    pub product: String,
    pub serial: String,
    pub name: String,
}

const HEADER: [u8; 8] = [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00];

fn descriptor_text(bytes: &[u8], tag: u8) -> Option<String> {
    (0..4).find_map(|slot| {
        let at = 54 + slot * 18;
        let d = &bytes[at..at + 18];
        (d[0] == 0 && d[1] == 0 && d[2] == 0 && d[3] == tag).then(|| {
            let text: String = d[5..18].iter().take_while(|c| **c != 0x0a).map(|c| *c as char).collect();
            text.trim().to_string()
        })
    })
}

pub fn parse_edid(bytes: &[u8]) -> Option<Edid> {
    if bytes.len() < 128 || bytes[..8] != HEADER {
        return None;
    }
    let id = u16::from_be_bytes([bytes[8], bytes[9]]);
    let letter = |shift: u16| (((id >> shift) & 0x1f) as u8 + b'A' - 1) as char;
    let vendor: String = [letter(10), letter(5), letter(0)].iter().collect();
    let product = format!("0x{:04x}", u16::from_le_bytes([bytes[10], bytes[11]]));
    let number = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    let serial = descriptor_text(bytes, 0xff)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| if number == 0 { String::new() } else { number.to_string() });
    Some(Edid { vendor, product, serial, name: descriptor_text(bytes, 0xfc).unwrap_or_default() })
}
```

- [ ] **Step 3: Failing X11 tests.** The fixture is generated in the test from a template so the EDID is real:

```rust
fn hex_lines(bytes: &[u8]) -> String {
    bytes.chunks(16).map(|c| format!("\t\t{}\n", c.iter().map(|b| format!("{b:02x}")).collect::<String>())).collect()
}

fn sample() -> String {
    let laptop = crate::edid::build("BOE", 0x095f, 0, None, "");
    let dell = crate::edid::build("DEL", 0x41b5, 0, Some("ABC123"), "DELL U2723QE");
    format!(
        "Screen 0: minimum 8 x 8, current 4480 x 1440, maximum 32767 x 32767\n\
eDP-1 connected primary 1920x1080+2560+180 (normal left inverted right x axis y axis) 309mm x 174mm\n\
\tEDID: \n{}\
\tscaling mode: Full aspect \n\
   1920x1080     60.01*+  59.97  \n\
HDMI-1 connected 2560x1440+0+0 (normal left inverted right x axis y axis) 597mm x 336mm\n\
\tEDID: \n{}\
   2560x1440     59.95*+\n\
DP-1 connected (normal left inverted right x axis y axis)\n\
DP-2 disconnected (normal left inverted right x axis y axis)\n\
HDMI-2 connected 1080x1920+4480+0 left (normal left inverted right x axis y axis) 527mm x 296mm\n",
        hex_lines(&laptop),
        hex_lines(&dell)
    )
}
```

Tests: four connected screens (DP-2 absent); eDP-1 `(2560, 180, 1920, 1080)`, built-in, primary, name `Built-in display`, key `boe:0x095f:`; HDMI-1 `(0, 0, 2560, 1440)`, name `DELL U2723QE`, key `del:0x41b5:abc123`; DP-1 connected without geometry is off; HDMI-2 rotated `left` is `1080x1920` as printed; `apply_args` for a layout with primary HDMI-1 is exactly `["--output", "eDP-1", "--pos", "2560x180", "--output", "HDMI-1", "--pos", "0x0", "--primary", "--output", "HDMI-2", "--pos", "4480x0"]`; `apply_args_changed`; Scripted backend runs `xrandr --current --props` for query; capabilities `primary: true, temporary: false, verify: false, remembers: false`.

- [ ] **Step 4: Implement `x11.rs`**

```rust
//! Plain X11 desktops (Xfce, MATE, i3, Openbox and the like) through
//! xrandr. `--current` reads without re-probing outputs, which would make
//! screens flicker on every status.

use super::{ApplyMode, Backend, Capabilities};
use crate::edid::parse_edid;
use crate::layout::Origin;
use crate::model::{Identity, Layout, Rect, Screen, State};
use crate::run::Runner;
use crate::Error;

fn geometry(token: &str) -> Option<Rect> {
    let (size, rest) = token.split_once('+')?;
    let (x, y) = rest.split_once('+')?;
    let (w, h) = size.split_once('x')?;
    Some(Rect::new(x.parse().ok()?, y.parse().ok()?, w.parse().ok()?, h.parse().ok()?))
}

fn decode_hex(hex: &str) -> Vec<u8> {
    (0..hex.len() / 2).filter_map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok()).collect()
}

pub fn parse(text: &str) -> Result<State, Error> {
    let mut screens: Vec<Screen> = Vec::new();
    let mut edid_hex: Option<String> = None;
    let finish_edid = |screens: &mut Vec<Screen>, hex: &mut Option<String>| {
        if let (Some(hex), Some(last)) = (hex.take(), screens.last_mut()) {
            if let Some(e) = parse_edid(&decode_hex(&hex)) {
                last.identity = Identity { vendor: e.vendor, product: e.product, serial: e.serial };
                if !last.builtin && !e.name.is_empty() {
                    last.name = e.name;
                }
            }
        }
    };
    for line in text.lines() {
        if let Some(hex) = edid_hex.as_mut() {
            if let Some(chunk) = line.strip_prefix("\t\t") {
                hex.push_str(chunk.trim());
                continue;
            }
            finish_edid(&mut screens, &mut edid_hex);
        }
        if line.starts_with("\tEDID:") {
            edid_hex = Some(String::new());
            continue;
        }
        if line.starts_with(char::is_whitespace) || line.starts_with("Screen ") {
            continue;
        }
        let mut words = line.split_whitespace();
        let (Some(name), Some(status)) = (words.next(), words.next()) else { continue };
        if status != "connected" {
            continue;
        }
        let rest: Vec<&str> = words.collect();
        let primary = rest.first() == Some(&"primary");
        let rect = rest.iter().take(2).find_map(|t| geometry(t));
        let upper = name.to_uppercase();
        let builtin = upper.starts_with("EDP") || upper.starts_with("LVDS") || upper.starts_with("DSI");
        screens.push(Screen {
            id: name.to_string(),
            connector: name.to_string(),
            name: if builtin { "Built-in display".into() } else { name.to_string() },
            identity: Identity::default(),
            builtin,
            enabled: rect.is_some(),
            primary: primary && rect.is_some(),
            rect: rect.unwrap_or_default(),
            scale: 1.0,
        });
    }
    finish_edid(&mut screens, &mut edid_hex);
    Ok(State { backend: "x11".into(), screens })
}

pub fn apply_args(text: &str, layout: &Layout) -> Result<Vec<String>, Error> {
    let state = parse(text)?;
    let mut on: Vec<&str> = state.enabled().map(|s| s.id.as_str()).collect();
    let mut named: Vec<&str> = layout.positions.iter().map(|p| p.id.as_str()).collect();
    on.sort_unstable();
    named.sort_unstable();
    if on != named {
        return Err(Error::Changed);
    }
    let mut args = Vec::new();
    for s in state.enabled() {
        let p = layout.position(&s.id).ok_or(Error::Changed)?;
        args.extend(["--output".to_string(), s.id.clone(), "--pos".to_string(), format!("{}x{}", p.x, p.y)]);
        if s.id == layout.primary {
            args.push("--primary".into());
        }
    }
    Ok(args)
}

pub struct X11 {
    runner: Box<dyn Runner>,
}

impl X11 {
    pub fn new(runner: Box<dyn Runner>) -> X11 {
        X11 { runner }
    }
    fn text(&self) -> Result<String, Error> {
        self.runner.run("xrandr", &["--current".to_string(), "--props".to_string()])
    }
}

impl Backend for X11 {
    fn name(&self) -> &'static str {
        "x11"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities { primary: true, temporary: false, verify: false, remembers: false, origin: Origin::TopLeft }
    }
    fn query(&self) -> Result<State, Error> {
        parse(&self.text()?)
    }
    fn apply(&self, layout: &Layout, _mode: ApplyMode) -> Result<(), Error> {
        let args = apply_args(&self.text()?, layout)?;
        self.runner.run("xrandr", &args).map(|_| ())
    }
}
```

The closure `finish_edid` borrows nothing outside its arguments, so it can be called in the loop; if the borrow checker objects, make it a private `fn`.

- [ ] **Step 5: Run; Step 6: Commit** `feat(core): add the X11 backend through xrandr with EDID identities`.

### Task 13: Windows backend

**Files:**
- Create: `crates/core/src/backend/ccd.rs` (pure), `crates/core/src/backend/windows.rs` (`#[cfg(windows)]`)
- Modify: `crates/core/Cargo.toml`, `backend/mod.rs`, `backend/detect.rs`

Cargo:

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.61", features = ["Win32_Devices_Display", "Win32_Foundation", "Win32_Graphics_Gdi"] }
```

**Interfaces:**
- Produces: `ccd::{CcdPath, to_state, pnp_id, screen_id, BUILTIN_TECHNOLOGIES}`; `CcdPath { adapter_low: u32, adapter_high: i32, source_id: u32, target_id: u32, technology: i32, source: Option<Rect>, gdi_name: String, friendly_name: String, edid: Option<(u16, u16)> }`; `windows::Windows` (unit struct).

- [ ] **Step 1: Failing tests in `ccd.rs`**:
- `pnp_id_unswaps_the_bytes`: `pnp_id(0xAC10) == "DEL"`, `pnp_id(0xE509) == "BOE"` (BOE is 0x09E5 big-endian).
- `to_state`: internal path (technology `0x80000000u32 as i32`, source `(0,0,2880,1800)`, friendly "") and external (technology 5 HDMI, source `(-2560,-200,2560,1440)`, friendly "DELL U2723QE", edid `(0xAC10, 0x41B5)`) give: built-in `Built-in display`, primary (at origin), key `boe...` absent (no edid: key `connector:display1`); external name `DELL U2723QE`, key `del:0x41b5:`, rect as given, not primary.
- `cloned_sources_list_the_second_as_off`: two paths with the same adapter and source id.
- `screen_id_is_stable`: `screen_id(&path) == "00000000000a1b2c-4357"` style: `format!("{:08x}{:08x}-{}", adapter_high as u32, adapter_low, target_id)`.

- [ ] **Step 2: Implement `ccd.rs`**

```rust
//! Windows' Connecting and Configuring Displays (CCD) data as plain values.
//! Desktop coordinates are physical pixels, and Windows defines the primary
//! display as the one at (0, 0).

use crate::model::{Identity, Rect, Screen, State};

/// LVDS, DisplayPort embedded, UDI embedded and "internal".
pub const BUILTIN_TECHNOLOGIES: [i32; 4] = [6, 11, 13, 0x8000_0000u32 as i32];

#[derive(Debug, Clone, PartialEq)]
pub struct CcdPath {
    pub adapter_low: u32,
    pub adapter_high: i32,
    pub source_id: u32,
    pub target_id: u32,
    pub technology: i32,
    pub source: Option<Rect>,
    pub gdi_name: String,
    pub friendly_name: String,
    /// (edidManufactureId as Windows stores it, edidProductCodeId)
    pub edid: Option<(u16, u16)>,
}

/// EDID keeps the PnP id big-endian; Windows hands it over byte-swapped.
pub fn pnp_id(raw: u16) -> String {
    let id = raw.swap_bytes();
    [10u16, 5, 0].iter().map(|s| (((id >> s) & 0x1f) as u8 + b'A' - 1) as char).collect()
}

pub fn screen_id(p: &CcdPath) -> String {
    format!("{:08x}{:08x}-{}", p.adapter_high as u32, p.adapter_low, p.target_id)
}

pub fn to_state(paths: &[CcdPath]) -> State {
    let mut seen_sources: Vec<(u32, i32, u32)> = Vec::new();
    let screens = paths
        .iter()
        .map(|p| {
            let source_key = (p.adapter_low, p.adapter_high, p.source_id);
            let first = !seen_sources.contains(&source_key);
            seen_sources.push(source_key);
            let builtin = BUILTIN_TECHNOLOGIES.contains(&p.technology);
            let connector = p.gdi_name.trim_start_matches(r"\\.\").to_string();
            let rect = p.source.filter(|_| first).unwrap_or_default();
            let enabled = first && p.source.is_some();
            let (vendor, product) = p.edid.map_or((String::new(), String::new()), |(m, prod)| (pnp_id(m), format!("0x{prod:04x}")));
            Screen {
                id: screen_id(p),
                connector,
                name: if !p.friendly_name.trim().is_empty() {
                    p.friendly_name.trim().to_string()
                } else if builtin {
                    "Built-in display".into()
                } else {
                    format!("Display {}", p.target_id)
                },
                identity: Identity { vendor, product, serial: String::new() },
                builtin,
                enabled,
                primary: enabled && rect.x == 0 && rect.y == 0,
                rect,
                scale: 1.0,
            }
        })
        .collect();
    State { backend: "windows".into(), screens }
}
```

(Fix the `to_state` test expectation: the internal path has no EDID, so its key is `connector:display1`; give its `gdi_name` as `\\.\DISPLAY1`.)

- [ ] **Step 3: Implement `windows.rs`**

```rust
//! Windows 10 and 11 through QueryDisplayConfig and SetDisplayConfig.

use std::mem::size_of;

use windows::Win32::Devices::Display::{
    DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig, SetDisplayConfig,
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME, DISPLAYCONFIG_MODE_INFO,
    DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SOURCE_DEVICE_NAME,
    DISPLAYCONFIG_TARGET_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS, SDC_ALLOW_CHANGES, SDC_APPLY, SDC_SAVE_TO_DATABASE,
    SDC_USE_SUPPLIED_DISPLAY_CONFIG, SDC_VALIDATE,
};
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS};

use super::ccd::{screen_id, to_state, CcdPath};
use super::{ApplyMode, Backend, Capabilities};
use crate::layout::Origin;
use crate::model::{Layout, Rect, State};
use crate::Error;

pub struct Windows;

fn wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn query_raw() -> Result<(Vec<DISPLAYCONFIG_PATH_INFO>, Vec<DISPLAYCONFIG_MODE_INFO>), Error> {
    loop {
        let (mut np, mut nm) = (0u32, 0u32);
        let rc = unsafe { GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut np, &mut nm) };
        if rc != ERROR_SUCCESS {
            return Err(Error::System(format!("GetDisplayConfigBufferSizes failed ({})", rc.0)));
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); np as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); nm as usize];
        let rc = unsafe { QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &mut np, paths.as_mut_ptr(), &mut nm, modes.as_mut_ptr(), None) };
        if rc == ERROR_INSUFFICIENT_BUFFER {
            continue; // a screen arrived between the two calls
        }
        if rc != ERROR_SUCCESS {
            return Err(Error::System(format!("QueryDisplayConfig failed ({})", rc.0)));
        }
        paths.truncate(np as usize);
        modes.truncate(nm as usize);
        return Ok((paths, modes));
    }
}

fn source_mode_index(path: &DISPLAYCONFIG_PATH_INFO, modes: &[DISPLAYCONFIG_MODE_INFO]) -> Option<usize> {
    let idx = unsafe { path.sourceInfo.Anonymous.modeInfoIdx } as usize;
    (idx < modes.len() && modes[idx].infoType == DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE).then_some(idx)
}

fn describe(path: &DISPLAYCONFIG_PATH_INFO, modes: &[DISPLAYCONFIG_MODE_INFO]) -> CcdPath {
    let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
    target.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
    target.header.size = size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
    target.header.adapterId = path.targetInfo.adapterId;
    target.header.id = path.targetInfo.id;
    let target_ok = unsafe { DisplayConfigGetDeviceInfo(&mut target.header) } == 0;

    let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
    source.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
    source.header.size = size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
    source.header.adapterId = path.sourceInfo.adapterId;
    source.header.id = path.sourceInfo.id;
    let source_ok = unsafe { DisplayConfigGetDeviceInfo(&mut source.header) } == 0;

    let rect = source_mode_index(path, modes).map(|i| {
        let m = unsafe { modes[i].Anonymous.sourceMode };
        Rect::new(m.position.x, m.position.y, m.width as i32, m.height as i32)
    });
    let edid_valid = target_ok && unsafe { target.flags.Anonymous.value } & 0x4 != 0;
    CcdPath {
        adapter_low: path.targetInfo.adapterId.LowPart,
        adapter_high: path.targetInfo.adapterId.HighPart,
        source_id: path.sourceInfo.id,
        target_id: path.targetInfo.id,
        technology: path.targetInfo.outputTechnology.0,
        source: rect,
        gdi_name: if source_ok { wide(&source.viewGdiDeviceName) } else { String::new() },
        friendly_name: if target_ok { wide(&target.monitorFriendlyDeviceName) } else { String::new() },
        edid: edid_valid.then_some((target.edidManufactureId, target.edidProductCodeId)),
    }
}

impl Backend for Windows {
    fn name(&self) -> &'static str {
        "windows"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities { primary: true, temporary: true, verify: true, remembers: true, origin: Origin::Primary }
    }

    fn query(&self) -> Result<State, Error> {
        let (paths, modes) = query_raw()?;
        Ok(to_state(&paths.iter().map(|p| describe(p, &modes)).collect::<Vec<_>>()))
    }

    fn apply(&self, layout: &Layout, mode: ApplyMode) -> Result<(), Error> {
        let (paths, mut modes) = query_raw()?;
        let described: Vec<CcdPath> = paths.iter().map(|p| describe(p, &modes)).collect();
        let state = to_state(&described);
        let mut on: Vec<&str> = state.enabled().map(|s| s.id.as_str()).collect();
        let mut named: Vec<&str> = layout.positions.iter().map(|p| p.id.as_str()).collect();
        on.sort_unstable();
        named.sort_unstable();
        if on != named {
            return Err(Error::Changed);
        }
        for (path, info) in paths.iter().zip(&described) {
            let (Some(i), Some(p)) = (source_mode_index(path, &modes), layout.position(&screen_id(info))) else { continue };
            unsafe {
                modes[i].Anonymous.sourceMode.position.x = p.x;
                modes[i].Anonymous.sourceMode.position.y = p.y;
            }
        }
        let flags = match mode {
            ApplyMode::Verify => SDC_VALIDATE | SDC_USE_SUPPLIED_DISPLAY_CONFIG,
            ApplyMode::Temporary => SDC_APPLY | SDC_USE_SUPPLIED_DISPLAY_CONFIG | SDC_ALLOW_CHANGES,
            ApplyMode::Persistent => SDC_APPLY | SDC_USE_SUPPLIED_DISPLAY_CONFIG | SDC_ALLOW_CHANGES | SDC_SAVE_TO_DATABASE,
        };
        let rc = unsafe { SetDisplayConfig(Some(&paths), Some(&modes), flags) };
        if rc != 0 {
            return Err(Error::System(format!("Windows refused the arrangement (error {rc}).")));
        }
        Ok(())
    }
}
```

Field and constant names follow `windows` 0.61; if `cargo check --target x86_64-pc-windows-gnu` reports a different name (for example `r#type` or a union path), fix it against `~/.cargo/registry/src/*/windows-0.61.3/src/Windows/Win32/Devices/Display/mod.rs`.

- [ ] **Step 4: Run the pure tests and the cross-check**

Run: `cargo test -p screen-side-core ccd && cargo check -p screen-side-core -p screen-side --target x86_64-pc-windows-gnu`
Expected: tests pass; check compiles with no errors (linking is not involved).

- [ ] **Step 5: Commit** `feat(core): add the Windows backend through the CCD API`.

### Task 14: macOS backend

**Files:**
- Create: `crates/core/src/backend/quartz.rs` (pure), `crates/core/src/backend/macos.rs` (`#[cfg(target_os = "macos")]`)
- Modify: `crates/core/Cargo.toml`, `backend/mod.rs`, `backend/detect.rs`

Cargo:

```toml
[target.'cfg(target_os = "macos")'.dependencies]
core-graphics = "0.25"
objc2 = "0.6"
objc2-app-kit = { version = "0.3", default-features = false, features = ["std", "NSScreen"] }
objc2-foundation = { version = "0.3", default-features = false, features = ["std", "NSString", "NSDictionary", "NSValue", "NSArray"] }
```

**Interfaces:**
- Produces: `quartz::{QuartzDisplay, to_state}`; `QuartzDisplay { id: u32, x: f64, y: f64, width: f64, height: f64, builtin: bool, main: bool, vendor: u32, model: u32, serial: u32, pixels_wide: u64, mirror_of: u32, name: Option<String> }`; `macos::Macos`.

- [ ] **Step 1: Failing tests in `quartz.rs`**: a built-in main display at `(0,0,1512,982)` with 3024 pixels wide (scale 2) and an external at `(-2560,-200,2560,1440)` vendor `0x10ac`, model `0x41b5`, serial 0 and name `DELL U2723QE` give: built-in primary scale 2, name `Built-in display` when `name` is None; external key `0x10ac:0x41b5:` (Quartz reports numbers, kept as hex), not primary; a display with `mirror_of != 0` is off; rects round half away from zero.

- [ ] **Step 2: Implement `quartz.rs`**

```rust
//! Quartz Display Services values as plain data. Global display space is
//! in points with the main display's top-left corner at (0, 0) and y going
//! down; the main display is by definition the one at the origin.

use crate::model::{Identity, Rect, Screen, State};

#[derive(Debug, Clone, PartialEq)]
pub struct QuartzDisplay {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub builtin: bool,
    pub main: bool,
    pub vendor: u32,
    pub model: u32,
    pub serial: u32,
    pub pixels_wide: u64,
    pub mirror_of: u32,
    pub name: Option<String>,
}

pub fn to_state(displays: &[QuartzDisplay]) -> State {
    let screens = displays
        .iter()
        .map(|d| {
            let enabled = d.mirror_of == 0;
            Screen {
                id: d.id.to_string(),
                connector: d.id.to_string(),
                name: d.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| {
                    if d.builtin { "Built-in display".into() } else { format!("Display {}", d.id) }
                }),
                identity: Identity {
                    vendor: format!("0x{:04x}", d.vendor),
                    product: format!("0x{:04x}", d.model),
                    serial: if d.serial == 0 { String::new() } else { d.serial.to_string() },
                },
                builtin: d.builtin,
                enabled,
                primary: enabled && d.main,
                rect: if enabled {
                    Rect::new(d.x.round() as i32, d.y.round() as i32, d.width.round() as i32, d.height.round() as i32)
                } else {
                    Rect::default()
                },
                scale: if d.width > 0.0 { d.pixels_wide as f64 / d.width } else { 1.0 },
            }
        })
        .collect();
    State { backend: "macos".into(), screens }
}
```

- [ ] **Step 3: Implement `macos.rs`**

```rust
//! macOS through Quartz Display Services. Names come from NSScreen, which
//! may only be asked on the main thread; elsewhere a generic name is used.

use core_graphics::display::{CGConfigureOption, CGDisplay};
use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_foundation::{NSNumber, NSString};

use super::quartz::{to_state, QuartzDisplay};
use super::{ApplyMode, Backend, Capabilities};
use crate::layout::Origin;
use crate::model::{Layout, State};
use crate::Error;

pub struct Macos;

fn names() -> Vec<(u32, String)> {
    let Some(mtm) = MainThreadMarker::new() else { return Vec::new() };
    let key = NSString::from_str("NSScreenNumber");
    NSScreen::screens(mtm)
        .iter()
        .filter_map(|screen| {
            let description = screen.deviceDescription();
            let number = description.objectForKey(&key)?.downcast::<NSNumber>().ok()?;
            Some((number.unsignedIntValue(), screen.localizedName().to_string()))
        })
        .collect()
}

fn displays() -> Result<Vec<QuartzDisplay>, Error> {
    let ids = CGDisplay::active_displays().map_err(|e| Error::System(format!("Quartz could not list displays ({e}).")))?;
    let names = names();
    Ok(ids
        .into_iter()
        .map(|id| {
            let d = CGDisplay::new(id);
            let b = d.bounds();
            QuartzDisplay {
                id,
                x: b.origin.x,
                y: b.origin.y,
                width: b.size.width,
                height: b.size.height,
                builtin: d.is_builtin(),
                main: d.is_main(),
                vendor: d.vendor_number(),
                model: d.model_number(),
                serial: d.serial_number(),
                pixels_wide: d.pixels_wide(),
                mirror_of: d.mirrors_display(),
                name: names.iter().find(|(n, _)| *n == id).map(|(_, name)| name.clone()),
            }
        })
        .collect())
}

impl Backend for Macos {
    fn name(&self) -> &'static str {
        "macos"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities { primary: true, temporary: true, verify: false, remembers: true, origin: Origin::Primary }
    }

    fn query(&self) -> Result<State, Error> {
        Ok(to_state(&displays()?))
    }

    fn apply(&self, layout: &Layout, mode: ApplyMode) -> Result<(), Error> {
        let state = to_state(&displays()?);
        let mut on: Vec<&str> = state.enabled().map(|s| s.id.as_str()).collect();
        let mut named: Vec<&str> = layout.positions.iter().map(|p| p.id.as_str()).collect();
        on.sort_unstable();
        named.sort_unstable();
        if on != named {
            return Err(Error::Changed);
        }
        let main = CGDisplay::main();
        let config = main.begin_configuration().map_err(|e| Error::System(format!("Quartz error {e}.")))?;
        for p in &layout.positions {
            let id: u32 = p.id.parse().map_err(|_| Error::Changed)?;
            if let Err(e) = CGDisplay::new(id).configure_display_origin(&config, p.x, p.y) {
                let _ = main.cancel_configuration(&config);
                return Err(Error::System(format!("macOS refused the position of display {id} ({e}).")));
            }
        }
        let option = match mode {
            ApplyMode::Persistent => CGConfigureOption::ConfigurePermanently,
            _ => CGConfigureOption::ConfigureForSession,
        };
        main.complete_configuration(&config, option)
            .map_err(|e| Error::System(format!("macOS refused the arrangement ({e}).")))
    }
}
```

`objc2` method names (`deviceDescription`, `objectForKey`, `downcast`, `unsignedIntValue`, `localizedName`) follow objc2 0.6 and objc2-app-kit 0.3; confirm with `cargo check --target aarch64-apple-darwin` and adjust to the generated names if they differ.

- [ ] **Step 4: Run** `cargo test -p screen-side-core quartz && cargo check -p screen-side-core -p screen-side --target aarch64-apple-darwin`. Expected: pass and compile.
- [ ] **Step 5: Commit** `feat(core): add the macOS backend through Quartz Display Services`.

### Task 15: Shortcut setup

**Files:**
- Create: `crates/core/src/shortcut.rs`
- Modify: `lib.rs`, `crates/cli/src/main.rs`, `crates/cli/tests/cli.rs`

**Interfaces:**
- Produces: `shortcut::{Desktop, Plan, desktop, plan_install, plan_remove, install, remove, show, quote_for_gnome, DEFAULT_GNOME_KEYS, GNOME_PATH}`:
  - `enum Desktop { Gnome, Kde, Sway, Hyprland, Other, Windows, Macos }`, `pub fn desktop(probe: &dyn Probe) -> Desktop` (uses the backend `Probe`: Windows and macOS by OS, Gnome when the Mutter bus is owned or `XDG_CURRENT_DESKTOP` contains `GNOME`, Kde by `XDG_CURRENT_DESKTOP`, Sway by `SWAYSOCK`, Hyprland by `HYPRLAND_INSTANCE_SIGNATURE`).
  - `enum Plan { Run(Vec<Vec<String>>), Instructions(String), AppManaged(String) }`.
  - `pub const GNOME_PATH: &str = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/screen-side/";`, `pub const DEFAULT_GNOME_KEYS: &str = "<Super><Alt>s";`.
  - `pub fn plan_install(desktop: Desktop, command: &[String], keys: &str, existing: &str) -> Plan` where `existing` is the output of `gsettings get org.gnome.settings-daemon.plugins.media-keys custom-keybindings`.
  - `pub fn plan_remove(desktop: Desktop, existing: &str) -> Plan`.
  - `pub fn install(runner: &dyn Runner, desktop: Desktop, command: &[String], keys: &str) -> Result<String, Error>`, `remove(...)`, `show(runner, desktop) -> Result<String, Error>`.
  - `pub fn shell_quote(arg: &str) -> String` (POSIX single quotes), `pub fn gvariant_string(s: &str) -> String`.

- [ ] **Step 1: Failing tests**:
- `gnome_install_appends_to_the_list`: existing `@as []` gives a `Run` of four `gsettings` calls: set `custom-keybindings` to `['/org/.../screen-side/']`, then `name` `'Screen Side toggle'`, `command`, `binding` `'<Super><Alt>s'`, using schema `org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:` + path.
- `gnome_install_keeps_existing_entries_once`: existing `['/org/x/custom0/', '/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/screen-side/']` keeps custom0 and does not duplicate ours.
- `gnome_command_quoting`: command `["/home/a b/Screen Side.AppImage", "--toggle"]` sets command to the GVariant string of `'/home/a b/Screen Side.AppImage' '--toggle'`; a path containing `'` is quoted as `'it'\''s'`; `gvariant_string("a'b\\c")` is `'a\'b\\\\c'`.
- `gnome_remove`: drops our path from the list and resets the relocatable schema with `gsettings reset-recursively`.
- `sway_and_hyprland_print_lines`: Sway `Instructions` contains `bindsym Mod4+Mod1+s exec '/usr/bin/screen-side' 'toggle'`; Hyprland contains `bind = SUPER ALT, S, exec, '/usr/bin/screen-side' 'toggle'`; KDE contains `System Settings` and `Custom Shortcuts` and the command.
- `windows_macos_and_other_are_app_managed`.
- `install_runs_the_plan` with `Scripted` (first reply to `gsettings get` is `@as []`, then four empty replies) and returns `Shortcut set: Super+Alt+S runs ...`.

- [ ] **Step 2: Implement** `shortcut.rs` with those functions. List parsing: strip a leading `@as `, then take every single-quoted item between `[` and `]`; format back as `['a', 'b']`. `gvariant_string` wraps in single quotes and escapes `\` and `'` with a backslash. `shell_quote` wraps in single quotes and replaces `'` with `'\''`. The command string is the arguments shell-quoted and joined with spaces (GNOME parses it with `g_shell_parse_argv`). Human key text for messages turns `<Super><Alt>s` into `Super+Alt+S`.

- [ ] **Step 3: Add the CLI command**

```rust
    /// Set up a keyboard shortcut that runs `screen-side toggle`.
    Shortcut {
        #[command(subcommand)]
        action: ShortcutAction,
    },
```

with `enum ShortcutAction { Install { #[arg(long, default_value = shortcut::DEFAULT_GNOME_KEYS)] keys: String }, Remove, Show }`. The command array is `[std::env::current_exe() canonicalised as a string, "toggle"]`. CLI tests (no GNOME on CI) run with `XDG_CURRENT_DESKTOP=sway` and `SWAYSOCK=/tmp/x` set and `SCREEN_SIDE_BACKEND=fake`: `shortcut install` exits 0 and prints the `bindsym` line containing the test binary's path; with `XDG_CURRENT_DESKTOP` unset and no sockets on Linux, it prints the app-managed message.

- [ ] **Step 4: Run** core and CLI tests. **Step 5: Commit** `feat: set up the toggle keyboard shortcut per desktop`.

### Task 16: Doctor

**Files:**
- Create: `crates/core/src/doctor.rs`
- Modify: `lib.rs`, `crates/cli/src/main.rs`, `crates/cli/tests/cli.rs`

**Interfaces:**
- Produces: `doctor::{Report, report, redact}`: `pub fn report(probe: &dyn Probe, version: &str) -> Report` (never fails; failures go into `Report.error`), `impl Display for Report`, `impl Serialize for Report`; `pub fn redact(state: &State) -> serde_json::Value` (screens with `serial` replaced by `"present"` or `"absent"`).
- `Report { version, os, arch, desktop, session, backend: Option<String>, reason: String, available: Vec<(String, bool)>, capabilities: Option<Capabilities>, details: Vec<(String, String)>, tools: Vec<(String, String)>, config_dir: String, layouts: Option<usize>, state: Option<serde_json::Value>, error: Option<String> }`.

- [ ] **Step 1: Failing tests**: with a probe choosing `fake`, `report` has backend `fake`, the sample's two screens, `layouts` count from a tempdir store, and `to_string()` contains `Screen Side 2.0.0`, `Backend: fake`, `Capabilities:` and `serial present` for HDMI-1 and never `FAKE0001`; a probe that finds nothing gives `backend None` and `error` with the `NoBackend` message, and the text says `No backend:`; JSON output (CLI `doctor --json`) has `schema 1` and contains no `FAKE0001`.
- `tools` lists `kscreen-doctor`, `wlr-randr`, `xrandr`, `gsettings` with `found` or `not found` on Linux only (by probe OS).

- [ ] **Step 2: Implement**, using `std::env::consts::{OS, ARCH}`, `XDG_CURRENT_DESKTOP` and `XDG_SESSION_TYPE` from the probe, `choose`, `create`, `backend.query()`, `backend.diagnostics()`, `Store::open()` (layouts count, or the error text).

- [ ] **Step 3: CLI `doctor [--json]`**, which never exits non-zero unless writing to stdout fails, so people can always paste it.

- [ ] **Step 4: Run** all Rust tests and clippy, and run `cargo run -q -p screen-side -- doctor` on this machine. Expected: backend `gnome`, the real monitors, serials shown as present or absent.

- [ ] **Step 5: Commit** `feat: add screen-side doctor for bug reports`.

---

## Phase 3: the app

### Task 17: Interface foundation (workspace, tokens, seam, mock)

Invoke `frontend-design:frontend-design` first. Direction to give it: a small native utility, not a web page. Calm and precise, one accent. Brand from the 1.0 icon: deep blue `#1c62a8` to `#4a90d9` and the pointer yellow `#ffd45e` (used sparingly, for the pointer and the selected edge only). System UI font stack. Light and dark.

**Files:**
- Create: `package.json` (root, npm workspaces `app` and `site`), `tsconfig.base.json`, `app/package.json`, `app/tsconfig.json`, `app/vite.config.ts`, `app/vitest.setup.ts`, `app/index.html`, `app/public/favicon.svg`, `app/src/main.tsx`, `app/src/tokens.css`, `app/src/base.css`, `app/src/backend/types.ts`, `app/src/backend/mock.ts`, `app/src/backend/mock.test.ts`, `app/src/backend/resolve.ts`, `app/src/tokens.test.ts`, `vitest.config.ts`

**Interfaces:**
- Produces the TypeScript seam (`app/src/backend/types.ts`), mirrored by the Rust `AppState` in Task 19:

```ts
export type Side = 'left' | 'right' | 'above' | 'below';
export type Align = 'start' | 'center' | 'end';
export type Platform = 'linux' | 'windows' | 'macos';

export interface ScreenInfo {
  number: number;
  id: string;
  connector: string;
  name: string;
  builtin: boolean;
  enabled: boolean;
  primary: boolean;
  x: number;
  y: number;
  width: number;
  height: number;
  scale: number;
}

export interface Capabilities {
  primary: boolean;
  temporary: boolean;
  verify: boolean;
  remembers: boolean;
  origin: 'top_left' | 'primary';
}

export interface Arrangement {
  anchor: string;
  placements: { screen: string; side: Side }[];
  align: Align;
  aligned: boolean;
  primary: string;
}

export interface LayoutInfo {
  name: string;
  auto: boolean;
  matches: boolean;
  summary: string;
}

export interface Settings {
  background: boolean;
  autoStart: boolean;
  autoApply: boolean;
  shortcut: string | null;
}

export type ShortcutSupport =
  | { kind: 'app' }
  | { kind: 'gnome'; installed: boolean; keys: string }
  | { kind: 'manual'; instructions: string };

export interface AppState {
  version: string;
  platform: Platform;
  backend: string | null;
  capabilities: Capabilities | null;
  screens: ScreenInfo[];
  arrangement: Arrangement | null;
  layouts: LayoutInfo[];
  activeLayout: string | null;
  settings: Settings;
  shortcut: ShortcutSupport;
  cliPath: string | null;
  error: string | null;
}

export interface Backend {
  getState(): Promise<AppState>;
  /** `screen` null moves every external screen. */
  moveScreen(screen: string | null, side: Side): Promise<AppState>;
  toggle(): Promise<AppState>;
  setAlign(align: Align): Promise<AppState>;
  setPrimary(screen: string): Promise<AppState>;
  saveLayout(name: string, auto: boolean): Promise<AppState>;
  applyLayout(name: string): Promise<AppState>;
  forgetLayout(name: string): Promise<AppState>;
  setLayoutAuto(name: string, auto: boolean): Promise<AppState>;
  updateSettings(patch: Partial<Settings>): Promise<AppState>;
  installShortcut(): Promise<AppState>;
  removeShortcut(): Promise<AppState>;
  diagnostics(): Promise<string>;
  openUrl(url: string): Promise<void>;
  copyText(text: string): Promise<void>;
  /** Called whenever the screens change outside the window. Returns an unsubscribe. */
  onChange(listener: () => void): () => void;
}
```

Every mutating call resolves with the new state or rejects with an `Error` whose message is the user-facing text from Rust.

- [ ] **Step 1: Root and app packages**

`package.json`:

```json
{
  "name": "screen-side",
  "private": true,
  "type": "module",
  "workspaces": ["app", "site"],
  "scripts": {
    "dev": "npm run dev -w app",
    "build": "npm run build -w app",
    "test": "vitest run",
    "test:e2e": "npm run e2e -w app && npm run e2e -w site",
    "typecheck": "tsc -p app --noEmit",
    "tauri": "npm run tauri -w app --"
  },
  "devDependencies": {
    "@types/node": "^22.0.0",
    "typescript": "^5.9.0",
    "vitest": "^5.0.0"
  }
}
```

`app/package.json` (version `2.0.0`): dependencies `@tauri-apps/api ^2`, `@tauri-apps/plugin-opener ^2`, `@tauri-apps/plugin-clipboard-manager ^2`, `react ^19`, `react-dom ^19`; devDependencies `@playwright/test ^1.50.0`, `@tauri-apps/cli ^2`, `@testing-library/dom ^10`, `@testing-library/jest-dom ^6`, `@testing-library/react ^16`, `@testing-library/user-event ^14`, `@types/react ^19`, `@types/react-dom ^19`, `@vitejs/plugin-react ^6`, `jsdom ^26`, `vite ^8`; scripts `dev: vite`, `build: tsc --noEmit && vite build`, `preview: vite preview`, `tauri: tauri`, `e2e: playwright test`.

`vitest.config.ts` at the root runs `app/src/**/*.test.{ts,tsx}` in `jsdom` with `app/vitest.setup.ts` (imports `@testing-library/jest-dom/vitest`).

`app/vite.config.ts`: React plugin, `server.port 1420` strict, `clearScreen: false`, `build.target: 'es2021'`, `envPrefix: ['VITE_', 'TAURI_']`.

`app/src/main.tsx` picks the backend: `'__TAURI_INTERNALS__' in window` loads `./backend/tauri` (Task 19) with a dynamic import; otherwise `resolveBackend()` from `./backend/resolve`.

Run `npm install` and commit `package-lock.json`.

- [ ] **Step 2: Tokens.** `app/src/tokens.css` declares the full light palette on bare `:root`, then the dark palette twice with identical declarations: once in `@media (prefers-color-scheme: dark) { :root:not([data-theme='light']) { ... } }` and once in `:root[data-theme='dark'] { ... }`. Tokens: `--bg, --surface, --surface-2, --border, --border-strong, --text, --muted, --accent, --accent-hover, --accent-tint, --on-accent, --pointer, --danger, --danger-tint, --focus, --radius-s, --radius-m, --radius-l, --space-1..6, --font-ui, --font-mono, --text-s, --text-m, --text-l, --dur-fast, --dur-med, --ease`. `--font-ui` is `system-ui, -apple-system, 'Segoe UI', Cantarell, Ubuntu, 'Noto Sans', sans-serif`.

`app/src/tokens.test.ts` reads the file with `fs`, extracts both dark blocks with a regex, and asserts they declare the same properties with the same values, and that every `--*` used in either dark block is also declared on bare `:root`.

- [ ] **Step 3: Failing mock tests (`mock.test.ts`)**:
- `createMock({ screens: 2 })` returns a laptop `eDP-1` (1280x800, built-in, primary) and `HDMI-1` (2560x1440) with the external left; `arrangement.placements` is `[{screen: 'HDMI-1', side: 'left'}]`.
- `moveScreen(null, 'right')` puts HDMI-1 at x 1280 and the laptop at x 0 (centred: laptop y 320).
- `setAlign('end')` after that puts both bottoms level.
- `createMock({ screens: 3 })` adds `DP-1` (1920x1080) above; `moveScreen('DP-1', 'left')` gives two left placements with DP-1 farther out.
- `moveScreen('DP-1', 'above')` with `HDMI-1` 2560x1440 on the left and centre alignment on a 3-screen mock where both would overlap rejects with `would overlap`.
- `toggle()` mirrors sides.
- `setPrimary('HDMI-1')` flips primary; with `createMock({ backend: 'wlroots' })` `capabilities.primary` is false and `setPrimary` rejects.
- `saveLayout('office', true)` adds a matching auto layout, `activeLayout` is `office`; `moveScreen(null, 'right')` makes `activeLayout` null; `applyLayout('office')` restores; `forgetLayout('office')` removes it.
- `updateSettings({ background: false })` persists in the mock.
- `onChange` listeners run after `simulateHotplug()` (a mock-only method that adds or removes `DP-1`).
- `createMock({ error: 'No backend' })` returns a state with `error` set and no screens.

- [ ] **Step 4: Implement `mock.ts`.** Keep a private `Screen[]`, an `Arrangement`, layouts and settings. `compute(arr)` is a TypeScript copy of the Rust chain maths limited to what the mock needs (sides, chains, the three alignments, overlap check, top-left normalisation), with a comment saying the Rust version in `crates/core/src/layout.rs` is the real one and this exists only so the interface runs in a browser. `resolve.ts` reads `?screens=1|2|3`, `?backend=gnome|wlroots|windows|macos|kde`, `?error=...` and `?theme=light|dark` (sets `document.documentElement.dataset.theme`).

- [ ] **Step 5: Run** `npm test` and `npm run typecheck`. Expected: pass.
- [ ] **Step 6: Commit** `feat(app): add the interface workspace, design tokens and a mock backend`.

### Task 18: Interface components

**Files:**
- Create: `app/src/App.tsx`, `app/src/App.css`, `app/src/App.test.tsx`, `app/src/components/{Preview,SidePicker,AlignPicker,ScreenChips,Banner,Layouts,Settings,Tabs}.tsx` with a `.css` and a `.test.tsx` beside each, `app/src/icons.tsx`, `app/src/text.ts`, `app/src/text.test.ts`

**Interfaces:**
- Consumes: `Backend`, `AppState` from Task 17.
- Produces: `<App backend={Backend} />`; `text.ts` exports `crossing(side: Side, screenName: string, anchorName: string): string`, `alignLabel(align: Align, horizontal: boolean): string` (Top/Centred/Bottom or Left/Centred/Right), `headline(state: AppState): string` (the same sentences as the CLI status headline).

Behaviour (each line is a test):
- **App**: loads state on mount and shows a loading line until then; shows three tabs (Arrange, Layouts, Settings) with roving keyboard focus (`role="tablist"`); a rejected call shows the `Banner` with the message and reloads state; `onChange` reloads state; with `state.error` and no screens it shows the banner, a "Copy diagnostics" button and no controls.
- **Preview**: draws every enabled screen as an SVG `<g role="button">` scaled into the viewBox, labelled with its name and `Primary` for the primary one; switched-off screens are listed below the drawing as text (`eDP-1 is off`); clicking a screen calls `onSelect(id)`; the selected one has `aria-pressed="true"`; for the selected external screen, a pointer arrow is drawn across the shared edge with the anchor (respecting `prefers-reduced-motion` for its small animation); keyboard: Enter and Space select.
- **ScreenChips**: with two or more externals, chips `All screens`, then each external by name; with one external, no chips (that screen is the selection).
- **SidePicker**: four buttons (Left, Right, Above, Below) with arrow icons; the pressed one is the selected screen's side, or the common side for All; clicking calls `moveScreen(selected or null, side)`; disabled while a call is in flight.
- **AlignPicker**: three segments labelled by axis (Top, Centred, Bottom for a left or right screen; Left, Centred, Right for above or below); `Custom` hint when `aligned` is false.
- **Primary**: a "Make primary" button for the selected screen when `capabilities.primary` and it is not primary; hidden otherwise.
- A line under the controls: `crossing(...)` for the selected screen.
- **Layouts**: matching layouts first, each with Apply, an Auto switch (`role="switch"`) and Delete (asks inline "Delete office?" with Delete and Cancel, no browser dialog); non-matching ones greyed with "For other screens" and no Apply; a form "Save current layout" with a name input (required, max 64), an "Apply automatically when these screens connect" checkbox, and Save; saving an existing name says "Replaced office".
- **Settings**: switches for "Keep running in the background" (shows "Uses the tray icon" and, on Linux, "Needs a tray: on GNOME, the AppIndicator extension"), "Start at login", "Apply saved layouts automatically"; Keyboard shortcut section by `shortcut.kind`: `app` shows a key recorder (focus the field, press keys, it shows `Super+Alt+S`, Save and Clear buttons) calling `updateSettings({ shortcut })`; `gnome` shows the keys and Set up or Remove; `manual` shows the instructions in a `<pre>` with a Copy button; then "Command line" with `cliPath` or "Not installed" and a link to the install instructions; "Copy diagnostics"; version and links (Website, Report a problem, Ideas and questions).

- [ ] **Step 1: Write the tests listed above** against `createMock` (Testing Library and user-event; no snapshot tests).
- [ ] **Step 2: Run to see them fail** (`npm test`).
- [ ] **Step 3: Implement the components** following the frontend-design direction from Task 17. All colours, sizes and durations come from `tokens.css`.
- [ ] **Step 4: Run** `npm test && npm run typecheck && npm run build`. Expected: pass and build.
- [ ] **Step 5: Look at it.** Run `npm run dev -w app` and open `http://localhost:1420/?screens=3` and `?screens=2&theme=dark` with Playwright (`browser_navigate`, `browser_take_screenshot`); check spacing, contrast and focus rings; fix what looks wrong.
- [ ] **Step 6: Commit** `feat(app): add the arrange, layouts and settings views`.

### Task 19: Tauri shell

**Files:**
- Create: `app/src-tauri/{Cargo.toml,build.rs,tauri.conf.json,tauri.windows.conf.json,tauri.macos.conf.json,screen-side-gui.desktop}`, `app/src-tauri/capabilities/default.json`, `app/src-tauri/icons/*` (generated), `app/src-tauri/icons/tray.png`, `app/src-tauri/icons/tray-template.png`, `app/src-tauri/src/{main.rs,lib.rs,args.rs,view.rs,commands.rs,tray.rs,watcher.rs,hotkey.rs}`, `app/src/backend/tauri.ts`
- Modify: root `Cargo.toml` (add `app/src-tauri` to `members`, not to `default-members`), `crates/core/src/shortcut.rs` (add `to_gnome_keys`)

**Interfaces:**
- Consumes: core everything; the TypeScript seam.
- Produces: binary `screen-side-gui`; commands `get_state, move_screen, toggle, set_align, set_primary, save_layout, apply_layout, forget_layout, set_layout_auto, update_settings, install_shortcut, remove_shortcut, diagnostics`; event `state-changed`; `view::app_state(...) -> AppState` (serde camelCase, matching `types.ts` field for field); `args::Action::{Show, Background, Toggle, Apply(String)}` with `args::parse(args: &[String]) -> Action`; `shortcut::to_gnome_keys("Super+Alt+S") == "<Super><Alt>s"`.

`app/src-tauri/Cargo.toml`:

```toml
[package]
name = "screen-side-gui"
description = "Screen Side app"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
publish = false

[lib]
name = "screen_side_gui_lib"
crate-type = ["lib", "cdylib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
screen-side-core = { path = "../../crates/core" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tauri = { version = "2", features = ["tray-icon", "image-png"] }
tauri-plugin-autostart = "2"
tauri-plugin-clipboard-manager = "2"
tauri-plugin-global-shortcut = "2"
tauri-plugin-opener = "2"
tauri-plugin-single-instance = "2"
```

`tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Screen Side",
  "version": "2.0.0",
  "identifier": "io.github.danieltyukov.ScreenSide",
  "mainBinaryName": "screen-side-gui",
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devUrl": "http://localhost:1420",
    "frontendDist": "../dist"
  },
  "app": {
    "withGlobalTauri": false,
    "windows": [
      {
        "label": "main",
        "title": "Screen Side",
        "width": 560,
        "height": 680,
        "minWidth": 440,
        "minHeight": 560,
        "resizable": true,
        "center": true,
        "visible": false
      }
    ],
    "security": {
      "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost"
    }
  },
  "bundle": {
    "active": true,
    "targets": ["deb", "rpm", "appimage"],
    "category": "Utility",
    "shortDescription": "Put your external monitor left, right, above or below your laptop screen",
    "longDescription": "Screen Side puts your external monitor on the side of the laptop screen where it sits on your desk, so the pointer crosses the matching edge. One click, a tray menu, a hotkey or a command, and each desk's layout comes back when you plug in.",
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"],
    "linux": {
      "deb": {
        "desktopTemplate": "screen-side-gui.desktop",
        "files": { "/usr/bin/screen-side": "../../target/release/screen-side" }
      },
      "rpm": {
        "desktopTemplate": "screen-side-gui.desktop",
        "files": { "/usr/bin/screen-side": "../../target/release/screen-side" }
      }
    },
    "macOS": { "signingIdentity": "-", "minimumSystemVersion": "12.0" }
  }
}
```

The window starts hidden; `lib.rs` shows it unless launched with `--background`. `tauri.windows.conf.json` sets targets `["nsis", "msi"]`, `webviewInstallMode: downloadBootstrapper (silent)` and NSIS `installMode: currentUser`. `tauri.macos.conf.json` sets targets `["dmg", "app"]`. `beforeBuildCommand` in the Linux release job is preceded by `cargo build --release -p screen-side` so the `files` source exists.

`capabilities/default.json` grants the `main` window `core:default`, `core:event:default`, `opener:allow-open-url` (and `opener:allow-default-urls`), `clipboard-manager:allow-write-text`.

- [ ] **Step 1: Failing Rust tests** (run with `cargo test -p screen-side-gui`, which needs the WebKitGTK toolchain installed here):
- `args::parse`: `[]` is `Show`; `["--background"]` is `Background`; `["--toggle"]` is `Toggle`; `["--apply", "office"]` is `Apply("office")`; `["--apply"]` is `Show` (nothing to apply); unknown flags are ignored (`Show`), because the OS can add its own (macOS `-psn_...`).
- `cliPath` comes from `run::which("screen-side")`.
- `view::app_state` from the fake sample with two layouts (one matching) and default settings: field names serialise as `activeLayout`, `cliPath`, `autoStart`; screens numbered as in `State::listed`; `shortcut` kind is `app` on Windows and macOS, `gnome` when the desktop is GNOME, `manual` with the Sway instructions on Sway, `app` on X11 (no `WAYLAND_DISPLAY`).
- `view::app_state` with a detection error: `error` set, `screens` empty, `backend` null.
- `shortcut::to_gnome_keys`: `Super+Alt+S` to `<Super><Alt>s`, `Ctrl+Shift+F12` to `<Primary><Shift>F12`.

- [ ] **Step 2: Implement.**
  - `main.rs`: `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`, then `screen_side_gui_lib::run()`.
  - `lib.rs` `run()`: parse args. `Toggle` and `Apply(name)` are done right here with `core` (detect, baseline or resolve, arrange, persistent), and the process exits with code 0, or 1 after logging the error to stderr; a running instance notices within one watcher interval. Otherwise build Tauri with plugins: single-instance (a second launch shows and focuses the window), autostart (`MacosLauncher::LaunchAgent`, args `["--background"]`), global-shortcut, opener, clipboard-manager. `manage(Shared { backend: Mutex<Option<Arc<dyn Backend>>>, choice, store, detect_error })`. In `setup`: create the tray when `settings.background`; start the watcher thread; register the hotkey; show the main window unless `Background`. On `CloseRequested` for `main`: if `settings.background`, `prevent_close()` and hide.
  - `commands.rs`: each command is a synchronous `#[tauri::command]` (so on macOS it runs on the main thread, where NSScreen names are available), does one core operation, saves settings where needed, and returns `view::app_state(...)` or `Err(String)` with the core error's text. `update_settings` also turns autostart on or off (`app.autolaunch()`), registers or unregisters the hotkey, and creates or removes the tray.
  - `watcher.rs`: a thread running `core::watch::run` every 2 seconds with `auto_apply` read from settings on each tick; on any event it emits `state-changed` and rebuilds the tray menu (through `app.run_on_main_thread`).
  - `tray.rs`: menu `Left`, `Right`, `Above`, `Below` (move all), separator, `Swap sides` (toggle), `Layouts` submenu (matching layouts enabled, others disabled with "(other screens)"), separator, `Open Screen Side`, `Quit`. Icon `icons/tray.png`; on macOS `icons/tray-template.png` with `icon_as_template(true)`. Left click opens the menu on Windows and Linux; on macOS the menu opens by default.
  - `hotkey.rs`: registers `settings.shortcut` with the global-shortcut plugin when the shortcut kind is `app`; the handler runs the same toggle as the tray; failure to register (taken by another app) becomes a state `error` the window shows.
  - `app/src/backend/tauri.ts`: implements `Backend` with `invoke` and `listen('state-changed', ...)`; `openUrl` uses `@tauri-apps/plugin-opener`; `copyText` uses the clipboard plugin.
  - Icons: render `app/icon-source.svg` to a 1024 PNG (`rsvg-convert -w 1024` or `inkscape`), run `npx tauri icon <png>` in `app/`; draw a monochrome 32x32 tray mark (two screens and the pointer) and its template variant.

- [ ] **Step 3: Run the shell's tests and lints**

Run: `cargo test -p screen-side-gui && cargo clippy -p screen-side-gui -- -D warnings && cargo fmt --all --check`
Expected: pass.

- [ ] **Step 4: Run the app for real on this GNOME session**

Run: `npm run tauri dev` (in `app/`), with `SCREEN_SIDE_BACKEND` unset.
Expected: the window shows this machine's monitors from Mutter. Check the Layouts and Settings tabs; check that Copy diagnostics produces the doctor text; check the tray icon appears (Ubuntu ships AppIndicator) when background is on. Do not click a side unless the maintainer has said moving the real screens is fine; use `SCREEN_SIDE_BACKEND=fake npm run tauri dev` to exercise moves, layouts and the tray.

- [ ] **Step 5: Commit** `feat(app): add the Tauri shell with tray, background watcher, autostart and hotkey`.

### Task 20: End-to-end tests and screenshots

**Files:**
- Create: `app/playwright.config.ts`, `app/e2e/app.spec.ts`, `app/e2e/screenshots.spec.ts`, `docs/img/app-light.png`, `docs/img/app-dark.png`, `docs/img/app-layouts.png`

`playwright.config.ts`: Chromium only, `webServer: { command: 'npm run build && npx vite preview --port 4173 --strictPort', url: 'http://localhost:4173', reuseExistingServer: !process.env.CI }`, `use.baseURL`, viewport 560x680, `testIgnore` the screenshots spec unless `SCREENSHOTS=1`.

- [ ] **Step 1: Write `app.spec.ts`**: with `?screens=2`, clicking Right presses Right and the crossing text says "right edge"; with `?screens=3`, choosing the DELL chip then Left moves only that screen (the preview's DELL group moves left of the laptop); saving a layout named `office` with auto on lists it with Apply; reloading keeps nothing (mock is in-memory), which is expected; with `?backend=wlroots` there is no Make primary button; with `?error=No%20backend` the banner and Copy diagnostics show; dark theme via `?theme=dark` has a dark background (`getComputedStyle(document.body).backgroundColor` is not white); keyboard: Tab reaches the tabs, the arrow keys move between tabs.
- [ ] **Step 2: Run** `npm run e2e -w app` (install Chromium first with `npx playwright install chromium`). Expected: pass.
- [ ] **Step 3: `screenshots.spec.ts`** writes `docs/img/app-light.png` (`?screens=3`, Arrange tab, DELL selected), `docs/img/app-dark.png` (same, `?theme=dark`) and `docs/img/app-layouts.png` (Layouts tab with two saved layouts) at device scale factor 2. Run `SCREENSHOTS=1 npx playwright test e2e/screenshots.spec.ts` in `app/` and look at the three images.
- [ ] **Step 4: Commit** `test(app): add end-to-end tests and generate the screenshots`.

---

## Phase 4: site, installers, CI, release and community

### Task 21: Project site

Invoke `frontend-design:frontend-design` first, with the app's tokens as the base and the direction "the product page of a precise little utility; the interactive desk diagram is the hero's centrepiece".

**Files:**
- Create: `site/package.json` (version `2.0.0`, scripts `dev`, `build`, `preview`, `e2e`), `site/vite.config.ts` (as owl-transfer's: `base: './'`, `outDir: 'dist'`, `server.fs.allow` the repo root), `site/index.html`, `site/style.css` (starts with `@import '../app/src/tokens.css';`), `site/main.js`, `site/demo.js`, `site/public/favicon.svg`, `site/public/og.png`, `site/og.svg`, `site/playwright.config.ts`, `site/e2e/site.spec.ts`

Content, in order (all copy plain and direct, no marketing tone; run it past the `humanizer` skill before committing):
1. Header: mark and "Screen Side", links GitHub, Docs (`docs/`), Releases, Discussions, theme toggle (stores `screen-side-theme`, same three-path pattern as the app).
2. Hero: `<h1>` "Your monitor is on the left. Now the pointer knows it." Lead: one paragraph saying what it does and where it runs. A primary download button whose label and link `main.js` sets from `navigator.userAgentData?.platform ?? navigator.platform` (Windows: `ScreenSide_x64-setup.exe`; macOS: `ScreenSide_universal.dmg`; Linux: `screen-side_x86_64.AppImage`, with `.deb` and `.rpm` as secondary links), all pointing at `https://github.com/danieltyukov/screen-side-switcher/releases/latest/download/<name>`; without JavaScript the button links to the releases page. Tabs Windows, macOS, Linux, Command line (radio inputs and labels as in garmin-hevy-sync, so they work without JavaScript) with copyable commands: the installers, `sudo apt install ./screen-side_amd64.deb`, `curl -LsSf https://danieltyukov.github.io/screen-side-switcher/install.sh | sh`, `powershell -ExecutionPolicy ByPass -c "irm https://danieltyukov.github.io/screen-side-switcher/install.ps1 | iex"`, `cargo install --git https://github.com/danieltyukov/screen-side-switcher screen-side`. Notes on the SmartScreen and Gatekeeper prompts.
3. Demo (`demo.js`): an inline SVG desk with a laptop and a monitor; buttons Left, Right, Above, Below and an alignment control move the monitor with a CSS transform transition; a pointer glyph travels from the laptop across the shared edge, highlighted in the pointer yellow; a caption states the crossing in words (`aria-live="polite"`). Reduced motion: no travel animation, the edge highlight stays.
4. What it does: five short items (several screens; saved layouts per desk; tray and hotkey; command line; primary and alignment), with the light and dark screenshots in a `<picture>`.
5. Works on: table with Desktop, Driven by, Tested on real hardware (Yes for GNOME; "Fixture-tested, reports welcome" for KDE, wlroots, X11, Windows and macOS until someone confirms; link to the desktop issue form).
6. Command line: a short block of the main commands.
7. Contribute: report a bug (`screen-side doctor`), ideas and questions (Discussions), add a desktop (`docs/BACKENDS.md`), help wanted (Homebrew, winget, AUR, Flathub).
8. Footer: MIT licence, version 2.0.0, links.

`<head>`: title "Screen Side", description, canonical `https://danieltyukov.github.io/screen-side-switcher/`, Open Graph and Twitter tags with `og.png` (1200x630, rendered from `og.svg` with `rsvg-convert -w 1200 -h 630`), `color-scheme`, `theme-color` light and dark, the blocking theme script.

- [ ] **Step 1: `site.spec.ts`**: the page loads with no console errors; the hero download button points at the macOS dmg when the user agent is a Mac (`test.use({ userAgent: ... })`) and at the setup exe for Windows; the Command line tab shows the curl command and Copy puts it on the clipboard (grant `clipboard-read`); clicking Above in the demo changes the caption to mention the top edge; the theme toggle sets `data-theme`; every internal link resolves (`install.sh` and `install.ps1` return 200 from the preview server).
- [ ] **Step 2: Build the page.** **Step 3: Run** `npm run build -w site && npm run e2e -w site`; look at it in light, dark and at 375px wide with Playwright screenshots; fix what looks wrong. **Step 4: Commit** `feat(site): add the project site`.

### Task 22: One-line CLI installers

**Files:**
- Create: `site/public/install.sh`, `site/public/install.ps1`, `scripts/test-install.sh`

`install.sh` (POSIX `sh`, `set -eu`), environment: `SCREEN_SIDE_INSTALL_DIR` (default `$HOME/.local/bin`), `SCREEN_SIDE_VERSION` (default `latest`), `SCREEN_SIDE_ARCHIVE` (a local archive, for tests and offline installs). Steps: detect `uname -s` (`Linux` or `Darwin`, else exit 1 with a message) and `uname -m` (`x86_64|amd64` to `x64`, `aarch64|arm64` to `arm64`; on macOS always `universal`); pick `screen-side-linux-x64.tar.gz`, `screen-side-linux-arm64.tar.gz` or `screen-side-macos-universal.tar.gz`; download it and `SHA256SUMS` with `curl -fsSL` (or `wget -qO-`) from `https://github.com/danieltyukov/screen-side-switcher/releases/<latest/download|download/vX>`; verify with `sha256sum` or `shasum -a 256`; extract to a temp dir; install `screen-side` with mode 755; remove 1.0 leftovers only when they are 1.0's (`$HOME/.local/share/screen-side-switcher`; `$HOME/.local/bin/screen-side-gui` and the old `$HOME/.local/bin/screen-side` only if they contain `screenside.`; the desktop entry `$HOME/.local/share/applications/io.github.danieltyukov.ScreenSide.desktop` only if its `Exec=` ends in `screen-side-gui` under `.local`; the icon beside it), each removal reported; print the installed version (`screen-side --version`), a PATH hint if the directory is not on `PATH`, and the app download link for this OS.

`install.ps1`: the same for Windows (`x64` or `arm64` from `$env:PROCESSOR_ARCHITECTURE`), into `$env:LOCALAPPDATA\Programs\screen-side`, `Get-FileHash` against `SHA256SUMS`, add the directory to the user `Path` with `[Environment]::SetEnvironmentVariable(..., 'User')` if missing, honour `SCREEN_SIDE_ARCHIVE` and `SCREEN_SIDE_INSTALL_DIR`.

`scripts/test-install.sh`: builds the CLI (`cargo build --release -p screen-side`), packs it as the release does into a temp dir with a `SHA256SUMS`, makes a fake `$HOME` with 1.0's files (a launcher containing `from screenside.cli import main`, the desktop entry, the lib dir), runs `install.sh` with `HOME`, `SCREEN_SIDE_ARCHIVE` and `SCREEN_SIDE_INSTALL_DIR` pointing into it, then asserts the new binary prints `2.0.0`, the 1.0 files are gone, and an unrelated file in `~/.local/bin` survives.

- [ ] **Step 1: Write `scripts/test-install.sh` first; run it; it fails** (no installer).
- [ ] **Step 2: Write `install.sh`; run** `shellcheck site/public/install.sh scripts/test-install.sh && sh scripts/test-install.sh`. Expected: pass.
- [ ] **Step 3: Write `install.ps1`.** It is exercised on the Windows CI runner (Task 23).
- [ ] **Step 4: Commit** `feat: add one-line installers for the command line tool`.

### Task 23: CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`, `scripts/pack-cli.sh`

Pin third-party actions to commit SHAs as garmin-hevy-sync does (look up current SHAs with `gh api repos/<owner>/<repo>/git/ref/tags/<tag>` when writing the file); first-party `actions/*` by major version as owl-transfer does. `on: push (main), pull_request, workflow_dispatch`; `permissions: contents: read`; concurrency cancel-in-progress per ref.

Jobs:
- `rust` (ubuntu-latest): `dtolnay/rust-toolchain@stable` with rustfmt and clippy and targets `x86_64-pc-windows-gnu, aarch64-apple-darwin`; cache; `cargo fmt --all --check`; `cargo clippy -p screen-side-core -p screen-side --all-targets -- -D warnings`; `cargo test -p screen-side-core -p screen-side`; `cargo check -p screen-side-core -p screen-side --target x86_64-pc-windows-gnu`; `cargo check -p screen-side-core -p screen-side --target aarch64-apple-darwin`; `shellcheck site/public/install.sh scripts/*.sh`; `sh scripts/test-install.sh`.
- `native` (matrix macos-latest, windows-latest): toolchain; cache; `cargo test -p screen-side-core -p screen-side`; `cargo build --release -p screen-side`; `SCREEN_SIDE_REAL=1 cargo test -p screen-side-core --test real -- --nocapture`; `target/release/screen-side doctor`; `target/release/screen-side status --json` (allowed to report one screen); Windows: `scripts/pack-cli.sh` equivalent in PowerShell to make a zip and run `install.ps1` with `SCREEN_SIDE_ARCHIVE`, then `screen-side --version`; macOS: `sh scripts/test-install.sh`.
- `shell` (ubuntu-22.04): WebKitGTK toolchain (`libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`); node 22; `npm ci`; `npm run build -w app`; `cargo clippy -p screen-side-gui -- -D warnings`; `cargo test -p screen-side-gui`.
- `web` (ubuntu-latest): node 22 with npm cache; `npm ci`; `npm run typecheck`; `npm test`; `npm run build -w app`; `npm run build -w site`; `npx playwright install --with-deps chromium`; `npm run test:e2e`; upload the Playwright reports on failure.

- [ ] **Step 1: Write `ci.yml` and `scripts/pack-cli.sh`** (`pack-cli.sh <target-triple> <asset-name>` makes a `.tar.gz` holding `screen-side`, `LICENSE` and `README.md`, used by CI and release).
- [ ] **Step 2: Validate locally**: `actionlint` if available (`go run github.com/rhysd/actionlint/cmd/actionlint@latest` or skip); run every command of the `rust` and `web` jobs locally.
- [ ] **Step 3: Commit** `ci: test on Linux, macOS and Windows, the interface and the site`.

### Task 24: Release and Pages workflows

**Files:**
- Create: `.github/workflows/release.yml`, `.github/workflows/pages.yml`, `scripts/check-version.sh`, `scripts/release-notes.sh`

`scripts/check-version.sh [tag]`: reads the version from the workspace `Cargo.toml`, `app/package.json`, `site/package.json` and `app/src-tauri/tauri.conf.json`; fails unless all four are equal and, when a tag is given, equal to the tag without `v`. Add it to the CI `rust` job too.

`scripts/release-notes.sh VERSION`: prints the CHANGELOG section `## [VERSION]` up to the next `## [`, failing if empty.

`release.yml` on `v*` tags, `permissions: contents: read` with `contents: write` only on the publish job. Jobs, each uploading to an artifact with `if-no-files-found: error`:
- `check`: `scripts/check-version.sh "$GITHUB_REF_NAME"`.
- `linux-app` (ubuntu-22.04): toolchain, WebKitGTK deps, `npm ci`, `cargo build --release -p screen-side`, `rm -rf target/release/bundle`, `npx tauri build --bundles deb,rpm,appimage` in `app/`, collect exactly one of each into `screen-side_amd64.deb`, `screen-side_x86_64.rpm`, `screen-side_x86_64.AppImage` (the owl-transfer `take` helper).
- `windows-app` (windows-latest): `npx tauri build --bundles nsis,msi`; collect `ScreenSide_x64-setup.exe`, `ScreenSide_x64.msi`.
- `macos-app` (macos-latest): targets `aarch64-apple-darwin,x86_64-apple-darwin`; `npx tauri build --target universal-apple-darwin --bundles dmg`; collect `ScreenSide_universal.dmg`.
- `cli` matrix: `ubuntu-latest` `x86_64-unknown-linux-musl` to `screen-side-linux-x64.tar.gz` (install `musl-tools`); `ubuntu-24.04-arm` `aarch64-unknown-linux-musl` to `screen-side-linux-arm64.tar.gz`; `macos-latest` both Apple targets plus `lipo -create` to `screen-side-macos-universal.tar.gz`; `windows-latest` `x86_64-pc-windows-msvc` to `screen-side-windows-x64.zip` and `aarch64-pc-windows-msvc` to `screen-side-windows-arm64.zip`.
- `publish` (needs all): download artifacts, write `SHA256SUMS` over all of them, notes from `scripts/release-notes.sh` plus a fixed paragraph about the unsigned installers and the one-liners, `gh release create "$TAG" --title "Screen Side $VERSION" --notes-file notes.md --verify-tag assets/*`.

`pages.yml`: on pushes to `main` touching `site/**`, `app/src/tokens.css`, `docs/img/**`, `package.json`, `package-lock.json` or the workflow; build job (`npm ci`, `npm run build -w site`, upload `site/dist`) with `pages: read`; deploy job with `pages: write, id-token: write`; concurrency `pages` without cancelling. Header comment: Pages must be set to "GitHub Actions" in the repository settings once.

- [ ] **Step 1: Write the scripts with a quick test each** (`sh scripts/check-version.sh v2.0.0` passes, `v2.0.1` fails; `sh scripts/release-notes.sh 2.0.0` prints the section once CHANGELOG exists in Task 25; run that check after Task 25).
- [ ] **Step 2: Write both workflows.** Dry-run what can be run locally: `npx tauri build --bundles deb,appimage` on this machine (Task 26 does this end to end).
- [ ] **Step 3: Commit** `ci: build installers and CLI archives on tags and deploy the site`.

### Task 25: Documentation and community files

Run the copy through the `humanizer` skill before committing, and check every file for emojis and em or en dashes (`grep -nP '[\x{2013}\x{2014}]'` and an emoji range check).

**Files:**
- Create or replace: `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `CHANGELOG.md`, `docs/ARCHITECTURE.md`, `docs/BACKENDS.md`, `.github/ISSUE_TEMPLATE/{bug.yml,feature.yml,desktop.yml,config.yml}`, `.github/PULL_REQUEST_TEMPLATE.md`, `.github/CODEOWNERS`, `.github/dependabot.yml`, `.editorconfig`

Content:
- `README.md` (about 110 lines): centred icon (`app/icon-source.svg`, width 112) and `<h1 align="center">Screen Side</h1>`; a `<picture>` with `docs/img/app-dark.png` and `docs/img/app-light.png`; two paragraphs (what it does and why the pointer is the point; where it runs); `Project site: <https://danieltyukov.github.io/screen-side-switcher/>`; **Install** (Windows, macOS, Linux with the release asset links and the unsigned-build notes; Command line with the two one-liners and `cargo install`); **Using it** (click a screen, pick a side, alignment, primary; layouts; background and tray; shortcut); **Command line** (the block from the spec); **Supported desktops** (the table from the site); **Build from source** (`npm ci`, `npm run tauri dev`, `cargo test`, `npm test`, `SCREEN_SIDE_BACKEND=fake` for working without a second screen); **Contributing** (link to CONTRIBUTING, Discussions, issue forms); **Licence** (MIT). Upgrading from 1.0: one sentence that the installer removes 1.0 from `~/.local`.
- `CONTRIBUTING.md`: layout of the repository; running the tests (Rust, interface, end-to-end, site); running the app against the fake backend and editing its state file; adding a desktop (link `docs/BACKENDS.md`); recording fixtures from a real session (`kscreen-doctor --json`, `wlr-randr --json`, `xrandr --current --props`, with serials removed); conventional commits; a line under "Unreleased" in `CHANGELOG.md`; how a release is cut (bump the four versions, CHANGELOG, tag `vX.Y.Z`, push the tag); help wanted (packaging).
- `docs/ARCHITECTURE.md`: the pieces and how a click becomes a layout; the coordinate rules per backend (the table from the spec, with the rounding notes); the origin rule; why intent is saved instead of coordinates; the watcher's loop guard; the JSON schema of `status --json`, `layouts --json` and `doctor --json`.
- `docs/BACKENDS.md`: one section per backend (how it reads, how it applies, primary, temporary, verify, what is tested where); "Adding a desktop" as a checklist: a pure parser with fixtures, `apply_args`, a `Backend` impl, a detection rule with a probe test, a row in the README and site tables.
- `SECURITY.md`: adapted from garmin-hevy-sync's: what Screen Side touches (display configuration, the config directory, gsettings custom keybindings, autostart entries, the tools it runs); private reporting through GitHub advisories; scope (installers and install scripts, the release workflow, shortcut commands written to gsettings, config file parsing, anything that runs external programs); out of scope; seven-day acknowledgement; only the latest release gets fixes.
- `CODE_OF_CONDUCT.md`: copy garmin-hevy-sync's Contributor Covenant 2.1 with this repository's reporting route.
- `CHANGELOG.md`: Keep a Changelog header; `## [Unreleased]`; `## [2.0.0] - <release date>` with Added (Windows, macOS, KDE, wlroots, X11 backends; app with tray, background and autostart; several screens; saved layouts and auto-apply; end alignment; primary; shortcut setup; doctor; JSON; installers and one-liners; site), Changed (rewritten in Rust and Tauri; GTK app replaced), Removed (the Python package and `install.sh`); `## [1.0.0] - 2026-08-31` with 1.0's summary.
- Issue forms, modelled on garmin-hevy-sync's: `bug.yml` (version from `screen-side --version` or the app's Settings; OS dropdown Windows, macOS, Linux; desktop dropdown GNOME, KDE Plasma, Sway, Hyprland, Other wlroots, X11, Windows, macOS; install method dropdown; what happened; `screen-side doctor` output rendered as text; a required checkbox that serials and names they consider private were removed), `feature.yml` (what you are trying to do, proposed change, kind), `desktop.yml` ("My desktop is not supported or misbehaves": desktop and version, session type, the raw tool output, what went wrong), `config.yml` (`blank_issues_enabled: false`; links: Ideas and questions to `https://github.com/danieltyukov/screen-side-switcher/discussions`, Security problem to the advisories page).
- `PULL_REQUEST_TEMPLATE.md`: What this changes; How I tested it (desktop, OS, real screens or fake backend); Checklist (`cargo test`, `npm test`, clippy, a test for the change, a CHANGELOG line, fixtures with serials removed).
- `CODEOWNERS`: `* @danieltyukov`.
- `dependabot.yml`: `cargo` (weekly, minor and patch grouped), `npm` at `/` (weekly, grouped), `github-actions` (weekly, grouped).
- `.editorconfig`: studocuhack's.

- [ ] **Step 1: Write the files.** **Step 2: Check** `sh scripts/release-notes.sh 2.0.0` prints the section; the dash and emoji greps are empty; every relative link in the Markdown resolves (`grep -o '](\([^)]*\))'` and test each path). **Step 3: Commit** `docs: add the README, contributing guide, architecture notes and issue forms`.

---

## Phase 5: verification and publishing

### Task 26: Full verification

- [ ] **Step 1: Everything green locally**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check -p screen-side-core -p screen-side --target x86_64-pc-windows-gnu
cargo check -p screen-side-core -p screen-side --target aarch64-apple-darwin
npm run typecheck && npm test && npm run build && npm run build -w site && npm run test:e2e
shellcheck site/public/install.sh scripts/*.sh && sh scripts/test-install.sh
sh scripts/check-version.sh v2.0.0
```

Expected: every command exits 0. Record the test counts.

- [ ] **Step 2: Real GNOME run.** `SCREEN_SIDE_REAL=1 cargo test -p screen-side-core --test real -- --nocapture`; `target/release/screen-side status`, `doctor`, `--json`. If the maintainer agrees, with the laptop screen on: `screen-side right --dry-run` (Mutter verify), then `screen-side right --temporary` and `screen-side toggle --temporary` to watch the real screens move, then put them back.
- [ ] **Step 3: Build and launch the Linux bundles.** `cargo build --release -p screen-side && (cd app && npx tauri build --bundles deb,appimage)`; run the AppImage; take a screenshot of the real window; check the tray; check `dpkg -c` lists `/usr/bin/screen-side` and `/usr/bin/screen-side-gui`.
- [ ] **Step 4: Whole-branch review.** Dispatch a reviewer (superpowers:requesting-code-review, or `pr-review-toolkit:code-reviewer`) on `git diff main...v2` with the spec and this plan; fix what it confirms; re-run Step 1.
- [ ] **Step 5: Commit** any fixes: `fix: <what>`.

### Task 27: Publish (only with the maintainer's go)

Each item is outward-facing; ask before doing it.

- [ ] Push `v2` and open a pull request to `main` with a summary, the test evidence and screenshots, or merge directly if the maintainer prefers. Watch CI with `gh pr checks --watch` and fix failures.
- [ ] After merging: `gh repo edit danieltyukov/screen-side-switcher --description "<spec description>" --homepage https://danieltyukov.github.io/screen-side-switcher/ --enable-discussions --add-topic ...` (remove `gtk4`, `libadwaita`; add `rust`, `tauri`, `windows`, `macos`, `kde`, `sway`, `hyprland`, `x11`, `cli`, `tray-app`, `monitor-arrangement`).
- [ ] Pages source "GitHub Actions": `gh api -X POST repos/danieltyukov/screen-side-switcher/pages -f build_type=workflow` (or PUT if it exists); run the Pages workflow; open the site and check it.
- [ ] Labels: `gh label create "desktop: gnome"` and the other five, plus `help wanted` and `good first issue` if missing.
- [ ] Tag: `git tag -a v2.0.0 -m "Screen Side 2.0.0"`, push the tag, watch the release workflow, then check every asset downloads and `SHA256SUMS` matches, and the site's download buttons resolve.
