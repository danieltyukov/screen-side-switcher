# Screen Side

Pick which side of your laptop screen an external monitor sits on, from a small
GTK4 app or one shell command. The point is the pointer: if the monitor is
placed on the left, the pointer leaves the left edge of the laptop screen and
arrives at the right edge of the external screen.

GNOME Settings can already do this by dragging boxes around. This exists because
dragging is slow when you switch desks often, and because the drag has to be
redone every time you sit at a different setup.

Works on Wayland and on X11, because it talks to Mutter over D-Bus rather than
to xrandr.

## Install

```
git clone https://github.com/danieltyukov/screen-side-switcher.git
cd screen-side-switcher
./install.sh
```

That installs into `~/.local`, and "Screen Side" then appears in your
applications list. Use `./install.sh --system` to install into `/usr/local` for
every user, and `./uninstall.sh` to remove it.

Requirements: GNOME 42 or newer, Python 3.10 or newer, and the GTK4 and
libadwaita bindings.

```
sudo apt install python3-gi gir1.2-gtk-4.0 gir1.2-adw-1
```

## Using it

Open Screen Side from the applications list, then click a side. The change is
applied immediately and saved, so it survives unplugging and reconnecting the
monitor.

The alignment control decides how the screens line up on the other axis. It
matters when the two screens are different heights: with top alignment the
pointer can only cross the part of the edge where they overlap, so centring
usually feels better.

## From the command line

```
screen-side                 show the current arrangement
screen-side left            put the external screen on the left
screen-side right           put it on the right
screen-side above           put it above
screen-side below           put it below
screen-side toggle          flip between left and right
screen-side left --align start
screen-side right --temporary
```

`toggle` is meant for a keyboard shortcut. In Settings, under Keyboard,
Custom Shortcuts, add a shortcut running:

```
/home/YOUR_USER/.local/bin/screen-side toggle
```

## How it works

Monitor layout on Wayland is owned by the compositor, so `xrandr` cannot change
it. The supported route is the `org.gnome.Mutter.DisplayConfig` D-Bus
interface: `GetCurrentState` reports the connected monitors, their modes and
the current logical layout, and `ApplyMonitorsConfig` sets a new one.

Two details cause most of the trouble when writing against that interface.

The first is the global `layout-mode` property. When it is `logical`, monitor
positions are given in scaled pixels, so a 3840 pixel wide panel at scale 2
occupies 1920 units. When it is `physical`, positions are in raw device pixels
and the same panel occupies 3840. Assuming the wrong one leaves a gap or an
overlap between the screens, and the pointer then either refuses to cross or
jumps. `screenside/mutter.py` reads the property and computes extents to match.

The second is that Mutter rejects overlapping monitors outright, and a layout
where two screens meet only at a corner gives the pointer nowhere to cross.
`screenside/layout.py` therefore computes exact adjacency along the chosen axis
and a deliberate alignment on the other one.

## Layout

```
screenside/mutter.py   D-Bus wrapper, state model, apply
screenside/layout.py   turns "left" into absolute coordinates
screenside/cli.py      command line
screenside/app.py      GTK4 and libadwaita interface
data/                  desktop entry and icon
```

## Licence

MIT. See LICENSE.
