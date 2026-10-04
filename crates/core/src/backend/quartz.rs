//! Quartz Display Services values as plain data. The global display space
//! is in points, with the main display's top-left corner at (0, 0) and y
//! going down; the main display is by definition the one at the origin.
//! No macOS API here, so this compiles and is tested everywhere.

use crate::model::{Identity, Rect, Screen, State};

#[derive(Debug, Clone, PartialEq)]
pub struct QuartzDisplay {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub builtin: bool,
    pub main: bool,
    pub vendor: u32,
    pub model: u32,
    pub serial: u32,
    pub pixels_wide: u64,
    /// The display this one mirrors, or 0.
    pub mirror_of: u32,
    pub name: Option<String>,
}

pub fn to_state(displays: &[QuartzDisplay]) -> State {
    let screens = displays
        .iter()
        .map(|d| {
            let enabled = d.mirror_of == 0;
            Screen {
                id: d.id.to_string(),
                connector: d.id.to_string(),
                name: d
                    .name
                    .clone()
                    .filter(|n| !n.trim().is_empty())
                    .unwrap_or_else(|| {
                        if d.builtin {
                            "Built-in display".into()
                        } else {
                            format!("Display {}", d.id)
                        }
                    }),
                identity: Identity {
                    vendor: format!("0x{:04x}", d.vendor),
                    product: format!("0x{:04x}", d.model),
                    serial: if d.serial == 0 {
                        String::new()
                    } else {
                        d.serial.to_string()
                    },
                },
                builtin: d.builtin,
                enabled,
                primary: enabled && d.main,
                rect: if enabled {
                    Rect::new(
                        d.x.round() as i32,
                        d.y.round() as i32,
                        d.width.round() as i32,
                        d.height.round() as i32,
                    )
                } else {
                    Rect::default()
                },
                scale: if d.width > 0.0 {
                    d.pixels_wide as f64 / d.width
                } else {
                    1.0
                },
            }
        })
        .collect();
    State {
        backend: "macos".into(),
        screens,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn builtin() -> QuartzDisplay {
        QuartzDisplay {
            id: 1,
            x: 0.0,
            y: 0.0,
            width: 1512.0,
            height: 982.0,
            builtin: true,
            main: true,
            vendor: 0x0610,
            model: 0xa050,
            serial: 0,
            pixels_wide: 3024,
            mirror_of: 0,
            name: None,
        }
    }

    fn dell() -> QuartzDisplay {
        QuartzDisplay {
            id: 2,
            x: -2560.0,
            y: -200.0,
            width: 2560.0,
            height: 1440.0,
            builtin: false,
            main: false,
            vendor: 0x10ac,
            model: 0x41b5,
            serial: 0,
            pixels_wide: 2560,
            mirror_of: 0,
            name: Some("DELL U2723QE".into()),
        }
    }

    #[test]
    fn reads_names_scale_and_the_main_display() {
        let state = to_state(&[builtin(), dell()]);
        assert_eq!(state.backend, "macos");
        let laptop = state.screen("1").unwrap();
        assert!(laptop.builtin && laptop.primary);
        assert_eq!(laptop.name, "Built-in display");
        assert_eq!(laptop.scale, 2.0);
        assert_eq!(laptop.rect, Rect::new(0, 0, 1512, 982));
        let ext = state.screen("2").unwrap();
        assert_eq!(ext.name, "DELL U2723QE");
        assert_eq!(ext.key(), "0x10ac:0x41b5:");
        assert_eq!(ext.rect, Rect::new(-2560, -200, 2560, 1440));
        assert!(!ext.primary);
    }

    #[test]
    fn a_mirror_is_off() {
        let mirror = QuartzDisplay {
            mirror_of: 1,
            ..dell()
        };
        assert!(!to_state(&[builtin(), mirror]).screen("2").unwrap().enabled);
    }

    #[test]
    fn rounds_points_half_away_from_zero() {
        let odd = QuartzDisplay {
            x: -1280.5,
            width: 1280.5,
            ..dell()
        };
        let r = to_state(&[odd]).screens[0].rect;
        assert_eq!((r.x, r.width), (-1281, 1281));
    }

    #[test]
    fn a_nameless_external_gets_a_numbered_name_and_a_serial_key() {
        let plain = QuartzDisplay {
            name: None,
            serial: 77,
            ..dell()
        };
        let s = &to_state(&[plain]).screens[0];
        assert_eq!(s.name, "Display 2");
        assert_eq!(s.key(), "0x10ac:0x41b5:77");
    }
}
