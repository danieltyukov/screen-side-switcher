//! Mutter's DisplayConfig state as plain data, and the conversions to and
//! from it. No D-Bus here, so this compiles and is tested everywhere.

use serde::{Deserialize, Serialize};

use crate::model::{Identity, Layout, Rect, Screen, State};
use crate::Error;

/// Positions in scaled pixels: a 2560 wide panel at scale 2 is 1280 wide.
pub const LAYOUT_LOGICAL: u32 = 1;
/// Positions in device pixels: the same panel is 2560 wide.
pub const LAYOUT_PHYSICAL: u32 = 2;
/// Transforms that swap width and height (90 and 270, flipped or not).
const ROTATED: [u32; 4] = [1, 3, 5, 7];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutterMode {
    pub id: String,
    pub width: i32,
    pub height: i32,
    pub current: bool,
    pub preferred: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutterMonitor {
    pub connector: String,
    pub vendor: String,
    pub product: String,
    pub serial: String,
    pub display_name: String,
    pub builtin: bool,
    pub modes: Vec<MutterMode>,
    /// `is-underscanning`, present only where underscanning is supported.
    #[serde(default)]
    pub underscanning: Option<bool>,
    /// `color-mode` (HDR), reported by newer Mutter.
    #[serde(default)]
    pub color_mode: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutterLogical {
    pub x: i32,
    pub y: i32,
    pub scale: f64,
    pub transform: u32,
    pub primary: bool,
    pub connectors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutterState {
    pub serial: u32,
    pub layout_mode: u32,
    pub supports_changing_layout_mode: bool,
    pub monitors: Vec<MutterMonitor>,
    pub logical: Vec<MutterLogical>,
}

/// One monitor in an ApplyMonitorsConfig request. Mutter resets any
/// monitor property that is not passed back, so the ones it reported are.
#[derive(Debug, Clone, PartialEq)]
pub struct ApplyMonitor {
    pub connector: String,
    pub mode: String,
    pub underscanning: Option<bool>,
    pub color_mode: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplyLogical {
    pub x: i32,
    pub y: i32,
    pub scale: f64,
    pub transform: u32,
    pub primary: bool,
    pub monitors: Vec<ApplyMonitor>,
}

impl MutterState {
    fn monitor(&self, connector: &str) -> Option<&MutterMonitor> {
        self.monitors.iter().find(|m| m.connector == connector)
    }

    fn mode(&self, connector: &str) -> Option<&MutterMode> {
        let m = self.monitor(connector)?;
        m.modes
            .iter()
            .find(|m| m.current)
            .or_else(|| m.modes.iter().find(|m| m.preferred))
    }

    /// The size this logical monitor occupies in the active coordinate space.
    fn extent(&self, logical: &MutterLogical) -> (i32, i32) {
        let Some(mode) = logical.connectors.first().and_then(|c| self.mode(c)) else {
            return (0, 0);
        };
        let (mut w, mut h) = (mode.width, mode.height);
        if ROTATED.contains(&logical.transform) {
            std::mem::swap(&mut w, &mut h);
        }
        if self.layout_mode == LAYOUT_LOGICAL && logical.scale > 0.0 {
            // Rounded the way Mutter rounds, or adjacent monitors leave a
            // one-pixel seam that the pointer cannot cross.
            w = (w as f64 / logical.scale).round() as i32;
            h = (h as f64 / logical.scale).round() as i32;
        }
        (w, h)
    }

    pub fn to_state(&self) -> State {
        let screens = self
            .monitors
            .iter()
            .map(|m| {
                let logical = self
                    .logical
                    .iter()
                    .find(|l| l.connectors.contains(&m.connector));
                let leads = logical.is_some_and(|l| l.connectors.first() == Some(&m.connector));
                let (rect, primary, scale) = match logical {
                    Some(l) if leads => {
                        let (w, h) = self.extent(l);
                        (Rect::new(l.x, l.y, w, h), l.primary, l.scale)
                    }
                    _ => (Rect::default(), false, 1.0),
                };
                let name = if !m.display_name.trim().is_empty() {
                    m.display_name.trim().to_string()
                } else if m.builtin {
                    "Built-in display".to_string()
                } else {
                    format!("{} {}", m.vendor, m.product).trim().to_string()
                };
                Screen {
                    id: m.connector.clone(),
                    connector: m.connector.clone(),
                    name,
                    identity: Identity {
                        vendor: m.vendor.clone(),
                        product: m.product.clone(),
                        serial: m.serial.clone(),
                    },
                    builtin: m.builtin,
                    enabled: leads,
                    primary,
                    rect,
                    scale,
                }
            })
            .collect();
        State {
            backend: "gnome".into(),
            screens,
        }
    }

    /// The logical monitors to hand to ApplyMonitorsConfig for `layout`,
    /// keeping each one's scale, transform, mode and mirrors.
    pub fn apply_config(&self, layout: &Layout) -> Result<Vec<ApplyLogical>, Error> {
        let mut leads: Vec<&str> = self
            .logical
            .iter()
            .filter_map(|l| l.connectors.first().map(String::as_str))
            .collect();
        let mut named: Vec<&str> = layout.positions.iter().map(|p| p.id.as_str()).collect();
        leads.sort_unstable();
        named.sort_unstable();
        if leads != named {
            return Err(Error::Changed);
        }
        self.logical
            .iter()
            .map(|l| {
                let lead = &l.connectors[0];
                let p = layout.position(lead).ok_or(Error::Changed)?;
                let monitors = l
                    .connectors
                    .iter()
                    .map(|c| {
                        let mode = self
                            .mode(c)
                            .ok_or_else(|| Error::System(format!("{c} reports no usable mode.")))?;
                        let monitor = self.monitor(c);
                        Ok(ApplyMonitor {
                            connector: c.clone(),
                            mode: mode.id.clone(),
                            underscanning: monitor.and_then(|m| m.underscanning),
                            color_mode: monitor.and_then(|m| m.color_mode),
                        })
                    })
                    .collect::<Result<_, Error>>()?;
                Ok(ApplyLogical {
                    x: p.x,
                    y: p.y,
                    scale: l.scale,
                    transform: l.transform,
                    primary: *lead == layout.primary,
                    monitors,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Position;

    fn fixture(name: &str) -> MutterState {
        let text = match name {
            "laptop-left" => include_str!("../../tests/fixtures/mutter/laptop-left.json"),
            "fractional" => include_str!("../../tests/fixtures/mutter/fractional.json"),
            "physical" => include_str!("../../tests/fixtures/mutter/physical.json"),
            "lid-closed" => include_str!("../../tests/fixtures/mutter/lid-closed.json"),
            "mirror" => include_str!("../../tests/fixtures/mutter/mirror.json"),
            other => panic!("no fixture {other}"),
        };
        serde_json::from_str(text).unwrap()
    }

    fn size(state: &State, id: &str) -> (i32, i32) {
        let r = state.screen(id).unwrap().rect;
        (r.width, r.height)
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
    fn laptop_left() {
        let state = fixture("laptop-left").to_state();
        assert_eq!(state.backend, "gnome");
        let laptop = state.screen("eDP-1").unwrap();
        assert_eq!(laptop.rect, Rect::new(2560, 320, 1280, 800));
        assert!(laptop.builtin && laptop.primary && laptop.enabled);
        assert_eq!(laptop.name, "Built-in display");
        assert_eq!(laptop.key(), "boe:0x095f:");
        let dell = state.screen("HDMI-1").unwrap();
        assert_eq!(dell.rect, Rect::new(0, 0, 2560, 1440));
        assert_eq!(dell.name, "Dell Inc. 27\"");
        assert!(!dell.primary);
    }

    #[test]
    fn fractional_sizes_round_like_mutter() {
        let state = fixture("fractional").to_state();
        assert_eq!(size(&state, "eDP-1"), (1707, 1067));
        assert_eq!(size(&state, "HDMI-1"), (1536, 864));
        assert_eq!(size(&state, "DP-1"), (1080, 1920));
    }

    #[test]
    fn physical_mode_ignores_scale() {
        assert_eq!(size(&fixture("physical").to_state(), "eDP-1"), (2560, 1600));
    }

    #[test]
    fn lid_closed_is_listed_but_off() {
        let state = fixture("lid-closed").to_state();
        let laptop = state.screen("eDP-1").unwrap();
        assert!(!laptop.enabled && !laptop.primary);
        assert_eq!(laptop.rect, Rect::default());
        assert!(state.screen("HDMI-1").unwrap().enabled);
    }

    #[test]
    fn mirror_lists_the_second_as_off() {
        let state = fixture("mirror").to_state();
        assert!(state.screen("eDP-1").unwrap().enabled);
        assert!(!state.screen("HDMI-1").unwrap().enabled);
    }

    #[test]
    fn apply_config_keeps_scale_transform_and_modes() {
        let config = fixture("laptop-left")
            .apply_config(&layout("eDP-1", &[("eDP-1", 0, 0), ("HDMI-1", 1280, 0)]))
            .unwrap();
        assert_eq!(
            config,
            vec![
                ApplyLogical {
                    x: 0,
                    y: 0,
                    scale: 2.0,
                    transform: 0,
                    primary: true,
                    monitors: vec![ApplyMonitor {
                        connector: "eDP-1".into(),
                        mode: "2560x1600@60.000".into(),
                        underscanning: None,
                        color_mode: None,
                    }],
                },
                ApplyLogical {
                    x: 1280,
                    y: 0,
                    scale: 1.0,
                    transform: 0,
                    primary: false,
                    // Mutter resets what is not passed back, so a move must
                    // not switch off underscanning or HDR.
                    monitors: vec![ApplyMonitor {
                        connector: "HDMI-1".into(),
                        mode: "2560x1440@59.951".into(),
                        underscanning: Some(true),
                        color_mode: Some(1),
                    }],
                },
            ]
        );
    }

    #[test]
    fn apply_config_keeps_mirrors_together() {
        let config = fixture("mirror")
            .apply_config(&layout("eDP-1", &[("eDP-1", 0, 0)]))
            .unwrap();
        assert_eq!(config.len(), 1);
        let connectors: Vec<&str> = config[0]
            .monitors
            .iter()
            .map(|m| m.connector.as_str())
            .collect();
        assert_eq!(connectors, ["eDP-1", "HDMI-1"]);
        // Each mirrored monitor keeps its own current mode.
        assert_eq!(config[0].monitors[1].mode, "2560x1440@59.951");
    }

    #[test]
    fn apply_config_spots_changed_screens() {
        let state = fixture("laptop-left");
        assert!(matches!(
            state.apply_config(&layout("eDP-1", &[("eDP-1", 0, 0), ("DP-9", 1280, 0)])),
            Err(Error::Changed)
        ));
        assert!(matches!(
            state.apply_config(&layout("eDP-1", &[("eDP-1", 0, 0)])),
            Err(Error::Changed)
        ));
    }
}
