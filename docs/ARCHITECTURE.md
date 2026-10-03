# Architecture

Screen Side has one job: put screens where they stand on the desk, so the
pointer crosses the edge that faces the other screen. Everything that decides
where screens go lives in `crates/core`; the command line, the app, the tray
and the watcher are thin layers that call it.

## From a click to a layout

1. **Detect the backend** (`backend/detect.rs`). Windows and macOS have one
   each. On Linux, Mutter's bus name means GNOME (or Cinnamon's Muffin);
   `XDG_CURRENT_DESKTOP=KDE` means KDE; any other Wayland session needs
   `wlr-randr`; plain X11 needs `xrandr`. `SCREEN_SIDE_BACKEND` overrides it.
2. **Query** the backend for a `State`: every connected screen with its
   identity, whether it is on, whether it is primary, and its rectangle in the
   backend's own units.
3. **Read the arrangement in force** (`layout::infer`): which side of the
   anchor (the built-in screen, else the primary) each other screen is on,
   nearest first, and how they line up.
4. **Change the intent**, not the coordinates: move one screen or all of
   them, toggle, align, set the primary.
5. **Compute** a `Layout` (`layout::compute`): the anchor at the origin,
   each side a chain going outward, every screen aligned against the anchor.
   Overlaps are refused, never nudged into a gap the pointer cannot cross.
6. **Normalise** by the backend's origin rule and **apply**.

## Coordinates

Each backend fills rectangles in the units its system positions screens in,
and converts sizes the way its compositor does, or adjacent screens end up a
pixel apart and the pointer stops at the seam.

| Backend | Units | Size of a screen |
|---|---|---|
| GNOME | logical or physical pixels, from Mutter's `layout-mode` | current mode, swapped for 90 and 270 degree transforms, divided by the scale and rounded in logical mode |
| KDE | logical pixels | current mode, swapped for rotation 2 or 8, divided by the scale and rounded |
| wlroots | logical pixels | current mode, swapped for 90 and 270, divided by the scale and truncated, as wlroots does |
| X11 | pixels | the CRTC size xrandr prints, already rotated |
| Windows | physical desktop pixels | the source mode size |
| macOS | points | `CGDisplayBounds` |

**Origin.** GNOME, KDE, wlroots and X11 want the layout to start at (0, 0).
Windows and macOS define the primary (main) display as the one at (0, 0), so
there the layout is shifted until the primary is at the origin, and setting
the primary is that shift.

**Centring** puts a screen at `floor((anchor - screen) / 2)` on the cross
axis. Reading an arrangement back allows one pixel of difference, so layouts
made by Screen Side 1.0, which centred against the taller screen, read as
centred.

**Toggle** mirrors every screen. When screens sit on both axes the mirror is
a point reflection, so start and end alignment swap too; otherwise a top
aligned screen on the left and one above would meet in the corner once
mirrored.

## Saved, temporary and unattended changes

A change from the window or the command line is saved by the system, so it
comes back after a reconnect. GNOME asks the person on screen to keep a saved
change and reverts it after 20 seconds without an answer (the backend's
`confirms` capability). Nobody answers that prompt for the watcher, the tray,
a hotkey or `screen-side-gui --toggle`, so those use `backend::unattended`,
which makes a temporary change where saving would ask. Saved layouts marked
Auto bring the arrangement back after a reconnect instead.

## Saved layouts

A layout is saved as intent: the anchor, each screen's side in order, the
alignment and the primary, against the identity keys of the screens that were
on (`vendor:product:serial`, or `connector:NAME` where a backend reports no
EDID). Applying it recomputes coordinates, so a layout survives a change of
resolution or scale. Two identical monitors without serial numbers are told
apart by connector order (`key`, `key#2`).

Files live in the config directory (`SCREEN_SIDE_CONFIG_DIR` overrides it):
`layouts.json` and `settings.json`, each with a `version`. Writes go to a
temporary file and are renamed into place. A file that cannot be read, or was
written by a newer version, is refused with its path and left untouched.

## The watcher

`watch::Watcher::tick` queries the screens every two seconds. Only a change
in the set of screens that are on can apply a saved layout, never a change of
position, so the watcher's own apply cannot trigger it again. It waits a
second for new screens to settle, applies the matching layout marked Auto,
and skips the apply when the screens are already where the layout puts them.
Errors are reported and the loop carries on. `screen-side watch` and the app's
background thread both run it.

## The app

The window only knows `app/src/backend/types.ts`. Under Tauri that seam is
`tauri.ts`, which calls the commands in `app/src-tauri/src/commands.rs`; in a
browser it is `mock.ts`, which is how the interface is developed, unit tested
and screenshotted without a display backend. Commands are synchronous, so
Tauri runs them on the main thread, which is where macOS hands out screen
names.

`screen-side-gui --toggle` and `--apply NAME` do their work without opening a
window and exit; desktop shortcuts run these.

## JSON output

Every JSON document carries `"schema": 1`. A change that removes or renames a
field bumps it.

`screen-side status --json`:

```json
{
  "schema": 1,
  "backend": "gnome",
  "capabilities": { "primary": true, "temporary": true, "verify": true, "remembers": true, "origin": "top_left" },
  "screens": [
    { "number": 1, "id": "HDMI-1", "connector": "HDMI-1", "name": "DELL U2723QE", "builtin": false,
      "enabled": true, "primary": false, "x": 0, "y": 0, "width": 2560, "height": 1440, "scale": 1.0 }
  ],
  "arrangement": { "anchor": "eDP-1", "placements": [{ "screen": "HDMI-1", "side": "left" }],
                   "align": "center", "aligned": true, "primary": "eDP-1" },
  "layout": "office"
}
```

`arrangement` is null when the screens are not in a side-by-side layout,
`layout` when no saved layout is in force. A `--dry-run` with `--json` prints
`{ "schema": 1, "dry_run": true, "checked": true, "positions": [...], "primary": "..." }`.

`screen-side layouts --json` prints `{ "schema": 1, "layouts": [{ "name",
"auto", "matches", "screens", "summary", "saved" }] }`, with `matches` null
when no backend could be reached.

`screen-side doctor --json` prints the same report as the text, with serial
numbers reduced to `"present"` or `"absent"`.
