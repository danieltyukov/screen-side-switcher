//! Plain X11 desktops (Xfce, MATE, i3, Openbox and the like) through
//! xrandr. `--current` reads without re-probing the outputs, which would
//! make the screens flicker on every status.

use super::{ApplyMode, Backend, Capabilities};
use crate::edid::parse_edid;
use crate::layout::Origin;
use crate::model::{Identity, Layout, Rect, Screen, State};
use crate::run::Runner;
use crate::Error;

/// `1920x1080+2560+180`, as xrandr prints a CRTC (already rotated).
fn geometry(token: &str) -> Option<Rect> {
    let (size, rest) = token.split_once('+')?;
    let (x, y) = rest.split_once('+')?;
    let (w, h) = size.split_once('x')?;
    Some(Rect::new(
        x.parse().ok()?,
        y.parse().ok()?,
        w.parse().ok()?,
        h.parse().ok()?,
    ))
}

fn decode_hex(hex: &str) -> Vec<u8> {
    (0..hex.len() / 2)
        .filter_map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok())
        .collect()
}

/// Gives the last screen the identity in the EDID collected for it.
fn finish_edid(screens: &mut [Screen], hex: &mut Option<String>) {
    if let (Some(hex), Some(last)) = (hex.take(), screens.last_mut()) {
        if let Some(e) = parse_edid(&decode_hex(&hex)) {
            last.identity = Identity {
                vendor: e.vendor,
                product: e.product,
                serial: e.serial,
            };
            if !last.builtin && !e.name.is_empty() {
                last.name = e.name;
            }
        }
    }
}

pub fn parse(text: &str) -> Result<State, Error> {
    let mut screens: Vec<Screen> = Vec::new();
    let mut edid_hex: Option<String> = None;
    for line in text.lines() {
        if let Some(hex) = edid_hex.as_mut() {
            if let Some(chunk) = line.strip_prefix("\t\t") {
                hex.push_str(chunk.trim());
                continue;
            }
            finish_edid(&mut screens, &mut edid_hex);
        }
        if line.starts_with("\tEDID:") {
            edid_hex = Some(String::new());
            continue;
        }
        if line.starts_with(char::is_whitespace) || line.starts_with("Screen ") {
            continue;
        }
        let mut words = line.split_whitespace();
        let (Some(name), Some(status)) = (words.next(), words.next()) else {
            continue;
        };
        if status != "connected" {
            continue;
        }
        let rest: Vec<&str> = words.collect();
        let primary = rest.first() == Some(&"primary");
        let rect = rest.iter().take(2).find_map(|t| geometry(t));
        let upper = name.to_uppercase();
        let builtin =
            upper.starts_with("EDP") || upper.starts_with("LVDS") || upper.starts_with("DSI");
        screens.push(Screen {
            id: name.to_string(),
            connector: name.to_string(),
            name: if builtin {
                "Built-in display".into()
            } else {
                name.to_string()
            },
            identity: Identity::default(),
            builtin,
            enabled: rect.is_some(),
            primary: primary && rect.is_some(),
            rect: rect.unwrap_or_default(),
            scale: 1.0,
        });
    }
    finish_edid(&mut screens, &mut edid_hex);
    Ok(State {
        backend: "x11".into(),
        screens,
    })
}

/// The xrandr arguments for `layout`, in a single call so the screen is
/// resized once.
pub fn apply_args(text: &str, layout: &Layout) -> Result<Vec<String>, Error> {
    let state = parse(text)?;
    super::ensure_same_screens(&state, layout)?;
    let mut args = Vec::new();
    for s in state.enabled() {
        let p = layout.position(&s.id).ok_or(Error::Changed)?;
        args.extend([
            "--output".to_string(),
            s.id.clone(),
            "--pos".to_string(),
            format!("{}x{}", p.x, p.y),
        ]);
        if s.id == layout.primary {
            args.push("--primary".into());
        }
    }
    Ok(args)
}

pub struct X11 {
    runner: Box<dyn Runner>,
}

impl X11 {
    pub fn new(runner: Box<dyn Runner>) -> X11 {
        X11 { runner }
    }

    fn text(&self) -> Result<String, Error> {
        self.runner
            .run("xrandr", &["--current".to_string(), "--props".to_string()])
    }
}

impl Backend for X11 {
    fn name(&self) -> &'static str {
        "x11"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            primary: true,
            temporary: false,
            verify: false,
            remembers: false,
            confirms: false,
            origin: Origin::TopLeft,
        }
    }

    fn query(&self) -> Result<State, Error> {
        parse(&self.text()?)
    }

    fn apply(&self, layout: &Layout, _mode: ApplyMode) -> Result<(), Error> {
        let args = apply_args(&self.text()?, layout)?;
        self.runner.run("xrandr", &args).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edid::build;
    use crate::model::Position;
    use crate::run::testing::Scripted;

    fn hex_lines(bytes: &[u8]) -> String {
        bytes
            .chunks(16)
            .map(|c| {
                format!(
                    "\t\t{}\n",
                    c.iter().map(|b| format!("{b:02x}")).collect::<String>()
                )
            })
            .collect()
    }

    fn sample() -> String {
        let laptop = build("BOE", 0x095f, 0, None, "");
        let dell = build("DEL", 0x41b5, 0, Some("ABC123"), "DELL U2723QE");
        format!(
            "Screen 0: minimum 8 x 8, current 5560 x 1920, maximum 32767 x 32767\n\
eDP-1 connected primary 1920x1080+2560+180 (normal left inverted right x axis y axis) 309mm x 174mm\n\
\tEDID: \n{}\
\tscaling mode: Full aspect \n\
\t\tsupported: Full, Center, Full aspect\n\
   1920x1080     60.01*+  59.97  \n\
HDMI-1 connected 2560x1440+0+0 (normal left inverted right x axis y axis) 597mm x 336mm\n\
\tEDID: \n{}\
   2560x1440     59.95*+\n\
DP-1 connected (normal left inverted right x axis y axis)\n\
DP-2 disconnected (normal left inverted right x axis y axis)\n\
HDMI-2 connected 1080x1920+4480+0 left (normal left inverted right x axis y axis) 527mm x 296mm\n\
   1920x1080     60.00*+\n",
            hex_lines(&laptop),
            hex_lines(&dell)
        )
    }

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
    fn reads_geometry_and_edid() {
        let state = parse(&sample()).unwrap();
        let ids: Vec<&str> = state.screens.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["eDP-1", "HDMI-1", "DP-1", "HDMI-2"]);
        let laptop = state.screen("eDP-1").unwrap();
        assert_eq!(laptop.rect, Rect::new(2560, 180, 1920, 1080));
        assert!(laptop.builtin && laptop.primary);
        assert_eq!(laptop.name, "Built-in display");
        assert_eq!(laptop.key(), "boe:0x095f:");
        let dell = state.screen("HDMI-1").unwrap();
        assert_eq!(dell.rect, Rect::new(0, 0, 2560, 1440));
        assert_eq!(dell.name, "DELL U2723QE");
        assert_eq!(dell.key(), "del:0x41b5:abc123");
        assert!(!state.screen("DP-1").unwrap().enabled);
        let portrait = state.screen("HDMI-2").unwrap().rect;
        assert_eq!((portrait.width, portrait.height), (1080, 1920));
        assert_eq!(portrait.x, 4480);
    }

    #[test]
    fn apply_args_with_primary() {
        let args = apply_args(
            &sample(),
            &layout(
                "HDMI-1",
                &[("eDP-1", 2560, 180), ("HDMI-1", 0, 0), ("HDMI-2", 4480, 0)],
            ),
        )
        .unwrap();
        assert_eq!(
            args,
            [
                "--output",
                "eDP-1",
                "--pos",
                "2560x180",
                "--output",
                "HDMI-1",
                "--pos",
                "0x0",
                "--primary",
                "--output",
                "HDMI-2",
                "--pos",
                "4480x0"
            ]
        );
    }

    #[test]
    fn apply_args_changed() {
        assert!(matches!(
            apply_args(&sample(), &layout("eDP-1", &[("eDP-1", 0, 0)])),
            Err(Error::Changed)
        ));
    }

    #[test]
    fn backend_reads_without_reprobing() {
        let text = sample();
        let runner = std::sync::Arc::new(Scripted::new(vec![("xrandr", Ok(text.as_str()))]));
        struct Shared(std::sync::Arc<Scripted>);
        impl Runner for Shared {
            fn run(&self, program: &str, args: &[String]) -> Result<String, Error> {
                self.0.run(program, args)
            }
        }
        let x11 = X11::new(Box::new(Shared(runner.clone())));
        assert_eq!(x11.query().unwrap().screens.len(), 4);
        assert_eq!(runner.calls()[0], ["xrandr", "--current", "--props"]);
        let caps = x11.capabilities();
        assert!(caps.primary && !caps.temporary && !caps.verify && !caps.remembers);
    }
}
