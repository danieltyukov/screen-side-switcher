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
    pub const ALL: [Kind; 7] = [
        Kind::Fake,
        Kind::Gnome,
        Kind::Kde,
        Kind::Wlroots,
        Kind::X11,
        Kind::Windows,
        Kind::Macos,
    ];

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
        let wanted = s.trim().to_lowercase();
        Kind::ALL
            .into_iter()
            .find(|k| k.as_str() == wanted)
            .ok_or_else(|| {
                Error::Usage(format!(
                    "SCREEN_SIDE_BACKEND={s} is not a backend. Use one of: fake, gnome, kde, wlroots, x11, windows, macos."
                ))
            })
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
    let pick = |kind: Kind, reason: &str| Choice {
        kind,
        reason: reason.to_string(),
        bus_name: None,
    };
    if let Some(name) = probe.var("SCREEN_SIDE_BACKEND") {
        let kind: Kind = name.parse()?;
        let bus_name = (kind == Kind::Gnome).then(|| {
            if probe.bus_has_owner(MUFFIN) && !probe.bus_has_owner(MUTTER) {
                MUFFIN
            } else {
                MUTTER
            }
            .to_string()
        });
        return Ok(Choice {
            kind,
            reason: "chosen by SCREEN_SIDE_BACKEND".into(),
            bus_name,
        });
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
    let desktop = probe
        .var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_uppercase();
    let wayland = probe.var("WAYLAND_DISPLAY").is_some();
    let x11 = !wayland && probe.var("DISPLAY").is_some();
    if desktop.split(':').any(|d| d == "KDE") {
        if probe.on_path("kscreen-doctor") {
            return Ok(pick(Kind::Kde, "KDE Plasma with kscreen-doctor"));
        }
        return Err(Error::NoBackend(
            "This is KDE Plasma, but kscreen-doctor is not installed. Install the libkscreen package, which provides it.".into(),
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
        return Err(Error::NoBackend(
            "This is an X11 session without xrandr. Install xrandr (x11-xserver-utils on Debian and Ubuntu).".into(),
        ));
    }
    Err(Error::NoBackend(
        "There is no graphical session here (neither WAYLAND_DISPLAY nor DISPLAY is set), so there are no screens to arrange.".into(),
    ))
}

pub fn create(choice: &Choice) -> Result<Box<dyn Backend>, Error> {
    match choice.kind {
        Kind::Fake => Ok(Box::new(super::fake::Fake::from_env()?)),
        #[cfg(all(unix, not(target_os = "macos")))]
        Kind::Gnome => Ok(Box::new(super::gnome::Gnome::connect(
            choice.bus_name.as_deref().unwrap_or(MUTTER),
        )?)),
        #[cfg(all(unix, not(target_os = "macos")))]
        Kind::Kde => Ok(Box::new(super::kde::Kde::new(Box::new(
            crate::run::SystemRunner,
        )))),
        #[cfg(all(unix, not(target_os = "macos")))]
        Kind::Wlroots => Ok(Box::new(super::wlroots::Wlroots::new(Box::new(
            crate::run::SystemRunner,
        )))),
        #[cfg(all(unix, not(target_os = "macos")))]
        Kind::X11 => Ok(Box::new(super::x11::X11::new(Box::new(
            crate::run::SystemRunner,
        )))),
        #[allow(unreachable_patterns)]
        other => Err(Error::Unsupported(format!(
            "The {} backend is not available on this operating system.",
            other.as_str()
        ))),
    }
}

pub fn detect() -> Result<(Box<dyn Backend>, Choice), Error> {
    let choice = choose(&SystemProbe)?;
    Ok((create(&choice)?, choice))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct FakeProbe {
        os: &'static str,
        vars: HashMap<&'static str, &'static str>,
        path: Vec<&'static str>,
        bus: Vec<&'static str>,
    }

    impl Probe for FakeProbe {
        fn os(&self) -> &str {
            self.os
        }
        fn var(&self, name: &str) -> Option<String> {
            self.vars.get(name).map(|v| v.to_string())
        }
        fn on_path(&self, program: &str) -> bool {
            self.path.contains(&program)
        }
        fn bus_has_owner(&self, name: &str) -> bool {
            self.bus.contains(&name)
        }
    }

    fn linux(
        vars: &[(&'static str, &'static str)],
        path: &[&'static str],
        bus: &[&'static str],
    ) -> FakeProbe {
        FakeProbe {
            os: "linux",
            vars: vars.iter().copied().collect(),
            path: path.to_vec(),
            bus: bus.to_vec(),
        }
    }

    fn no_backend(probe: &FakeProbe) -> String {
        match choose(probe) {
            Err(Error::NoBackend(message)) => message,
            other => panic!("expected NoBackend, got {other:?}"),
        }
    }

    #[test]
    fn the_override_wins() {
        let p = linux(&[("SCREEN_SIDE_BACKEND", "kde")], &["wlr-randr"], &[MUTTER]);
        assert_eq!(choose(&p).unwrap().kind, Kind::Kde);
        let bad = linux(&[("SCREEN_SIDE_BACKEND", "nope")], &[], &[]);
        assert!(matches!(choose(&bad), Err(Error::Usage(_))));
    }

    #[test]
    fn windows_and_macos_by_os() {
        let w = FakeProbe {
            os: "windows",
            ..Default::default()
        };
        assert_eq!(choose(&w).unwrap().kind, Kind::Windows);
        let m = FakeProbe {
            os: "macos",
            ..Default::default()
        };
        assert_eq!(choose(&m).unwrap().kind, Kind::Macos);
    }

    #[test]
    fn mutter_on_the_bus_means_gnome() {
        let c = choose(&linux(
            &[("WAYLAND_DISPLAY", "wayland-0")],
            &["wlr-randr"],
            &[MUTTER],
        ))
        .unwrap();
        assert_eq!(c.kind, Kind::Gnome);
        assert_eq!(c.bus_name.as_deref(), Some(MUTTER));
        let cinnamon = choose(&linux(&[("DISPLAY", ":0")], &["xrandr"], &[MUFFIN])).unwrap();
        assert_eq!(cinnamon.kind, Kind::Gnome);
        assert_eq!(cinnamon.bus_name.as_deref(), Some(MUFFIN));
    }

    #[test]
    fn kde_needs_kscreen_doctor() {
        let ok = linux(
            &[
                ("XDG_CURRENT_DESKTOP", "KDE"),
                ("WAYLAND_DISPLAY", "wayland-0"),
            ],
            &["kscreen-doctor"],
            &[],
        );
        assert_eq!(choose(&ok).unwrap().kind, Kind::Kde);
        let missing = linux(
            &[
                ("XDG_CURRENT_DESKTOP", "KDE"),
                ("WAYLAND_DISPLAY", "wayland-0"),
            ],
            &[],
            &[],
        );
        assert!(no_backend(&missing).contains("kscreen-doctor"));
    }

    #[test]
    fn other_wayland_needs_wlr_randr() {
        let ok = linux(
            &[
                ("WAYLAND_DISPLAY", "wayland-1"),
                ("XDG_CURRENT_DESKTOP", "sway"),
            ],
            &["wlr-randr"],
            &[],
        );
        assert_eq!(choose(&ok).unwrap().kind, Kind::Wlroots);
        let missing = linux(&[("WAYLAND_DISPLAY", "wayland-1")], &[], &[]);
        assert!(no_backend(&missing).contains("wlr-randr"));
    }

    #[test]
    fn x11_needs_xrandr_and_no_wayland() {
        let ok = linux(&[("DISPLAY", ":0")], &["xrandr"], &[]);
        assert_eq!(choose(&ok).unwrap().kind, Kind::X11);
        // XWayland: xrandr exists but only sees the X11 view of a Wayland session.
        let xwayland = linux(
            &[("DISPLAY", ":0"), ("WAYLAND_DISPLAY", "wayland-0")],
            &["xrandr"],
            &[],
        );
        assert!(no_backend(&xwayland).contains("wlr-randr"));
    }

    #[test]
    fn nothing_graphical() {
        assert!(no_backend(&linux(&[], &[], &[])).contains("no graphical session"));
    }

    #[test]
    fn creating_the_fake_works_everywhere() {
        let backend = create(&Choice {
            kind: Kind::Fake,
            reason: String::new(),
            bus_name: None,
        })
        .unwrap();
        assert_eq!(backend.name(), "fake");
    }
}
