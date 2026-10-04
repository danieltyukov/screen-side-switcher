//! `screen-side doctor`: everything a bug report needs, in one paste.
//! Serial numbers are reduced to "present" or "absent".

use std::fmt;

use serde::Serialize;

use crate::backend::{choose, create, Capabilities, Probe};
use crate::layout::Origin;
use crate::store::Store;

#[derive(Debug, Clone, Serialize)]
pub struct DoctorScreen {
    pub id: String,
    pub connector: String,
    pub name: String,
    pub vendor: String,
    pub product: String,
    pub serial: &'static str,
    pub builtin: bool,
    pub enabled: bool,
    pub primary: bool,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub schema: u32,
    pub version: String,
    pub os: String,
    pub arch: String,
    pub desktop: String,
    pub session: String,
    pub backend: Option<String>,
    pub reason: String,
    pub capabilities: Option<Capabilities>,
    pub details: Vec<(String, String)>,
    pub tools: Vec<(String, String)>,
    pub config_dir: Option<String>,
    pub layouts: Option<usize>,
    pub layouts_error: Option<String>,
    pub screens: Vec<DoctorScreen>,
    pub error: Option<String>,
}

/// Never fails: whatever goes wrong becomes part of the report.
pub fn report(probe: &dyn Probe, version: &str, store: Option<&Store>) -> Report {
    let mut r = Report {
        schema: 1,
        version: version.to_string(),
        os: probe.os().to_string(),
        arch: std::env::consts::ARCH.to_string(),
        desktop: probe.var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
        session: probe.var("XDG_SESSION_TYPE").unwrap_or_default(),
        backend: None,
        reason: String::new(),
        capabilities: None,
        details: Vec::new(),
        tools: Vec::new(),
        config_dir: store.map(|s| s.dir().display().to_string()),
        layouts: None,
        layouts_error: None,
        screens: Vec::new(),
        error: None,
    };
    if !matches!(probe.os(), "windows" | "macos") {
        for tool in ["kscreen-doctor", "wlr-randr", "xrandr", "gsettings"] {
            let found = if probe.on_path(tool) {
                "found"
            } else {
                "not found"
            };
            r.tools.push((tool.to_string(), found.to_string()));
        }
    }
    if let Some(store) = store {
        match store.layouts() {
            Ok(layouts) => r.layouts = Some(layouts.len()),
            Err(e) => r.layouts_error = Some(e.to_string()),
        }
    }
    let backend = choose(probe).and_then(|choice| {
        r.reason = choice.reason.clone();
        create(&choice)
    });
    let backend = match backend {
        Ok(b) => b,
        Err(e) => {
            r.error = Some(e.to_string());
            return r;
        }
    };
    r.backend = Some(backend.name().to_string());
    r.capabilities = Some(backend.capabilities());
    r.details = backend.diagnostics();
    match backend.query() {
        Ok(state) => {
            r.screens = state
                .listed()
                .iter()
                .map(|s| DoctorScreen {
                    id: s.id.clone(),
                    connector: s.connector.clone(),
                    name: s.name.clone(),
                    vendor: s.identity.vendor.clone(),
                    product: s.identity.product.clone(),
                    serial: if s.identity.serial.trim().is_empty() {
                        "absent"
                    } else {
                        "present"
                    },
                    builtin: s.builtin,
                    enabled: s.enabled,
                    primary: s.primary,
                    x: s.rect.x,
                    y: s.rect.y,
                    width: s.rect.width,
                    height: s.rect.height,
                    scale: s.scale,
                })
                .collect();
        }
        Err(e) => r.error = Some(e.to_string()),
    }
    r
}

fn yes(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Screen Side {}", self.version)?;
        writeln!(
            f,
            "System: {} {}, desktop {}, session {}",
            self.os,
            self.arch,
            if self.desktop.is_empty() {
                "unknown"
            } else {
                &self.desktop
            },
            if self.session.is_empty() {
                "unknown"
            } else {
                &self.session
            },
        )?;
        match &self.backend {
            Some(b) => writeln!(f, "Backend: {b} ({})", self.reason)?,
            None => writeln!(
                f,
                "No backend: {}",
                self.error.as_deref().unwrap_or("unknown")
            )?,
        }
        if let Some(c) = &self.capabilities {
            writeln!(
                f,
                "Capabilities: primary {}, temporary {}, check without applying {}, remembers layouts {}, asks to keep changes {}, origin {}",
                yes(c.primary),
                yes(c.temporary),
                yes(c.verify),
                yes(c.remembers),
                yes(c.confirms),
                match c.origin {
                    Origin::TopLeft => "top-left",
                    Origin::Primary => "primary screen",
                }
            )?;
        }
        if !self.details.is_empty() {
            let d: Vec<String> = self
                .details
                .iter()
                .map(|(k, v)| format!("{k} {v}"))
                .collect();
            writeln!(f, "Details: {}", d.join("; "))?;
        }
        if !self.tools.is_empty() {
            let t: Vec<String> = self.tools.iter().map(|(k, v)| format!("{k} {v}")).collect();
            writeln!(f, "Tools: {}", t.join(", "))?;
        }
        match (&self.config_dir, self.layouts, &self.layouts_error) {
            (Some(dir), Some(n), _) => writeln!(f, "Config: {dir} ({n} saved layouts)")?,
            (Some(dir), None, Some(e)) => writeln!(f, "Config: {dir} (layouts unreadable: {e})")?,
            (Some(dir), _, _) => writeln!(f, "Config: {dir}")?,
            (None, _, _) => writeln!(f, "Config: unavailable")?,
        }
        if self.backend.is_some() {
            if let Some(e) = &self.error {
                writeln!(f, "Problem: {e}")?;
            }
        }
        if !self.screens.is_empty() {
            writeln!(f, "Screens:")?;
            for s in &self.screens {
                let place = if s.enabled {
                    format!(
                        "{}x{} at {},{} scale {}{}",
                        s.width,
                        s.height,
                        s.x,
                        s.y,
                        s.scale,
                        if s.primary { ", primary" } else { "" }
                    )
                } else {
                    "off".to_string()
                };
                writeln!(
                    f,
                    "  {} ({}){}: vendor {}, product {}, serial {}; {place}",
                    s.connector,
                    s.name,
                    if s.builtin { ", built-in" } else { "" },
                    if s.vendor.is_empty() {
                        "unknown"
                    } else {
                        &s.vendor
                    },
                    if s.product.is_empty() {
                        "unknown"
                    } else {
                        &s.product
                    },
                    s.serial,
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::fake::FakeFile;
    use crate::backend::Backend;
    use crate::layout::baseline;
    use crate::store::capture;
    use std::collections::HashMap;

    struct P {
        os: &'static str,
        vars: HashMap<&'static str, &'static str>,
        path: Vec<&'static str>,
    }

    impl Probe for P {
        fn os(&self) -> &str {
            self.os
        }
        fn var(&self, name: &str) -> Option<String> {
            self.vars.get(name).map(|v| v.to_string())
        }
        fn on_path(&self, program: &str) -> bool {
            self.path.contains(&program)
        }
        fn bus_has_owner(&self, _: &str) -> bool {
            false
        }
    }

    fn fake_probe() -> P {
        P {
            os: "linux",
            vars: [
                ("SCREEN_SIDE_BACKEND", "fake"),
                ("XDG_CURRENT_DESKTOP", "ubuntu:GNOME"),
                ("XDG_SESSION_TYPE", "wayland"),
            ]
            .into_iter()
            .collect(),
            path: vec!["xrandr"],
        }
    }

    fn store_with_a_layout() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::at(dir.path());
        let state = crate::backend::fake::Fake::in_memory(FakeFile::sample())
            .query()
            .unwrap();
        store
            .put(capture("office", &state, &baseline(&state).unwrap(), false).unwrap())
            .unwrap();
        (dir, store)
    }

    #[test]
    fn reports_the_backend_screens_and_layouts() {
        let (_dir, store) = store_with_a_layout();
        let r = report(&fake_probe(), "2.0.0", Some(&store));
        assert_eq!(r.backend.as_deref(), Some("fake"));
        assert_eq!(r.layouts, Some(1));
        assert_eq!(r.screens.len(), 2);
        let text = r.to_string();
        for wanted in [
            "Screen Side 2.0.0",
            "Backend: fake",
            "Capabilities:",
            "serial present",
            "ubuntu:GNOME",
        ] {
            assert!(text.contains(wanted), "missing {wanted} in:\n{text}");
        }
        assert!(!text.contains("FAKE0001"));
    }

    #[test]
    fn json_has_a_schema_and_no_serials() {
        let r = report(&fake_probe(), "2.0.0", None);
        let json = serde_json::to_string(&r).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["schema"], 1);
        // Listed left to right: the monitor (with a serial), then the laptop.
        assert_eq!(v["screens"][0]["serial"], "present");
        assert_eq!(v["screens"][1]["serial"], "absent");
        assert!(!json.contains("FAKE0001"));
    }

    #[test]
    fn no_backend_is_reported_not_raised() {
        let probe = P {
            os: "linux",
            vars: HashMap::new(),
            path: vec![],
        };
        let r = report(&probe, "2.0.0", None);
        assert_eq!(r.backend, None);
        assert!(r.error.as_deref().unwrap().contains("no graphical session"));
        assert!(r.to_string().contains("No backend:"));
    }

    #[test]
    fn tools_are_listed_on_linux_only() {
        let r = report(&fake_probe(), "2.0.0", None);
        let names: Vec<&str> = r.tools.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            ["kscreen-doctor", "wlr-randr", "xrandr", "gsettings"]
        );
        assert_eq!(r.tools[2].1, "found");
        let windows = P {
            os: "windows",
            vars: [("SCREEN_SIDE_BACKEND", "fake")].into_iter().collect(),
            path: vec![],
        };
        assert!(report(&windows, "2.0.0", None).tools.is_empty());
    }
}
