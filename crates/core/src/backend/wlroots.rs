//! wlroots compositors (Sway, Hyprland, niri, river, labwc, Wayfire) through
//! wlr-randr, which speaks wlr-output-management. Nothing persists: the
//! compositor forgets on reconnect, which saved layouts and `watch` cover.

use serde::Deserialize;

use super::{ApplyMode, Backend, Capabilities};
use crate::layout::Origin;
use crate::model::{Identity, Layout, Rect, Screen, State};
use crate::run::Runner;
use crate::Error;

#[derive(Deserialize)]
struct Head {
    name: String,
    make: Option<String>,
    model: Option<String>,
    serial: Option<String>,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    modes: Vec<Mode>,
    position: Option<Point>,
    transform: Option<String>,
    scale: Option<f64>,
}

#[derive(Deserialize)]
struct Mode {
    width: i32,
    height: i32,
    #[serde(default)]
    current: bool,
}

#[derive(Deserialize)]
struct Point {
    x: i32,
    y: i32,
}

fn heads(json: &str) -> Result<Vec<Head>, Error> {
    serde_json::from_str(json).map_err(|e| Error::Tool {
        program: "wlr-randr".into(),
        message: format!("unexpected --json output ({e}); wlr-randr 0.3 or newer is needed"),
    })
}

fn to_screen(h: Head) -> Screen {
    let scale = h.scale.filter(|s| *s > 0.0).unwrap_or(1.0);
    let rect = match (
        h.enabled,
        h.position.as_ref(),
        h.modes.iter().find(|m| m.current),
    ) {
        (true, Some(p), Some(m)) => {
            let (mut w, mut ht) = (m.width, m.height);
            if matches!(
                h.transform.as_deref(),
                Some("90" | "270" | "flipped-90" | "flipped-270")
            ) {
                std::mem::swap(&mut w, &mut ht);
            }
            // wlroots truncates (wlr_output_effective_resolution).
            Rect::new(
                p.x,
                p.y,
                (w as f64 / scale) as i32,
                (ht as f64 / scale) as i32,
            )
        }
        _ => Rect::default(),
    };
    let upper = h.name.to_uppercase();
    let builtin = upper.starts_with("EDP") || upper.starts_with("LVDS") || upper.starts_with("DSI");
    let make = h.make.unwrap_or_default();
    let model = h.model.unwrap_or_default();
    let name = if builtin {
        "Built-in display".to_string()
    } else {
        let joined = format!("{make} {model}").trim().to_string();
        if joined.is_empty() {
            h.name.clone()
        } else {
            joined
        }
    };
    Screen {
        id: h.name.clone(),
        connector: h.name,
        name,
        identity: Identity {
            vendor: make,
            product: model,
            serial: h.serial.unwrap_or_default(),
        },
        builtin,
        enabled: h.enabled && rect.width > 0,
        primary: false,
        rect,
        scale,
    }
}

pub fn parse(json: &str) -> Result<State, Error> {
    Ok(State {
        backend: "wlroots".into(),
        screens: heads(json)?.into_iter().map(to_screen).collect(),
    })
}

/// The wlr-randr arguments for `layout`, as one atomic configuration.
pub fn apply_args(json: &str, layout: &Layout, verify: bool) -> Result<Vec<String>, Error> {
    let state = parse(json)?;
    super::ensure_same_screens(&state, layout)?;
    let mut args = Vec::new();
    for s in state.enabled() {
        let p = layout.position(&s.id).ok_or(Error::Changed)?;
        args.extend([
            "--output".to_string(),
            s.id.clone(),
            "--pos".to_string(),
            format!("{},{}", p.x, p.y),
        ]);
    }
    if verify {
        args.push("--dryrun".into());
    }
    Ok(args)
}

pub struct Wlroots {
    runner: Box<dyn Runner>,
}

impl Wlroots {
    pub fn new(runner: Box<dyn Runner>) -> Wlroots {
        Wlroots { runner }
    }

    fn json(&self) -> Result<String, Error> {
        self.runner.run("wlr-randr", &["--json".to_string()])
    }
}

impl Backend for Wlroots {
    fn name(&self) -> &'static str {
        "wlroots"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            primary: false,
            temporary: false,
            verify: true,
            remembers: false,
            confirms: false,
            origin: Origin::TopLeft,
        }
    }

    fn query(&self) -> Result<State, Error> {
        parse(&self.json()?)
    }

    fn apply(&self, layout: &Layout, mode: ApplyMode) -> Result<(), Error> {
        let args = apply_args(&self.json()?, layout, mode == ApplyMode::Verify)?;
        self.runner.run("wlr-randr", &args).map(|_| ())
    }

    fn diagnostics(&self) -> Vec<(String, String)> {
        ["SWAYSOCK", "HYPRLAND_INSTANCE_SIGNATURE", "NIRI_SOCKET"]
            .iter()
            .filter(|v| std::env::var_os(v).is_some())
            .map(|v| ("compositor socket".to_string(), v.to_string()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{apply_checked, Applied};
    use crate::model::Position;
    use crate::run::testing::Scripted;
    use std::sync::Arc;

    const SWAY: &str = include_str!("../../tests/fixtures/wlroots/sway.json");
    const ROTATED: &str = include_str!("../../tests/fixtures/wlroots/rotated.json");

    fn layout(positions: &[(&str, i32, i32)]) -> Layout {
        Layout {
            positions: positions
                .iter()
                .map(|(id, x, y)| Position {
                    id: id.to_string(),
                    x: *x,
                    y: *y,
                })
                .collect(),
            primary: "eDP-1".into(),
        }
    }

    struct Shared(Arc<Scripted>);
    impl Runner for Shared {
        fn run(&self, program: &str, args: &[String]) -> Result<String, Error> {
            self.0.run(program, args)
        }
    }

    #[test]
    fn sizes_truncate_like_wlroots() {
        let state = parse(SWAY).unwrap();
        let laptop = state.screen("eDP-1").unwrap();
        assert_eq!(laptop.rect, Rect::new(0, 0, 1706, 1066));
        assert!(laptop.builtin);
        assert_eq!(laptop.key(), "boe:0x095f:");
        let dell = state.screen("DP-1").unwrap();
        assert_eq!(dell.rect, Rect::new(1706, 0, 2560, 1440));
        assert_eq!(dell.name, "Dell Inc. DELL U2723QE");
        assert_eq!(dell.key(), "dell inc.:dell u2723qe:abc123");
        let off = state.screen("HDMI-A-1").unwrap();
        assert!(!off.enabled);
    }

    #[test]
    fn transform_swaps() {
        let r = parse(ROTATED).unwrap().screen("DP-2").unwrap().rect;
        assert_eq!((r.width, r.height), (1080, 1920));
    }

    #[test]
    fn nobody_is_primary() {
        assert!(parse(SWAY).unwrap().screens.iter().all(|s| !s.primary));
    }

    #[test]
    fn apply_args_in_one_configuration() {
        let l = layout(&[("eDP-1", 2560, 0), ("DP-1", 0, 0)]);
        assert_eq!(
            apply_args(SWAY, &l, false).unwrap(),
            ["--output", "eDP-1", "--pos", "2560,0", "--output", "DP-1", "--pos", "0,0"]
        );
        assert_eq!(
            apply_args(SWAY, &l, true).unwrap().last().unwrap(),
            "--dryrun"
        );
    }

    #[test]
    fn apply_args_changed() {
        assert!(matches!(
            apply_args(SWAY, &layout(&[("eDP-1", 0, 0)]), false),
            Err(Error::Changed)
        ));
    }

    #[test]
    fn verify_runs_a_dry_run() {
        let runner = Arc::new(Scripted::new(vec![
            ("wlr-randr", Ok(SWAY)),
            ("wlr-randr", Ok(SWAY)),
            ("wlr-randr", Ok("")),
        ]));
        let backend = Wlroots::new(Box::new(Shared(runner.clone())));
        assert_eq!(backend.query().unwrap().screens.len(), 3);
        let l = layout(&[("eDP-1", 2560, 0), ("DP-1", 0, 0)]);
        assert_eq!(
            apply_checked(&backend, &l, ApplyMode::Verify).unwrap(),
            Applied::Checked
        );
        let calls = runner.calls();
        assert_eq!(calls[0], ["wlr-randr", "--json"]);
        assert_eq!(calls[2].last().unwrap(), "--dryrun");
        let caps = backend.capabilities();
        assert!(!caps.primary && !caps.temporary && caps.verify && !caps.remembers);
    }

    #[test]
    fn old_wlr_randr_says_so() {
        assert!(matches!(
            parse("eDP-1 \"BOE 0x095F\"\n  Enabled: yes"),
            Err(Error::Tool { ref message, .. }) if message.contains("0.3")
        ));
    }
}
