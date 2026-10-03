# Screen Side 2.0: cross-platform rewrite, design

Date: 2026-10-04
Status: approved in conversation, pending spec review

## Intent

Screen Side 1.0 is a GNOME-only Python tool that puts an external monitor on the
left, right, above or below the laptop screen so the pointer crosses the edge
that matches the desk. Version 2.0 makes it a project anyone can use and
contribute to:

- it runs on Windows, macOS and the common Linux desktops, not only GNOME;
- it ships as real installers plus a command-line tool;
- it gains several-screen support, saved layouts per desk, primary and end
  alignment, shortcut setup, `doctor` and JSON output;
- it has a project site, a concise README, and the community files and
  workflows the maintainer's other public repositories have;
- it is tested: unit, parser-fixture, CLI integration, interface and end-to-end
  tests, CI on all three operating systems, and real runs where a real backend
  is available.

The maintainer chose a Tauri rewrite (the owl-transfer pattern) over keeping
Python. The repository conventions follow owl-transfer, garmin-hevy-sync and
studocuhack.

Success means: a person on any of the supported desktops downloads one file or
runs one command, and switches the side of their external screen with a click,
a tray menu, a hotkey or a command, and their per-desk layout comes back by
itself when they plug in at that desk.

## Repository layout

```
Cargo.toml                 workspace: crates/core, crates/cli, app/src-tauri
crates/core/               screen-side-core (lib): model, layout maths, backends,
                           saved layouts, watch, shortcut, doctor. No Tauri.
  src/model.rs             Screen, State, Rect, Identity, Layout
  src/layout.rs            Arrangement, compute, infer, operations
  src/store.rs             saved layouts and settings on disk
  src/watch.rs             change detection and auto-apply
  src/shortcut.rs          per-desktop shortcut setup
  src/doctor.rs            diagnostics report
  src/backend/mod.rs       Backend trait, Capabilities, detect()
  src/backend/{fake,gnome,kde,wlroots,x11,windows,macos}.rs
  tests/fixtures/          recorded backend output
crates/cli/                `screen-side` binary (clap)
  tests/                   integration tests against the fake backend
app/                       React 19 + Vite interface
  src/backend/             the TypeScript Backend seam, Tauri adapter, mock
  src/tokens.css           design tokens shared with the site
  e2e/                     Playwright against the built interface and the mock
app/src-tauri/             Tauri 2 shell: commands, tray, watcher, autostart,
                           global shortcut, single instance
site/                      project site (Vite workspace package)
  install.sh, install.ps1  CLI one-line installers, served from Pages
docs/
  ARCHITECTURE.md          how the pieces fit, the coordinate rules
  BACKENDS.md              per-desktop notes and how to add a desktop
  img/                     screenshots (generated from the mock)
.github/                   issue forms, PR template, CODEOWNERS, dependabot,
                           workflows ci.yml, pages.yml, release.yml
CHANGELOG.md CONTRIBUTING.md SECURITY.md CODE_OF_CONDUCT.md LICENSE README.md
.editorconfig .gitignore package.json (npm workspaces: app, site)
```

Removed: `screenside/`, `bin/`, `install.sh`, `uninstall.sh`, `data/` (the icon
moves to `app/icon-source.svg`, the desktop entry to `app/src-tauri/`). Version
becomes 2.0.0. The 1.0 code stays in git history.

## Core model

```rust
pub struct Rect { pub x: i32, pub y: i32, pub width: i32, pub height: i32 }

pub struct Identity {            // for matching saved layouts across sessions
    pub vendor: String,          // EDID PnP id ("DEL") or make ("Dell Inc.")
    pub product: String,         // product code or model
    pub serial: String,          // may be empty
}

pub struct Screen {
    pub id: String,              // stable within a session: connector name,
                                 // CCD target id, CGDirectDisplayID
    pub connector: String,       // "eDP-1", "HDMI-1", "\\.\DISPLAY2"
    pub name: String,            // "DELL U2723QE", "Built-in display"
    pub identity: Identity,
    pub builtin: bool,
    pub enabled: bool,           // part of the desktop right now
    pub primary: bool,
    pub rect: Rect,              // in the backend's coordinate space
    pub scale: f64,              // informational
}

pub struct State { pub backend: String, pub screens: Vec<Screen> }

pub struct Layout {              // what a backend is asked to apply
    pub positions: Vec<(String /* screen id */, i32, i32)>,
    pub primary: String,
}
```

`Identity::key()` is `vendor:product:serial` lower-cased and trimmed. When the
backend reports neither vendor nor product the key is `connector:<connector>`.

Each backend fills `rect` in its own coordinate space and does the unit
conversion itself, so the layout maths only ever sees sizes that are adjacent
when they touch. This is the generalisation of 1.0's `layout-mode` fix.

| Backend | Space |
|---|---|
| gnome | logical or physical pixels, from Mutter's `layout-mode`; size from the current mode, rotated by transform, divided by scale (rounded) in logical mode |
| kde | logical pixels: current mode size, swapped for rotation 2 or 8, divided by scale |
| wlroots | logical pixels: current mode size, swapped for 90/270 transforms, divided by scale |
| x11 | pixels, as reported by xrandr (already rotated) |
| windows | physical desktop pixels from the CCD source mode |
| macos | points from `CGDisplayBounds` |
| fake | whatever the state file says |

## Layout maths (`layout.rs`)

```rust
pub enum Side { Left, Right, Above, Below }
pub enum Align { Start, Center, End }

pub struct Placement { pub screen: String, pub side: Side }

pub struct Arrangement {
    pub anchor: String,              // screen id
    pub placements: Vec<Placement>,  // nearest first within each side
    pub align: Align,
    pub primary: String,             // screen id
}
```

Only enabled screens take part. Disabled ones (a closed lid) are listed but
never moved, enabled or disabled.

**Anchor.** The enabled built-in screen; else the primary; else the first
enabled screen.

**compute(state, arrangement) -> Result<Layout, LayoutError>.**
1. The anchor is placed at (0, 0) with its size.
2. Each side is a chain. For Left, the first screen's right edge meets the
   anchor's left edge, the next screen's right edge meets the first's left edge,
   and so on. Right, Above and Below are the same along their axis.
3. On the cross axis every screen is aligned against the anchor: Start puts
   their top (or left) edges level, End their bottom (or right) edges, Center
   uses `floor((anchor - screen) / 2)`.
4. Checks, in order: every enabled screen has a placement or is the anchor
   (`MissingScreen`); no two rectangles overlap with positive area (`Overlap {a,
   b}`, explained in plain words); every screen shares an edge segment of
   positive length with at least one other, and the touch graph is connected
   (`Disconnected`). Chains guarantee the last two for one side; perpendicular
   sides can overlap (a tall screen on the left and a wide one above), and that
   is refused, never "fixed" by opening a gap the pointer cannot cross.
5. Normalisation by the backend's origin rule: `TopLeft` (gnome, kde, wlroots,
   x11, fake) translates so the minimum x and y are 0; `Primary` (windows,
   macos) translates so the primary screen is at (0, 0), which is how those
   systems define the primary/main display.

**infer(state) -> Option<Arrangement>.** Reads the arrangement in force.
Each non-anchor enabled screen is classified by which half-plane it lies in
relative to the anchor (entirely left, right, above or below; a diagonal screen
takes the axis with the larger gap). Order within a side is by distance from the
anchor. Alignment is the first of Center, Start, End for which every screen's
offset matches within 1 pixel (equal sizes match all three and report Center);
if none matches the arrangement is still returned with `align` set to Center
and `aligned: false` so the interface can show "custom". Returns None if any
screen overlaps the anchor.

**Operations** (all produce a new Arrangement, then `compute`):
- `move_all(side)`: every non-anchor screen goes to `side`, keeping their
  current order along the new axis (fallback: the backend's list order). This is
  1.0's behaviour.
- `move_screen(id, side)`: one screen moves; it becomes the farthest on its new
  side; the others keep their sides and order.
- `toggle()`: mirror, Left with Right and Above with Below. With nothing
  inferable it means `move_all(Left)`.
- `set_align(align)`, `set_primary(id)`.

When `infer` returns None (a hand-made layout), operations start from "every
non-anchor screen to the right of the anchor in x order", then apply.

## Backends

```rust
pub trait Backend {
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> Capabilities;
    fn query(&self) -> Result<State>;
    fn apply(&self, layout: &Layout, mode: ApplyMode) -> Result<()>;
    fn diagnostics(&self) -> Vec<(String, String)>;
}
pub enum ApplyMode { Persistent, Temporary, Verify }
pub struct Capabilities {
    pub primary: bool,        // can set the primary screen
    pub temporary: bool,      // ApplyMode::Temporary distinct from Persistent
    pub verify: bool,         // a real dry run by the system
    pub remembers: bool,      // the system restores layouts per set of screens
    pub origin: Origin,       // TopLeft or Primary
}
```

`apply` re-reads whatever private state it needs (Mutter's serial and mode ids,
CCD paths) and fails with `Error::Changed` if the screens changed in between.
`--dry-run` uses `Verify` where `capabilities.verify`, otherwise it computes
and prints without calling the system.

**Detection** (`detect()`), first match wins:
1. `SCREEN_SIDE_BACKEND` names one (`fake`, `gnome`, `kde`, `wlroots`, `x11`,
   `windows`, `macos`); an unknown name is a usage error.
2. Windows: `windows`. macOS: `macos`.
3. Linux and BSD: `gnome` if the session bus has an owner for
   `org.gnome.Mutter.DisplayConfig` (GNOME, Pantheon, Budgie on Mutter) or
   `org.cinnamon.Muffin.DisplayConfig` (Cinnamon; same interface, untested);
   `kde` if `XDG_CURRENT_DESKTOP` contains `KDE` and `kscreen-doctor` is on
   PATH; `wlroots` if `WAYLAND_DISPLAY` is set and `wlr-randr` is on PATH;
   `x11` if `WAYLAND_DISPLAY` is unset, `DISPLAY` is set and `xrandr` is on
   PATH.
4. Otherwise `Error::NoBackend` with the specific missing piece ("this looks
   like a Wayland session without wlr-randr; install wlr-randr").

| Backend | Query | Apply | Primary | Temporary | Verify | Remembers |
|---|---|---|---|---|---|---|
| gnome | `GetCurrentState` over zbus (blocking) | `ApplyMonitorsConfig`, method 2/1/0, keeps scale, transform, mode; passes the current `layout-mode` when `supports-changing-layout-mode` | yes | yes | yes | yes |
| kde | `kscreen-doctor --json` | `kscreen-doctor output.N.position.X,Y ...` in one call; primary via `output.N.priority.1` (Plasma 6) or `output.N.primary` (Plasma 5), chosen by whether its JSON has `priority` fields | yes | no | no | yes |
| wlroots | `wlr-randr --json` | `wlr-randr --output N --pos X,Y ...` in one call (one atomic configuration) | no | no (never persists) | yes, `--dryrun` | no |
| x11 | `xrandr --current --props` (geometry lines and EDID blocks) | `xrandr --output N --pos XxY ... [--output P --primary]` in one call | yes | no (never persists) | no | no |
| windows | `QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS)` + `DisplayConfigGetDeviceInfo` for names and EDID ids | `SetDisplayConfig` with `SDC_APPLY | SDC_USE_SUPPLIED_DISPLAY_CONFIG | SDC_ALLOW_CHANGES` (+ `SDC_SAVE_TO_DATABASE` when persistent; `SDC_VALIDATE` for verify) | yes (origin) | yes | yes | yes |
| macos | `CGGetActiveDisplayList`, `CGDisplayBounds`, `CGDisplayIsBuiltin`, vendor/model/serial numbers; names from `NSScreen.localizedName` on the main thread | `CGBeginDisplayConfiguration` + `CGConfigureDisplayOrigin` + `CGCompleteDisplayConfiguration` (permanently or for session) | yes (origin) | yes | no | yes |
| fake | JSON file at `SCREEN_SIDE_FAKE_STATE` (default: a built-in two-screen state) | writes positions back to the file | flag in file | yes | yes | flag in file |

Built-in detection: Mutter's `is-builtin`; KDE type 7 (Panel); connector
prefixes `eDP`, `LVDS`, `DSI` for wlroots and x11; CCD output technology
`INTERNAL`, `LVDS`, `DISPLAYPORT_EMBEDDED` or `UDI_EMBEDDED`;
`CGDisplayIsBuiltin`. Mirrored secondaries (macOS `CGDisplayMirrorsDisplay`,
KDE clones) are listed as disabled.

Identity sources: Mutter vendor/product/serial; wlr-randr make/model/serial;
xrandr EDID (manufacturer id bytes 8-9, product bytes 10-11, serial descriptor
or bytes 12-15); CCD `edidManufactureId` (byte-swapped PnP id) and
`edidProductCodeId`, no serial; Quartz vendor/model/serial numbers; KDE has no
EDID in its JSON, so its key is the connector name.

Each parser is a pure function from the tool's raw output (or the API's plain
structs) to `State`, tested against fixtures in `crates/core/tests/fixtures/`.
The process-running and API-calling part of each backend is kept thin.

## Saved layouts and settings (`store.rs`)

Directory: `SCREEN_SIDE_CONFIG_DIR` if set, else
`directories::ProjectDirs::from("io.github", "danieltyukov", "ScreenSide")`
config dir (`~/.config/screenside`, `~/Library/Application
Support/io.github.danieltyukov.ScreenSide`,
`%APPDATA%\danieltyukov\ScreenSide\config`). Writes are atomic (temp file and
rename). A file with a newer `version` than this build understands is refused
rather than overwritten.

`layouts.json`:

```json
{
  "version": 1,
  "layouts": [
    {
      "name": "office",
      "screens": ["boe:0x095f:", "del:0x41b5:abc123"],
      "anchor": "boe:0x095f:",
      "placements": [{ "screen": "del:0x41b5:abc123", "side": "left" }],
      "align": "center",
      "primary": "del:0x41b5:abc123",
      "auto": true,
      "saved": "2026-10-04T12:00:00Z"
    }
  ]
}
```

`screens` is the sorted multiset of identity keys of the enabled screens when
saved. A layout matches a state when that multiset is equal. Identical keys
(two monitors of one model without serials) are resolved in connector order.
Applying a saved layout maps keys to current screen ids, builds an Arrangement
and runs `compute`, so it keeps working after scale or resolution changes.
Names are 1 to 64 characters, trimmed, unique case-insensitively.

`settings.json` (app only): `background` (bool; default true on Windows and
macOS, false on Linux), `auto_apply` (bool, default true), `shortcut`
(string or null, default null: off until the user picks keys).

## Watch (`watch.rs`)

`Watcher::tick(backend, store) -> Event` is the unit under test; `watch` loops
it every 2 seconds. Fingerprint: the sorted identity keys of enabled screens.
On a fingerprint change: if an `auto` layout matches and `auto_apply` is on,
wait 1 second, re-query, and apply it if the fingerprint is still the same.
Its own apply does not change the fingerprint, so it cannot loop. Errors are
reported as events and never stop the loop. Used by `screen-side watch` and by
the app's background thread, which also emits a state-changed event on any
change of positions so the window and tray stay current.

## Shortcut (`shortcut.rs`)

`plan(desktop, command, keys) -> ShortcutPlan` is pure and tested;
`install`/`remove` execute it.
- GNOME (Wayland or X11): a custom keybinding at
  `/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/screen-side/`
  added to the `custom-keybindings` list via `gsettings`, name "Screen Side
  toggle", command `<command> toggle`. Remove takes it out of the list and
  resets its keys. Default keys `<Super><Alt>s`.
- Sway, Hyprland, KDE: print the exact line or settings path to add, with the
  absolute command.
- Windows, macOS, other X11: say that the app registers the shortcut while it
  runs (Settings, Keyboard shortcut).
The command is the absolute path of the running binary; the CLI passes its own
path plus `toggle`, the app passes its own path (or `$APPIMAGE`) plus
`--toggle`. Paths are quoted for the target shell; layout names never reach a
shell.

## Doctor (`doctor.rs`)

Text and JSON: Screen Side version, OS and version, architecture, desktop
(`XDG_CURRENT_DESKTOP`, `XDG_SESSION_TYPE`), chosen backend and why, the other
backends' availability, `capabilities`, tool versions (`kscreen-doctor`,
`wlr-randr`, `xrandr`), config directory and how many layouts it holds, and the
current state. Serials are shown as "present" or "absent", never printed,
because the bug template asks people to paste this.

## Command line (`crates/cli`)

```
screen-side [status] [--json]
screen-side left|right|above|below [--screen S] [--align start|center|end]
                                   [--temporary] [--dry-run] [--json]
screen-side toggle [--temporary] [--dry-run]
screen-side primary S [--dry-run]
screen-side align start|center|end [--dry-run]
screen-side save NAME [--auto]
screen-side apply NAME [--temporary] [--dry-run]
screen-side layouts [--json]
screen-side forget NAME
screen-side watch [--interval SECONDS]
screen-side shortcut install|remove|show [--keys KEYS]
screen-side doctor [--json]
screen-side --version | --help
```

`S` is a 1-based number from `status`, a connector, or a case-insensitive
substring of the name; ambiguity is an error that lists the candidates.
Exit codes: 0 done; 1 the system or the layout maths refused (message on
stderr); 2 usage error, unknown screen, or a feature the backend lacks.
`--json` output carries `"schema": 1` and is documented in ARCHITECTURE.md.

Status text:

```
External screen is left of the built-in screen. Alignment: centred.
  1  HDMI-1   DELL U2723QE        2560x1440  at 0,0       scale 1
  2  eDP-1    Built-in display    1280x800   at 2560,320  scale 1.5  primary
Backend: gnome (Mutter, Wayland). Saved layout in force: office.
```

## App (`app/` and `app/src-tauri/`)

Binary `screen-side-gui`, product name "Screen Side", identifier
`io.github.danieltyukov.ScreenSide`, native window decorations, 560x680
default, 440x560 minimum, single instance.

Launch arguments: `--background` (start hidden; used by autostart),
`--toggle`, `--apply NAME`. With `--toggle` or `--apply` the work is done
and, if no instance is running, the process exits without a window; if one is
running the arguments are forwarded to it.

Window, three tabs:
- **Arrange.** An SVG preview of the enabled screens to scale, labelled, the
  primary marked; click a screen to select it (the selection defaults to the
  only external screen). The side buttons move the selected screen (or all
  screens when "All screens" is selected). Alignment segmented control (Start,
  Centre, End; labels say Top/Bottom or Left/Right according to the axis).
  "Make primary" on the selected screen when the backend supports it. A line of
  plain text under the preview says where the pointer crosses. Errors show in a
  banner and the preview returns to the real state.
- **Layouts.** Saved layouts, the one matching the connected screens first,
  others greyed with "for other screens". Apply, auto-apply switch, delete.
  "Save current layout" with a name field.
- **Settings.** Keep running in the background with a tray icon; start at
  login; apply saved layouts automatically; keyboard shortcut (key recorder on
  Windows, macOS and X11; a "Set up" button on GNOME; instructions on Sway,
  Hyprland and KDE); the path of the command-line tool if found; "Copy
  diagnostics" (the doctor text); version and links (site, issues,
  Discussions).

Tray (when background is on): Left, Right, Above, Below (all external
screens), Toggle, a Layouts submenu (matching layouts enabled, others
disabled), Open Screen Side, Quit. Template icon on macOS. Closing the window
hides it when background is on and quits otherwise.

Shell modules: `commands.rs` (invoke handlers, each a thin call into core),
`watcher.rs` (background thread on `core::watch`, emits `state-changed`),
`tray.rs`, `shortcut.rs` (tauri-plugin-global-shortcut, not registered under
Wayland), `settings.rs`. Plugins: single-instance, autostart, global-shortcut,
opener, clipboard-manager. On macOS, commands that query the backend run on the
main thread (for `NSScreen` names).

Interface: React 19, TypeScript, Vite. `src/backend/types.ts` is the only seam
(same pattern as owl-transfer); `tauri.ts` adapts invoke and events; `mock.ts`
holds an in-memory multi-screen state with a small TypeScript copy of the chain
maths so the interface is usable and testable in a browser (`?screens=1|2|3`,
`?backend=wlroots` to see capability differences). Design tokens in
`src/tokens.css`, light and dark with the same three-path rule as owl-transfer,
system UI font stack. Brand colours from the 1.0 icon: deep blue and the
pointer yellow.

## Distribution

Release workflow on a `v*` tag, published with `gh release create` (no
third-party release action). Stable asset names, so `releases/latest/download/`
links on the site and in the README never change:

| Asset | Built on |
|---|---|
| `screen-side_amd64.deb`, `screen-side_x86_64.rpm`, `screen-side_x86_64.AppImage` | ubuntu-22.04 (glibc floor) |
| `ScreenSide_x64-setup.exe` (NSIS, per-user, fetches WebView2), `ScreenSide_x64.msi` | windows-latest |
| `ScreenSide_universal.dmg` (ad-hoc signed) | macos-latest |
| `screen-side-linux-x64.tar.gz`, `screen-side-linux-arm64.tar.gz` (musl, static) | ubuntu-latest, ubuntu-24.04-arm |
| `screen-side-macos-universal.tar.gz` | macos-latest |
| `screen-side-windows-x64.zip`, `screen-side-windows-arm64.zip` | windows-latest |
| `SHA256SUMS` | publish job |

The `.deb` and `.rpm` also install `/usr/bin/screen-side` (Tauri `files`).
The release checks that the tag equals the workspace version, and takes its
notes from the CHANGELOG section for that version.

`site/install.sh` (macOS, Linux): detects OS and architecture, downloads the
CLI archive and `SHA256SUMS` from the latest release, verifies, installs to
`~/.local/bin` (or `SCREEN_SIDE_INSTALL_DIR`), removes a 1.0 install from
`~/.local` if present (`~/.local/share/screen-side-switcher`, the 1.0
launchers, desktop entry and icon, each only if it is the 1.0 file), and points
to the app downloads. `site/install.ps1` (Windows): the same into
`%LOCALAPPDATA%\Programs\screen-side`, added to the user PATH. Also
`cargo install --git https://github.com/danieltyukov/screen-side-switcher screen-side`.

No code signing: SmartScreen (More info, Run anyway) and Gatekeeper (System
Settings, Privacy and Security, Open Anyway) are explained in the README, the
site and the release notes.

## Site (`site/`)

Static, one page, built with Vite, published to
`https://danieltyukov.github.io/screen-side-switcher/` by `pages.yml`. Imports
`app/src/tokens.css`. Light and dark with a toggle, same storage pattern as
owl-transfer. No framework, no tracking.

Sections:
1. Header: mark, wordmark, links (GitHub, Docs, Releases, Discussions), theme
   toggle.
2. Hero: one-line promise, short lead, a download button for the visitor's OS
   (script picks it; without script all downloads are listed), and tabs
   Windows, macOS, Linux, Command line with copyable commands.
3. Interactive demo: an SVG desk with a laptop and a monitor. Buttons Left,
   Right, Above, Below and an alignment control rearrange it with a short
   transition and animate the pointer crossing the shared edge. Reduced motion
   turns animation off.
4. What it does: several screens, saved layouts per desk, tray and hotkey,
   command line, primary and alignment. Screenshots of the app (light and dark).
5. Works on: the supported-desktops table with how each is driven and whether
   it is tested on real hardware or by fixtures (asking for reports).
6. Command line: the main commands.
7. Contribute: report a bug (with `doctor`), suggest an idea (Discussions), add
   a desktop (BACKENDS.md), help wanted (packaging for Homebrew, winget, AUR,
   Flathub).
8. Footer: licence, version, links.

Also `favicon.svg`, `og.png` (1200x630, rendered from SVG), canonical and
Open Graph tags, and `install.sh`/`install.ps1` copied to the site root.

## README

About 110 lines, prose like the other repositories: centred icon and title, a
`<picture>` light/dark screenshot, two short paragraphs, project site link,
Install (per OS, with the unsigned-build notes), Using it, Command line,
Supported desktops (table), Build from source, Contributing, Licence.

## Community and repository

- `CONTRIBUTING.md`: layout, running tests, running the app against the fake
  backend, adding a desktop (link to BACKENDS.md), conventional commit
  messages, CHANGELOG under "Unreleased", how releases are cut.
- `SECURITY.md`: private advisories; scope is the installers and install
  scripts, the release pipeline, the shortcut commands written to gsettings,
  the config files, anything that runs external tools.
- `CODE_OF_CONDUCT.md`: Contributor Covenant 2.1, as in garmin-hevy-sync.
- `CHANGELOG.md`: Keep a Changelog, `## [2.0.0]` and `## [1.0.0]`.
- Issue forms: `bug.yml` (version, OS, desktop, install method, what happened,
  `screen-side doctor` output), `feature.yml`, `desktop.yml` (support for a new
  or broken desktop: desktop, session type, raw tool output). `config.yml`
  links Discussions (Ideas, Q&A) and private security reporting.
- `PULL_REQUEST_TEMPLATE.md`, `CODEOWNERS` (`* @danieltyukov`), `dependabot.yml`
  (cargo, npm, github-actions, weekly, grouped).
- `.editorconfig` as in studocuhack.
- GitHub settings, done when publishing and only with the maintainer's go:
  description, homepage, topics, Discussions on, Pages source "GitHub Actions",
  labels per desktop (`desktop: gnome`, `desktop: kde`, `desktop: wlroots`,
  `desktop: x11`, `desktop: windows`, `desktop: macos`).

Repository description (under 350 characters): "Put your external monitor
left, right, above or below your laptop screen with one click, a hotkey or a
command, and get each desk's layout back when you plug in. Windows, macOS and
Linux (GNOME, KDE, Sway, Hyprland, X11)."

## Testing

The maintainer asked for this to be tested; every layer has its own tests and
CI runs them on every pull request.

- **Core unit tests:** compute (each side, each alignment, chains of 2 and 3,
  mixed sides, overlap refusal, disconnected refusal, both origin rules,
  rounding of odd differences), infer (each side, chains, diagonal, overlap,
  custom alignment), operations (move_all, move_screen, toggle, set_primary,
  fallback when infer fails), 1.0's behaviour (the two-screen cases produce the
  same coordinates as 1.0 up to the documented 1-pixel centring rule), identity
  keys, layout matching including duplicate keys, store round trip and atomic
  write and newer-version refusal, watcher ticks (no change, change with and
  without a matching auto layout, own apply does not loop, errors become
  events), shortcut plans per desktop, detection with injected environment.
- **Parser fixtures:** recorded output for Mutter (built from the D-Bus tuple
  types), `kscreen-doctor --json` (Plasma 5 and 6), `wlr-randr --json`,
  `xrandr --current --props` (with EDID), CCD structs, Quartz values; rotated
  and scaled screens, disabled screens, mirrored screens.
- **CLI integration** (assert_cmd) on the fake backend: every command, text and
  JSON output, exit codes, `--dry-run` leaves the state file untouched,
  `--temporary`, ambiguous `--screen`, unsupported primary on a wlroots-like
  fake.
- **Interface:** vitest and Testing Library against the mock (preview, side
  buttons, alignment labels by axis, primary hidden without the capability,
  layouts list and save, settings per platform, error banner); Playwright
  against `vite preview` of the built app with the mock, light and dark, which
  also produces the screenshots in `docs/img`.
- **Site:** a Playwright smoke (page loads, OS button chosen, demo buttons move
  the screens, copy buttons) and `shellcheck` on `install.sh`.
- **CI (`ci.yml`):** fmt; clippy with `-D warnings`; core and CLI tests on
  ubuntu, macos and windows runners; on macOS and Windows runners the real
  backend runs `screen-side doctor` and `screen-side status --json` against the
  runner's display; cross `cargo check` for the other targets on Linux;
  interface typecheck, vitest, build and Playwright; site build and smoke;
  `cargo check` of the Tauri shell with the WebKitGTK toolchain; shellcheck;
  the install scripts run against a locally built archive.
- **Real runs here:** the GNOME backend (status, doctor, verify-only dry run)
  on the maintainer's GNOME 46 Wayland session; building and launching the
  Linux app. Moving the real screens is done only with the maintainer's go.
  KDE, wlroots and X11 are covered by fixtures until someone runs them; the
  docs and the site say so and invite reports.

## Out of scope

Mirroring; changing resolution, scale, refresh rate or rotation; enabling or
disabling screens; code signing and notarisation; Homebrew, winget, AUR and
Flathub packages (listed as help wanted); mobile; a per-screen alignment.

## Risks

- The KDE, wlroots, x11 and Cinnamon backends are written against upstream
  source and recorded output, not real sessions. Mitigation: fixture tests,
  the `desktop.yml` issue form, and saying so in the docs.
- Windows and macOS apply paths can only be exercised for real on hardware with
  two screens; CI runners have one. Mitigation: query paths run in CI, apply
  paths are thin over pure conversion functions that are unit tested, and the
  `Verify` mode on Windows is run in CI.
- Unsigned installers put some people off. Mitigation: clear instructions, and
  the CLI one-liners, which need no Gatekeeper or SmartScreen step for the CLI.
