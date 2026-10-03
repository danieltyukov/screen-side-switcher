//! The picture every backend reports and every operation works on.
//!
//! Positions and sizes are in the backend's own coordinate space (logical
//! pixels, physical pixels or points). Backends convert before filling a
//! [`Rect`], so two screens that touch on the desktop touch here too.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(&self) -> i32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.height
    }

    /// True when the two share a positive area. Touching is not overlapping.
    pub fn overlaps(&self, other: &Rect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }

    /// Length of the edge the two share. Zero when they meet only at a
    /// corner or not at all, which is where the pointer cannot cross.
    pub fn shared_edge(&self, other: &Rect) -> i32 {
        if self.right() == other.x || other.right() == self.x {
            (self.bottom().min(other.bottom()) - self.y.max(other.y)).max(0)
        } else if self.bottom() == other.y || other.bottom() == self.y {
            (self.right().min(other.right()) - self.x.max(other.x)).max(0)
        } else {
            0
        }
    }
}

/// What a monitor says it is, for recognising it in a later session.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Identity {
    pub vendor: String,
    pub product: String,
    pub serial: String,
}

impl Identity {
    /// `vendor:product:serial`, lower-cased. A backend that reports neither
    /// vendor nor product gets `connector:<name>` instead.
    pub fn key(&self, connector: &str) -> String {
        let norm = |s: &str| s.trim().to_lowercase();
        let (vendor, product, serial) =
            (norm(&self.vendor), norm(&self.product), norm(&self.serial));
        if vendor.is_empty() && product.is_empty() {
            format!("connector:{}", norm(connector))
        } else {
            format!("{vendor}:{product}:{serial}")
        }
    }
}

fn one() -> f64 {
    1.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Screen {
    /// Stable within a session: connector, CCD target, CGDirectDisplayID.
    pub id: String,
    pub connector: String,
    pub name: String,
    #[serde(default)]
    pub identity: Identity,
    pub builtin: bool,
    /// Part of the desktop right now. A closed lid is connected but off.
    pub enabled: bool,
    pub primary: bool,
    pub rect: Rect,
    #[serde(default = "one")]
    pub scale: f64,
}

impl Screen {
    pub fn key(&self) -> String {
        self.identity.key(&self.connector)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub backend: String,
    pub screens: Vec<Screen>,
}

impl State {
    pub fn screen(&self, id: &str) -> Option<&Screen> {
        self.screens.iter().find(|s| s.id == id)
    }

    pub fn enabled(&self) -> impl Iterator<Item = &Screen> {
        self.screens.iter().filter(|s| s.enabled)
    }

    pub fn primary(&self) -> Option<&Screen> {
        self.enabled().find(|s| s.primary)
    }

    /// The order `status` numbers screens in: enabled ones left to right
    /// and top to bottom, then the switched-off ones by connector.
    pub fn listed(&self) -> Vec<&Screen> {
        let mut on: Vec<&Screen> = self.enabled().collect();
        on.sort_by_key(|s| (s.rect.x, s.rect.y, s.connector.clone()));
        let mut off: Vec<&Screen> = self.screens.iter().filter(|s| !s.enabled).collect();
        off.sort_by_key(|s| s.connector.clone());
        on.extend(off);
        on
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub id: String,
    pub x: i32,
    pub y: i32,
}

/// What a backend is asked to apply: a position for every enabled screen
/// and which one is primary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layout {
    pub positions: Vec<Position>,
    pub primary: String,
}

impl Layout {
    pub fn position(&self, id: &str) -> Option<&Position> {
        self.positions.iter().find(|p| p.id == id)
    }

    /// The layout already in force. Used to check an apply path without
    /// moving anything.
    pub fn from_state(state: &State) -> Layout {
        let primary = state
            .primary()
            .or_else(|| state.enabled().next())
            .map(|s| s.id.clone())
            .unwrap_or_default();
        Layout {
            positions: state
                .enabled()
                .map(|s| Position {
                    id: s.id.clone(),
                    x: s.rect.x,
                    y: s.rect.y,
                })
                .collect(),
            primary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(id: &str, x: i32, y: i32, w: i32, h: i32) -> Screen {
        Screen {
            id: id.into(),
            connector: id.into(),
            name: id.into(),
            identity: Identity::default(),
            builtin: false,
            enabled: true,
            primary: false,
            rect: Rect::new(x, y, w, h),
            scale: 1.0,
        }
    }

    #[test]
    fn overlap_needs_positive_area() {
        let a = Rect::new(0, 0, 100, 100);
        assert!(a.overlaps(&Rect::new(50, 50, 100, 100)));
        assert!(
            !a.overlaps(&Rect::new(100, 0, 100, 100)),
            "touching is not overlapping"
        );
        assert!(
            !a.overlaps(&Rect::new(100, 100, 10, 10)),
            "a corner is not overlapping"
        );
    }

    #[test]
    fn shared_edge_is_zero_at_a_corner() {
        let a = Rect::new(0, 0, 100, 100);
        assert_eq!(a.shared_edge(&Rect::new(100, 20, 50, 50)), 50);
        assert_eq!(a.shared_edge(&Rect::new(-50, 90, 50, 50)), 10);
        assert_eq!(a.shared_edge(&Rect::new(20, 100, 200, 10)), 80);
        assert_eq!(a.shared_edge(&Rect::new(100, 100, 10, 10)), 0);
        assert_eq!(a.shared_edge(&Rect::new(150, 0, 10, 10)), 0);
    }

    #[test]
    fn identity_key_falls_back_to_the_connector() {
        let id = Identity {
            vendor: " DEL ".into(),
            product: "0x41B5".into(),
            serial: "ABC".into(),
        };
        assert_eq!(id.key("HDMI-1"), "del:0x41b5:abc");
        assert_eq!(Identity::default().key("HDMI-1"), "connector:hdmi-1");
    }

    #[test]
    fn listed_puts_enabled_screens_first_in_reading_order() {
        let mut off = screen("eDP-1", 0, 0, 0, 0);
        off.enabled = false;
        let state = State {
            backend: "fake".into(),
            screens: vec![
                off,
                screen("B", 1920, 0, 100, 100),
                screen("A", 0, 0, 1920, 1080),
            ],
        };
        let order: Vec<&str> = state.listed().iter().map(|s| s.id.as_str()).collect();
        assert_eq!(order, ["A", "B", "eDP-1"]);
    }

    #[test]
    fn layout_from_state_keeps_current_positions() {
        let mut a = screen("A", 0, 0, 10, 10);
        a.primary = true;
        let state = State {
            backend: "fake".into(),
            screens: vec![a, screen("B", 10, 0, 10, 10)],
        };
        let layout = Layout::from_state(&state);
        assert_eq!(layout.primary, "A");
        assert_eq!(
            layout.position("B"),
            Some(&Position {
                id: "B".into(),
                x: 10,
                y: 0
            })
        );
    }
}
