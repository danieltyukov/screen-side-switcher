# Contributing

Thanks for helping. Screen Side is small on purpose: one job, done well on
every desktop. The most useful contributions are reports and fixes for
desktops the maintainer cannot test, and packaging.

## Layout

```
crates/core/        the logic: model, layout maths, backends, saved layouts, watch,
                    shortcut setup, doctor. No Tauri.
  src/backend/      one file per desktop, each split into a pure part (parsing,
                    building the request) and a thin part that talks to the system
  tests/fixtures/   recorded output from real sessions
crates/cli/         the screen-side command
app/                the window: React and TypeScript behind src/backend/types.ts
app/src-tauri/      the Tauri shell: commands, tray, watcher, autostart, hotkey
site/               the project site, and install.sh and install.ps1
docs/               ARCHITECTURE.md, BACKENDS.md and the screenshots
scripts/            packing, version and installer checks used by CI
```

## Running the tests

```
cargo test --workspace            # core, CLI and the shell (the shell needs WebKitGTK on Linux)
cargo test -p screen-side-core -p screen-side   # without the shell
npm ci && npm test                # the interface, against the mock backend
npm run test:e2e                  # Playwright: the built app and the site
sh scripts/test-install.sh        # install.sh against a locally built archive
SCREEN_SIDE_REAL=1 cargo test -p screen-side-core --test real -- --nocapture
```

The last one talks to your real display system. It only queries and then
checks the current layout without changing it, so it is safe on a desktop.

On Linux the shell needs the WebKitGTK toolchain:
`libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev`.

## Working without a second screen

`SCREEN_SIDE_BACKEND=fake` swaps the real display system for two pretend
screens kept in a JSON file:

```
export SCREEN_SIDE_BACKEND=fake SCREEN_SIDE_FAKE_STATE=/tmp/screens.json
cargo run -p screen-side -- status
cargo run -p screen-side -- right
npm run tauri dev
```

Edit the file to add screens, switch one off (`"enabled": false`) or change
what the system supports (`capabilities`). In a browser, `npm run dev` opens
the window against an in-memory mock: `?screens=1|2|3`, `?backend=wlroots`,
`?theme=dark` and `?error=...` show the other states.

## Adding or fixing a desktop

[docs/BACKENDS.md](docs/BACKENDS.md) has the checklist. In short: a parser
from the desktop's own output to a `State`, tested against output recorded on
a real session; the arguments that apply a `Layout`; a `Backend` impl; and a
detection rule. Record fixtures with `kscreen-doctor --json`,
`wlr-randr --json` or `xrandr --current --props`, and replace serial numbers
with something made up before committing them.

## Commits and pull requests

- Conventional commit messages: `feat:`, `fix:`, `docs:`, `test:`, `ci:`, `chore:`.
- A test for the change, in the layer it belongs to.
- A line under "Unreleased" in `CHANGELOG.md` for anything a person would notice.
- `cargo fmt`, `cargo clippy -- -D warnings` and `npm run typecheck` clean.

## Releases

Bump the version in `Cargo.toml`, `app/package.json`, `site/package.json`
and `app/src-tauri/tauri.conf.json` (`sh scripts/check-version.sh` checks
they agree), move the "Unreleased" notes into a section for the version, and
push a `vX.Y.Z` tag. The release workflow builds every installer and archive
and publishes them with the CHANGELOG section as notes.

## Help wanted

- Packages for Homebrew, winget, the AUR and Flathub.
- Reports from KDE, Sway, Hyprland, X11, Windows and macOS with two or more
  screens: what worked and what did not.
- Translations of the window, once there is more than one person asking.
