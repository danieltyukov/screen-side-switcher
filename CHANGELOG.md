# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [2.0.0] - 2026-10-04

Screen Side now runs on Windows, macOS and the common Linux desktops, as an app with a tray icon and as a command line tool.

### Added

- Windows (display configuration API), macOS (Quartz Display Services), KDE Plasma (`kscreen-doctor`), wlroots desktops such as Sway, Hyprland, niri, river and labwc (`wlr-randr`), and other X11 desktops (`xrandr`), next to GNOME.
- An app built with Tauri: a drawing of the screens to scale, with the edge the pointer crosses marked; a tray menu; running in the background; starting at login.
- Several external screens: move one or all of them, with screens on the same side lined up outward in order. Arrangements whose screens would overlap are refused with an explanation.
- Saved layouts per set of screens, applied on demand or automatically when those screens connect (`save`, `apply`, `layouts`, `forget`, `watch`, and the Layouts tab).
- End alignment (bottom or right edges level) next to start and centre.
- Choosing the primary screen.
- Keyboard shortcut setup: a GNOME custom keybinding, the exact line for Sway, Hyprland and KDE, and a global shortcut registered by the app on Windows, macOS and X11.
- `screen-side doctor` for bug reports, and `--json` output for scripts.
- `--dry-run`, which on GNOME, Windows and wlroots asks the system to check the change without making it.
- Installers for every platform on each release, one-line installers for the command line tool, and a project site.

### Changed

- Rewritten in Rust (core and command line) and Tauri (the app). The GTK 4 window is replaced by the cross-platform app.
- `toggle` mirrors every screen: left with right and above with below.

### Removed

- The Python package, `install.sh` and `uninstall.sh`. The new `install.sh` removes a 1.0 install from `~/.local`.

## [1.0.0] - 2026-08-31

### Added

- A GTK 4 app and a command line tool for GNOME that put the external screen left, right, above or below the laptop screen through Mutter's DisplayConfig D-Bus interface, on Wayland and X11.
