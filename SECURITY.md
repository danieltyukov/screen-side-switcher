# Security policy

Screen Side changes your display layout, writes a few small files in your
config directory, and on some systems registers a keyboard shortcut or a
login item. Problems that let it do any of that for someone else, or do more
than that, matter most.

## Reporting a vulnerability

Please do not open a public issue for a security problem.

Report it privately through GitHub security advisories: the repository's
Security tab, "Report a vulnerability", or
<https://github.com/danieltyukov/screen-side-switcher/security/advisories/new>.
Include the version (`screen-side --version`), the operating system and
desktop, how it was installed, the steps to reproduce, and what an attacker
could gain. A draft pull request on the advisory is welcome.

## Scope

In scope:

- The installers and install scripts: `site/public/install.sh`,
  `site/public/install.ps1`, the `.deb`, `.rpm`, AppImage, NSIS, MSI and
  disk image builds.
- The release workflow and anything that decides what ends up in a release.
- Commands Screen Side writes for other programs to run: the GNOME custom
  keybinding set through `gsettings`, the autostart entry, and the lines it
  prints for Sway, Hyprland and KDE.
- Reading `layouts.json` and `settings.json` in the config directory
  (`~/.config/screenside`, `~/Library/Application Support/io.github.danieltyukov.ScreenSide`,
  `%APPDATA%\danieltyukov\ScreenSide\config`).
- The external programs it runs: `kscreen-doctor`, `wlr-randr`, `xrandr` and
  `gsettings`.

Out of scope:

- Problems in the desktops and display systems themselves; report those
  upstream.
- Attacks that need the user's account already.
- The unsigned installers asking for confirmation on first run; that is a
  known trade-off, explained in the README.

## What to expect

An acknowledgement within seven days. The project is maintained by one person
in their spare time; if you hear nothing after two weeks, comment on the
advisory. Fixes ship in a patch release, described in `CHANGELOG.md` and the
advisory. Reporters are credited unless they ask otherwise.

## Supported versions

Only the latest release receives fixes.
