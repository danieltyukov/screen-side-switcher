//! A keyboard shortcut for `toggle`. GNOME gets a real custom keybinding
//! through gsettings; Sway, Hyprland and KDE get the exact line or setting
//! to add; on Windows, macOS and X11 the app registers it while it runs,
//! because only Wayland forbids applications from grabbing global keys.

use crate::backend::detect::MUTTER;
use crate::backend::Probe;
use crate::run::Runner;
use crate::Error;

pub const SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";
pub const CUSTOM: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
pub const GNOME_PATH: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/screen-side/";
pub const DEFAULT_GNOME_KEYS: &str = "<Super><Alt>s";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desktop {
    Gnome,
    Kde,
    Sway,
    Hyprland,
    OtherWayland,
    OtherX11,
    Windows,
    Macos,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Commands to run, program first.
    Run(Vec<Vec<String>>),
    /// What the person adds themselves.
    Instructions(String),
    /// The app registers the shortcut while it runs.
    AppManaged(String),
}

/// Which kind of desktop this is, for shortcuts. `SCREEN_SIDE_DESKTOP`
/// overrides it.
pub fn desktop(probe: &dyn Probe) -> Desktop {
    if let Some(name) = probe.var("SCREEN_SIDE_DESKTOP") {
        match name.trim().to_lowercase().as_str() {
            "gnome" => return Desktop::Gnome,
            "kde" => return Desktop::Kde,
            "sway" => return Desktop::Sway,
            "hyprland" => return Desktop::Hyprland,
            "wayland" => return Desktop::OtherWayland,
            "x11" => return Desktop::OtherX11,
            "windows" => return Desktop::Windows,
            "macos" => return Desktop::Macos,
            _ => {}
        }
    }
    match probe.os() {
        "windows" => return Desktop::Windows,
        "macos" => return Desktop::Macos,
        _ => {}
    }
    let current = probe
        .var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_uppercase();
    let has = |name: &str| current.split(':').any(|d| d == name);
    if probe.var("SWAYSOCK").is_some() || has("SWAY") {
        Desktop::Sway
    } else if probe.var("HYPRLAND_INSTANCE_SIGNATURE").is_some() || has("HYPRLAND") {
        Desktop::Hyprland
    } else if has("KDE") {
        Desktop::Kde
    } else if has("GNOME") || probe.bus_has_owner(MUTTER) {
        Desktop::Gnome
    } else if probe.var("WAYLAND_DISPLAY").is_some() {
        Desktop::OtherWayland
    } else {
        Desktop::OtherX11
    }
}

/// POSIX single quotes, which GNOME's command parser and every shell read
/// the same way.
pub fn shell_quote(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', r"'\''"))
}

/// A GVariant string literal, as gsettings expects for string keys.
pub fn gvariant_string(s: &str) -> String {
    format!("'{}'", s.replace('\\', r"\\").replace('\'', r"\'"))
}

fn parse_gvariant_string(s: &str) -> String {
    let s = s.trim();
    let inner = s
        .strip_prefix('\'')
        .and_then(|s| s.strip_suffix('\''))
        .unwrap_or(s);
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn command_line(command: &[String]) -> String {
    command
        .iter()
        .map(|a| shell_quote(a))
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_list(text: &str) -> Vec<String> {
    let text = text.trim();
    let text = text.strip_prefix("@as").unwrap_or(text);
    text.split('\'')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

fn format_list(items: &[String]) -> String {
    if items.is_empty() {
        return "@as []".into();
    }
    let quoted: Vec<String> = items.iter().map(|i| format!("'{i}'")).collect();
    format!("[{}]", quoted.join(", "))
}

/// `<Super><Alt>s` as people read it: `Super+Alt+S`.
pub fn human_keys(keys: &str) -> String {
    let mut parts = Vec::new();
    let mut rest = keys;
    while let Some(start) = rest.strip_prefix('<') {
        let Some((name, after)) = start.split_once('>') else {
            break;
        };
        parts.push(match name {
            "Primary" | "Control" | "Ctrl" => "Ctrl".to_string(),
            other => other.to_string(),
        });
        rest = after;
    }
    if !rest.is_empty() {
        parts.push(if rest.chars().count() == 1 {
            rest.to_uppercase()
        } else {
            rest.to_string()
        });
    }
    parts.join("+")
}

fn gsettings(args: &[&str]) -> Vec<String> {
    std::iter::once("gsettings")
        .chain(args.iter().copied())
        .map(str::to_string)
        .collect()
}

const APP_MANAGED: &str = "On this system the Screen Side app registers the shortcut while it runs. Open Screen Side, then Settings, Keyboard shortcut.";

pub fn plan_install(desktop: Desktop, command: &[String], keys: &str, existing: &str) -> Plan {
    let line = command_line(command);
    let custom = format!("{CUSTOM}:{GNOME_PATH}");
    match desktop {
        Desktop::Gnome => {
            let mut list = parse_list(existing);
            if !list.iter().any(|p| p == GNOME_PATH) {
                list.push(GNOME_PATH.to_string());
            }
            Plan::Run(vec![
                gsettings(&["set", SCHEMA, "custom-keybindings", &format_list(&list)]),
                gsettings(&["set", &custom, "name", &gvariant_string("Screen Side toggle")]),
                gsettings(&["set", &custom, "command", &gvariant_string(&line)]),
                gsettings(&["set", &custom, "binding", &gvariant_string(keys)]),
            ])
        }
        Desktop::Sway => Plan::Instructions(format!(
            "Add this line to your Sway config (~/.config/sway/config), then reload Sway:\n\n    bindsym Mod4+Mod1+s exec {line}\n"
        )),
        Desktop::Hyprland => Plan::Instructions(format!(
            "Add this line to ~/.config/hypr/hyprland.conf:\n\n    bind = SUPER ALT, S, exec, {line}\n"
        )),
        Desktop::Kde => Plan::Instructions(format!(
            "Open System Settings, Keyboard, Shortcuts, then Add New, Command or Script (on Plasma 5: Custom Shortcuts, Edit, New, Global Shortcut, Command/URL), and use this command:\n\n    {line}\n"
        )),
        Desktop::OtherWayland => Plan::Instructions(format!(
            "Add a keyboard shortcut in your desktop's settings or config that runs:\n\n    {line}\n"
        )),
        Desktop::OtherX11 | Desktop::Windows | Desktop::Macos => {
            Plan::AppManaged(APP_MANAGED.into())
        }
    }
}

pub fn plan_remove(desktop: Desktop, existing: &str) -> Plan {
    match desktop {
        Desktop::Gnome => {
            let list: Vec<String> = parse_list(existing)
                .into_iter()
                .filter(|p| p != GNOME_PATH)
                .collect();
            Plan::Run(vec![
                gsettings(&["set", SCHEMA, "custom-keybindings", &format_list(&list)]),
                gsettings(&["reset-recursively", &format!("{CUSTOM}:{GNOME_PATH}")]),
            ])
        }
        Desktop::OtherX11 | Desktop::Windows | Desktop::Macos => {
            Plan::AppManaged(APP_MANAGED.into())
        }
        _ => Plan::Instructions(
            "Remove the line or shortcut you added for Screen Side from your desktop's settings."
                .into(),
        ),
    }
}

fn run_plan(runner: &dyn Runner, plan: Plan, done: String) -> Result<String, Error> {
    match plan {
        Plan::Run(commands) => {
            for c in &commands {
                runner.run(&c[0], &c[1..])?;
            }
            Ok(done)
        }
        Plan::Instructions(text) | Plan::AppManaged(text) => Ok(text),
    }
}

fn existing_list(runner: &dyn Runner, desktop: Desktop) -> Result<String, Error> {
    if desktop != Desktop::Gnome {
        return Ok(String::new());
    }
    runner.run(
        "gsettings",
        &["get".into(), SCHEMA.into(), "custom-keybindings".into()],
    )
}

pub fn install(
    runner: &dyn Runner,
    desktop: Desktop,
    command: &[String],
    keys: &str,
) -> Result<String, Error> {
    let existing = existing_list(runner, desktop)?;
    let done = format!(
        "Shortcut set: {} runs {}. Change the keys in Settings, Keyboard, Custom Shortcuts.",
        human_keys(keys),
        command_line(command)
    );
    run_plan(
        runner,
        plan_install(desktop, command, keys, &existing),
        done,
    )
}

pub fn remove(runner: &dyn Runner, desktop: Desktop) -> Result<String, Error> {
    let existing = existing_list(runner, desktop)?;
    run_plan(
        runner,
        plan_remove(desktop, &existing),
        "Shortcut removed.".into(),
    )
}

pub fn show(runner: &dyn Runner, desktop: Desktop) -> Result<String, Error> {
    if desktop != Desktop::Gnome {
        return Ok(
            match plan_install(
                desktop,
                &["screen-side".into(), "toggle".into()],
                DEFAULT_GNOME_KEYS,
                "",
            ) {
                Plan::Instructions(t) | Plan::AppManaged(t) => t,
                Plan::Run(_) => String::new(),
            },
        );
    }
    let list = parse_list(&existing_list(runner, desktop)?);
    if !list.iter().any(|p| p == GNOME_PATH) {
        return Ok(
            "No Screen Side shortcut is set. Add one with: screen-side shortcut install".into(),
        );
    }
    let custom = format!("{CUSTOM}:{GNOME_PATH}");
    let get = |key: &str| {
        runner
            .run("gsettings", &["get".into(), custom.clone(), key.into()])
            .map(|v| parse_gvariant_string(&v))
    };
    Ok(format!(
        "{} runs {}.",
        human_keys(&get("binding")?),
        get("command")?
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::testing::Scripted;
    use std::collections::HashMap;

    struct EnvProbe {
        os: &'static str,
        vars: HashMap<&'static str, &'static str>,
        mutter: bool,
    }

    impl Probe for EnvProbe {
        fn os(&self) -> &str {
            self.os
        }
        fn var(&self, name: &str) -> Option<String> {
            self.vars.get(name).map(|v| v.to_string())
        }
        fn on_path(&self, _: &str) -> bool {
            false
        }
        fn bus_has_owner(&self, name: &str) -> bool {
            self.mutter && name == crate::backend::detect::MUTTER
        }
    }

    fn linux(vars: &[(&'static str, &'static str)], mutter: bool) -> EnvProbe {
        EnvProbe {
            os: "linux",
            vars: vars.iter().copied().collect(),
            mutter,
        }
    }

    fn cmd() -> Vec<String> {
        vec!["/usr/bin/screen-side".into(), "toggle".into()]
    }

    fn runs(plan: Plan) -> Vec<Vec<String>> {
        match plan {
            Plan::Run(commands) => commands,
            other => panic!("expected commands, got {other:?}"),
        }
    }

    #[test]
    fn desktop_detection() {
        assert_eq!(
            desktop(&linux(&[("XDG_CURRENT_DESKTOP", "ubuntu:GNOME")], false)),
            Desktop::Gnome
        );
        assert_eq!(desktop(&linux(&[], true)), Desktop::Gnome);
        assert_eq!(
            desktop(&linux(&[("XDG_CURRENT_DESKTOP", "KDE")], false)),
            Desktop::Kde
        );
        assert_eq!(
            desktop(&linux(&[("SWAYSOCK", "/run/sway.sock")], true)),
            Desktop::Sway
        );
        assert_eq!(
            desktop(&linux(&[("HYPRLAND_INSTANCE_SIGNATURE", "abc")], false)),
            Desktop::Hyprland
        );
        assert_eq!(
            desktop(&linux(&[("WAYLAND_DISPLAY", "wayland-1")], false)),
            Desktop::OtherWayland
        );
        assert_eq!(
            desktop(&linux(&[("DISPLAY", ":0")], false)),
            Desktop::OtherX11
        );
        assert_eq!(
            desktop(&linux(&[("SCREEN_SIDE_DESKTOP", "sway")], true)),
            Desktop::Sway,
            "the override beats the bus"
        );
        let mac = EnvProbe {
            os: "macos",
            vars: HashMap::new(),
            mutter: false,
        };
        assert_eq!(desktop(&mac), Desktop::Macos);
    }

    #[test]
    fn gnome_install_appends_to_the_list() {
        let commands = runs(plan_install(
            Desktop::Gnome,
            &cmd(),
            DEFAULT_GNOME_KEYS,
            "@as []",
        ));
        let custom = format!("{CUSTOM}:{GNOME_PATH}");
        assert_eq!(
            commands,
            vec![
                vec![
                    "gsettings".to_string(),
                    "set".into(),
                    SCHEMA.into(),
                    "custom-keybindings".into(),
                    format!("['{GNOME_PATH}']")
                ],
                vec![
                    "gsettings".to_string(),
                    "set".into(),
                    custom.clone(),
                    "name".into(),
                    "'Screen Side toggle'".into()
                ],
                vec![
                    "gsettings".to_string(),
                    "set".into(),
                    custom.clone(),
                    "command".into(),
                    r"'\'/usr/bin/screen-side\' \'toggle\''".into()
                ],
                vec![
                    "gsettings".to_string(),
                    "set".into(),
                    custom,
                    "binding".into(),
                    "'<Super><Alt>s'".into()
                ],
            ]
        );
    }

    #[test]
    fn gnome_install_keeps_existing_entries_once() {
        let existing = format!("['/org/x/custom0/', '{GNOME_PATH}']");
        let commands = runs(plan_install(
            Desktop::Gnome,
            &cmd(),
            DEFAULT_GNOME_KEYS,
            &existing,
        ));
        assert_eq!(
            commands[0][4],
            format!("['/org/x/custom0/', '{GNOME_PATH}']")
        );
    }

    #[test]
    fn quoting() {
        assert_eq!(
            shell_quote("/home/a b/Screen Side.AppImage"),
            "'/home/a b/Screen Side.AppImage'"
        );
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
        assert_eq!(gvariant_string(r"a'b\c"), r"'a\'b\\c'");
        let command = vec![
            "/home/a b/Screen Side.AppImage".to_string(),
            "--toggle".to_string(),
        ];
        let commands = runs(plan_install(
            Desktop::Gnome,
            &command,
            DEFAULT_GNOME_KEYS,
            "@as []",
        ));
        assert_eq!(
            commands[2][4],
            r"'\'/home/a b/Screen Side.AppImage\' \'--toggle\''"
        );
    }

    #[test]
    fn gnome_remove() {
        let existing = format!("['/org/x/custom0/', '{GNOME_PATH}']");
        let commands = runs(plan_remove(Desktop::Gnome, &existing));
        assert_eq!(commands[0][4], "['/org/x/custom0/']");
        assert_eq!(
            commands[1],
            [
                "gsettings",
                "reset-recursively",
                &format!("{CUSTOM}:{GNOME_PATH}")
            ]
        );
        let empty = runs(plan_remove(Desktop::Gnome, &format!("['{GNOME_PATH}']")));
        assert_eq!(empty[0][4], "@as []");
    }

    #[test]
    fn sway_hyprland_and_kde_print_what_to_add() {
        let text = |d| match plan_install(d, &cmd(), DEFAULT_GNOME_KEYS, "") {
            Plan::Instructions(t) => t,
            other => panic!("expected instructions, got {other:?}"),
        };
        assert!(text(Desktop::Sway)
            .contains("bindsym Mod4+Mod1+s exec '/usr/bin/screen-side' 'toggle'"));
        assert!(text(Desktop::Hyprland)
            .contains("bind = SUPER ALT, S, exec, '/usr/bin/screen-side' 'toggle'"));
        let kde = text(Desktop::Kde);
        assert!(kde.contains("System Settings") && kde.contains("'/usr/bin/screen-side' 'toggle'"));
        assert!(text(Desktop::OtherWayland).contains("'/usr/bin/screen-side' 'toggle'"));
    }

    #[test]
    fn windows_macos_and_x11_are_app_managed() {
        for d in [Desktop::Windows, Desktop::Macos, Desktop::OtherX11] {
            assert!(matches!(
                plan_install(d, &cmd(), DEFAULT_GNOME_KEYS, ""),
                Plan::AppManaged(_)
            ));
        }
    }

    #[test]
    fn human_keys_read_like_a_menu() {
        assert_eq!(human_keys("<Super><Alt>s"), "Super+Alt+S");
        assert_eq!(human_keys("<Primary><Shift>F12"), "Ctrl+Shift+F12");
    }

    #[test]
    fn install_runs_the_plan() {
        let runner = Scripted::new(vec![
            ("gsettings", Ok("@as []\n")),
            ("gsettings", Ok("")),
            ("gsettings", Ok("")),
            ("gsettings", Ok("")),
            ("gsettings", Ok("")),
        ]);
        let message = install(&runner, Desktop::Gnome, &cmd(), DEFAULT_GNOME_KEYS).unwrap();
        assert!(
            message.starts_with("Shortcut set: Super+Alt+S runs"),
            "{message}"
        );
        let calls = runner.calls();
        assert_eq!(calls[0], ["gsettings", "get", SCHEMA, "custom-keybindings"]);
        assert_eq!(calls.len(), 5);
    }

    #[test]
    fn show_reports_the_gnome_shortcut() {
        let runner = Scripted::new(vec![
            (
                "gsettings",
                Ok(&*Box::leak(format!("['{GNOME_PATH}']\n").into_boxed_str())),
            ),
            ("gsettings", Ok("'<Super><Alt>s'\n")),
            (
                "gsettings",
                Ok("'\\'/usr/bin/screen-side\\' \\'toggle\\''\n"),
            ),
        ]);
        let text = show(&runner, Desktop::Gnome).unwrap();
        assert!(text.contains("Super+Alt+S"), "{text}");
        let none = Scripted::new(vec![("gsettings", Ok("@as []\n"))]);
        assert!(show(&none, Desktop::Gnome)
            .unwrap()
            .contains("No Screen Side shortcut"));
    }
}
