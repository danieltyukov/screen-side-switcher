//! KDE Plasma (Wayland and X11) through kscreen-doctor, which ships with
//! libkscreen. KScreen saves every change itself, so there is no temporary
//! mode, and its JSON carries no EDID, so screens are known by connector.

use serde::Deserialize;

use super::{ApplyMode, Backend, Capabilities};
use crate::layout::Origin;
use crate::model::{Identity, Layout, Rect, Screen, State};
use crate::run::Runner;
use crate::Error;

/// libkscreen's `Output::Panel`.
const PANEL: i64 = 7;
/// libkscreen rotation flags that turn the screen on its side.
const ROTATION_LEFT: i64 = 2;
const ROTATION_RIGHT: i64 = 8;

#[derive(Deserialize)]
struct Config {
    outputs: Vec<Output>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Output {
    name: String,
    #[serde(rename = "type", default)]
    kind: i64,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    connected: bool,
    priority: Option<u32>,
    primary: Option<bool>,
    #[serde(default)]
    pos: Point,
    #[serde(default = "one")]
    scale: f64,
    #[serde(default = "one_i")]
    rotation: i64,
    #[serde(default)]
    current_mode_id: String,
    #[serde(default)]
    replication_source: i64,
    #[serde(default)]
    modes: Vec<Mode>,
}

#[derive(Deserialize, Default)]
struct Point {
    x: i32,
    y: i32,
}

#[derive(Deserialize)]
struct Mode {
    id: String,
    size: Size,
}

#[derive(Deserialize)]
struct Size {
    width: i32,
    height: i32,
}

fn one() -> f64 {
    1.0
}

fn one_i() -> i64 {
    1
}

fn load(json: &str) -> Result<Config, Error> {
    serde_json::from_str(json).map_err(|e| Error::Tool {
        program: "kscreen-doctor".into(),
        message: format!("unexpected --json output ({e})"),
    })
}

fn builtin(o: &Output) -> bool {
    let c = o.name.to_uppercase();
    o.kind == PANEL || c.starts_with("EDP") || c.starts_with("LVDS") || c.starts_with("DSI")
}

fn to_state(config: &Config) -> State {
    let screens = config
        .outputs
        .iter()
        .filter(|o| o.connected)
        .map(|o| {
            let enabled = o.enabled && o.replication_source == 0;
            let rect = match o.modes.iter().find(|m| m.id == o.current_mode_id) {
                Some(m) if enabled => {
                    let (mut w, mut h) = (m.size.width, m.size.height);
                    if o.rotation == ROTATION_LEFT || o.rotation == ROTATION_RIGHT {
                        std::mem::swap(&mut w, &mut h);
                    }
                    let scale = if o.scale > 0.0 { o.scale } else { 1.0 };
                    // KWin rounds the scaled size.
                    Rect::new(
                        o.pos.x,
                        o.pos.y,
                        (w as f64 / scale).round() as i32,
                        (h as f64 / scale).round() as i32,
                    )
                }
                _ => Rect::default(),
            };
            Screen {
                id: o.name.clone(),
                connector: o.name.clone(),
                name: if builtin(o) {
                    "Built-in display".into()
                } else {
                    o.name.clone()
                },
                identity: Identity::default(),
                builtin: builtin(o),
                enabled,
                primary: enabled && (o.priority == Some(1) || o.primary == Some(true)),
                rect,
                scale: o.scale,
            }
        })
        .collect();
    State {
        backend: "kde".into(),
        screens,
    }
}

pub fn parse(json: &str) -> Result<State, Error> {
    Ok(to_state(&load(json)?))
}

/// The kscreen-doctor arguments that put the screens where `layout` says.
pub fn apply_args(json: &str, layout: &Layout) -> Result<Vec<String>, Error> {
    let config = load(json)?;
    let state = to_state(&config);
    let mut on: Vec<&str> = state.enabled().map(|s| s.id.as_str()).collect();
    let mut named: Vec<&str> = layout.positions.iter().map(|p| p.id.as_str()).collect();
    on.sort_unstable();
    named.sort_unstable();
    if on != named {
        return Err(Error::Changed);
    }
    // Plasma 6 orders outputs by priority; Plasma 5 has a primary flag.
    let plasma6 = config.outputs.iter().any(|o| o.priority.is_some());
    let mut args: Vec<String> = state
        .enabled()
        .filter_map(|s| layout.position(&s.id))
        .map(|p| format!("output.{}.position.{},{}", p.id, p.x, p.y))
        .collect();
    args.push(if plasma6 {
        format!("output.{}.priority.1", layout.primary)
    } else {
        format!("output.{}.primary", layout.primary)
    });
    Ok(args)
}

pub struct Kde {
    runner: Box<dyn Runner>,
}

impl Kde {
    pub fn new(runner: Box<dyn Runner>) -> Kde {
        Kde { runner }
    }

    fn json(&self) -> Result<String, Error> {
        self.runner.run("kscreen-doctor", &["--json".to_string()])
    }
}

impl Backend for Kde {
    fn name(&self) -> &'static str {
        "kde"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            primary: true,
            temporary: false,
            verify: false,
            remembers: true,
            confirms: false,
            origin: Origin::TopLeft,
        }
    }

    fn query(&self) -> Result<State, Error> {
        parse(&self.json()?)
    }

    fn apply(&self, layout: &Layout, _mode: ApplyMode) -> Result<(), Error> {
        let args = apply_args(&self.json()?, layout)?;
        self.runner.run("kscreen-doctor", &args).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{apply_checked, Applied};
    use crate::model::Position;
    use crate::run::testing::Scripted;

    const PLASMA6: &str = include_str!("../../tests/fixtures/kde/plasma6.json");
    const PLASMA5: &str = include_str!("../../tests/fixtures/kde/plasma5-rotated.json");

    fn layout(primary: &str, positions: &[(&str, i32, i32)]) -> Layout {
        Layout {
            positions: positions
                .iter()
                .map(|(id, x, y)| Position {
                    id: id.to_string(),
                    x: *x,
                    y: *y,
                })
                .collect(),
            primary: primary.into(),
        }
    }

    #[test]
    fn plasma6() {
        let state = parse(PLASMA6).unwrap();
        assert_eq!(
            state.screens.len(),
            2,
            "the disconnected output is left out"
        );
        let laptop = state.screen("eDP-1").unwrap();
        assert!(laptop.builtin && laptop.primary);
        assert_eq!(laptop.rect, Rect::new(2560, 0, 1707, 1067));
        assert_eq!(laptop.name, "Built-in display");
        let hdmi = state.screen("HDMI-A-1").unwrap();
        assert_eq!(hdmi.rect, Rect::new(0, 0, 2560, 1440));
        assert_eq!(hdmi.key(), "connector:hdmi-a-1");
        assert_eq!(hdmi.name, "HDMI-A-1");
        assert!(!hdmi.primary);
    }

    #[test]
    fn rotation_swaps_before_scaling() {
        let state = parse(PLASMA5).unwrap();
        let r = state.screen("DP-2").unwrap().rect;
        assert_eq!((r.width, r.height), (1080, 1920));
    }

    #[test]
    fn plasma5_primary_flag() {
        assert!(parse(PLASMA5).unwrap().screen("eDP-1").unwrap().primary);
    }

    #[test]
    fn apply_args_plasma6() {
        let args = apply_args(
            PLASMA6,
            &layout("eDP-1", &[("eDP-1", 2560, 0), ("HDMI-A-1", 0, 0)]),
        )
        .unwrap();
        assert_eq!(
            args,
            [
                "output.HDMI-A-1.position.0,0",
                "output.eDP-1.position.2560,0",
                "output.eDP-1.priority.1"
            ]
        );
    }

    #[test]
    fn apply_args_plasma5() {
        let args = apply_args(
            PLASMA5,
            &layout("DP-2", &[("eDP-1", 1080, 0), ("DP-2", 0, 0)]),
        )
        .unwrap();
        assert_eq!(args.last().unwrap(), "output.DP-2.primary");
    }

    #[test]
    fn apply_args_changed() {
        assert!(matches!(
            apply_args(
                PLASMA6,
                &layout("eDP-1", &[("eDP-1", 0, 0), ("DP-9", 0, 0)])
            ),
            Err(Error::Changed)
        ));
    }

    #[test]
    fn backend_runs_kscreen_doctor() {
        let runner = Scripted::new(vec![
            ("kscreen-doctor", Ok(PLASMA6)),
            ("kscreen-doctor", Ok(PLASMA6)),
            ("kscreen-doctor", Ok("")),
        ]);
        let kde = Kde::new(Box::new(runner));
        assert_eq!(kde.query().unwrap().screens.len(), 2);
        let l = layout("eDP-1", &[("eDP-1", 2560, 0), ("HDMI-A-1", 0, 0)]);
        assert_eq!(
            apply_checked(&kde, &l, ApplyMode::Verify).unwrap(),
            Applied::Computed
        );
        kde.apply(&l, ApplyMode::Persistent).unwrap();
        let caps = kde.capabilities();
        assert!(caps.primary && !caps.temporary && !caps.verify && caps.remembers);
    }

    #[test]
    fn recorded_command_lines() {
        let runner = std::sync::Arc::new(Scripted::new(vec![
            ("kscreen-doctor", Ok(PLASMA6)),
            ("kscreen-doctor", Ok("")),
        ]));
        struct Shared(std::sync::Arc<Scripted>);
        impl Runner for Shared {
            fn run(&self, program: &str, args: &[String]) -> Result<String, Error> {
                self.0.run(program, args)
            }
        }
        let kde = Kde::new(Box::new(Shared(runner.clone())));
        kde.apply(
            &layout("eDP-1", &[("eDP-1", 2560, 0), ("HDMI-A-1", 0, 0)]),
            ApplyMode::Persistent,
        )
        .unwrap();
        let calls = runner.calls();
        assert_eq!(calls[0], ["kscreen-doctor", "--json"]);
        assert_eq!(calls[1][0], "kscreen-doctor");
        assert_eq!(calls[1][1], "output.HDMI-A-1.position.0,0");
    }

    #[test]
    fn bad_json_is_a_tool_error_naming_kscreen_doctor() {
        assert!(matches!(
            parse("not json"),
            Err(Error::Tool { ref program, .. }) if program == "kscreen-doctor"
        ));
    }
}
