# Backends

One file per desktop in `crates/core/src/backend/`. Each is split in two: a
pure part that turns the system's data into a `State` and a `Layout` into the
system's request, compiled and tested on every operating system; and a thin
part that talks to the system. Capabilities say what each one can do.

| Backend | Primary | Temporary | Checks without applying | Remembers per set of screens |
|---|---|---|---|---|
| gnome | yes | yes | yes | yes |
| kde | yes | no | no | yes |
| wlroots | no | no | yes | no |
| x11 | yes | no | no | no |
| windows | yes | yes | yes | yes |
| macos | yes | yes | no | yes |

Where the system remembers nothing (wlroots, X11), saved layouts marked Auto
and `screen-side watch` do the remembering.

## GNOME (`mutter.rs`, `gnome.rs`)

`org.gnome.Mutter.DisplayConfig` over the session bus with zbus:
`GetCurrentState` to read and `ApplyMonitorsConfig` to write, with method 2
(persistent), 1 (temporary) or 0 (verify). Each logical monitor keeps its
scale, transform, mode and any mirrored monitors; only positions and the
primary flag change. The current `layout-mode` is passed back when Mutter
allows changing it, and each monitor's underscanning and colour mode are
passed back so a move does not reset them. A saved change makes GNOME Shell
ask "Keep these display settings?"; unattended changes are temporary for that
reason (see ARCHITECTURE.md). Cinnamon's `org.cinnamon.Muffin.DisplayConfig` has the
same interface and is picked when Mutter is absent (untested).

Tested: fixtures for logical and physical layout modes, fractional scales,
rotation, a closed lid and mirroring, plus `SCREEN_SIDE_REAL=1` on GNOME 46.

## KDE Plasma (`kde.rs`)

`kscreen-doctor --json` to read (libkscreen's serializer: `outputs` with
`name`, `type`, `enabled`, `connected`, `pos`, `scale`, `rotation`,
`currentModeId`, `modes`, and `priority` on Plasma 6 or `primary` on Plasma
5), and `kscreen-doctor output.NAME.position.X,Y ... output.NAME.priority.1`
(or `output.NAME.primary`) to write, in one call. Type 7 is a built-in panel.
The JSON has no EDID, so screens are known by connector.

## wlroots (`wlroots.rs`)

`wlr-randr --json` (0.3 or newer) to read and
`wlr-randr --output A --pos X,Y --output B --pos X,Y` to write, which is one
atomic configuration; `--dryrun` checks without applying. There is no primary
screen in the protocol. Works with Sway, Hyprland, niri, river, labwc and
Wayfire, anything that implements wlr-output-management.

## X11 (`x11.rs`, `edid.rs`)

`xrandr --current --props` to read (`--current` avoids re-probing, which can
make screens flicker), with identities decoded from the EDID blocks, and one
`xrandr --output A --pos XxY ... --primary` call to write.

## Windows (`ccd.rs`, `windows.rs`)

`QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS)` and `DisplayConfigGetDeviceInfo`
for names and EDID manufacturer and product ids, then `SetDisplayConfig` with
the source modes moved: `SDC_SAVE_TO_DATABASE` to persist, `SDC_VALIDATE` to
check. The primary display is the one at the origin. Clones share a source and
are listed as off.

## macOS (`quartz.rs`, `macos.rs`)

`CGGetActiveDisplayList` and `CGDisplayBounds` to read, names from
`NSScreen.localizedName` on the main thread, and
`CGBeginDisplayConfiguration`, `CGConfigureDisplayOrigin` and
`CGCompleteDisplayConfiguration` to write, permanently or for the session.
The main display is the one at the origin. Mirrored displays are listed as off.

## Adding a desktop

1. Record what the desktop's tool prints on a real session with two screens,
   and save it under `crates/core/tests/fixtures/<desktop>/` with serial
   numbers replaced.
2. Write `parse(output) -> State` and `apply_args(output, &Layout) -> Vec<String>`
   (or the API equivalent) as pure functions, with tests against the fixture:
   sizes in the compositor's units, rotation, scale, a switched-off screen,
   and `Error::Changed` when the layout names screens that are not on.
3. Implement `Backend`: `name`, `capabilities`, `query` and `apply`. Run
   external tools through `Runner` so tests can script them
   (`run::testing::Scripted`).
4. Add a detection rule in `backend/detect.rs` with a probe test, and an arm
   in `create`.
5. Add a row to the tables in the README and on the site.
6. Run it on the real desktop with `SCREEN_SIDE_REAL=1 cargo test -p screen-side-core --test real`
   and `screen-side doctor`, and say in the pull request what you ran it on.
