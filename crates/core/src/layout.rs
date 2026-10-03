//! Turns "put this screen on the left" into coordinates the system accepts.
//!
//! Systems refuse arrangements whose screens overlap, and the pointer cannot
//! cross where two screens meet only at a corner. So screens are placed with
//! exact adjacency along one axis and a deliberate alignment on the other,
//! and anything that would overlap is refused rather than nudged into a gap.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::model::{Layout, Position, Rect, Screen, State};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
    Above,
    Below,
}

impl Side {
    pub const ALL: [Side; 4] = [Side::Left, Side::Right, Side::Above, Side::Below];

    pub fn horizontal(self) -> bool {
        matches!(self, Side::Left | Side::Right)
    }

    pub fn mirrored(self) -> Side {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
            Side::Above => Side::Below,
            Side::Below => Side::Above,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Right => "right",
            Side::Above => "above",
            Side::Below => "below",
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Side {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "left" => Ok(Side::Left),
            "right" => Ok(Side::Right),
            "above" | "up" | "top" => Ok(Side::Above),
            "below" | "down" | "bottom" => Ok(Side::Below),
            other => Err(format!(
                "unknown side '{other}', expected left, right, above or below"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Start,
    #[default]
    Center,
    End,
}

impl Align {
    /// How the alignment reads for screens side by side (`horizontal`) or
    /// stacked.
    pub fn label(self, horizontal: bool) -> &'static str {
        match (self, horizontal) {
            (Align::Start, true) => "top",
            (Align::Start, false) => "left",
            (Align::Center, _) => "centred",
            (Align::End, true) => "bottom",
            (Align::End, false) => "right",
        }
    }
}

impl fmt::Display for Align {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Align::Start => "start",
            Align::Center => "center",
            Align::End => "end",
        })
    }
}

impl FromStr for Align {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "start" | "top" | "left" => Ok(Align::Start),
            "center" | "centre" | "middle" => Ok(Align::Center),
            "end" | "bottom" | "right" => Ok(Align::End),
            other => Err(format!(
                "unknown alignment '{other}', expected start, center or end"
            )),
        }
    }
}

/// Where the finished layout is anchored. Windows and macOS define the
/// primary display as the one at (0, 0); everything else wants the layout
/// to start at (0, 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    TopLeft,
    Primary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    pub screen: String,
    pub side: Side,
}

/// The intent behind a layout: which side each screen is on, nearest first
/// within a side, how they line up, and which is primary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Arrangement {
    pub anchor: String,
    pub placements: Vec<Placement>,
    pub align: Align,
    /// False when the screens in force match no alignment ("custom").
    pub aligned: bool,
    pub primary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    #[error("No screen is switched on.")]
    NoScreens,
    #[error("Only one screen is switched on, so there is nothing to arrange.")]
    OnlyOneScreen,
    #[error("{0} is not connected or is switched off.")]
    UnknownScreen(String),
    #[error("{0} has no place in the arrangement.")]
    MissingScreen(String),
    #[error("{0} reports no usable resolution.")]
    NoResolution(String),
    #[error(
        "{0} and {1} would overlap. Put one of them on another side, or use another alignment."
    )]
    Overlap(String, String),
    #[error("{0} would not touch the other screens, so the pointer could not reach it.")]
    Disconnected(String),
    #[error("Pick an external screen to move. The built-in screen stays where it is.")]
    AnchorSelected,
}

/// The screen everything is arranged around: the built-in one if it is on,
/// else the primary, else the first enabled screen.
pub fn anchor(state: &State) -> Option<&Screen> {
    state
        .enabled()
        .find(|s| s.builtin)
        .or_else(|| state.primary())
        .or_else(|| state.enabled().next())
}

fn offset(align: Align, anchor_len: i32, len: i32) -> i32 {
    match align {
        Align::Start => 0,
        Align::End => anchor_len - len,
        Align::Center => (anchor_len - len).div_euclid(2),
    }
}

/// Index of the first rectangle that cannot be reached from the first one by
/// crossing shared edges, if any.
pub(crate) fn first_unreachable(rects: &[Rect]) -> Option<usize> {
    if rects.is_empty() {
        return None;
    }
    let mut reached = vec![false; rects.len()];
    reached[0] = true;
    let mut stack = vec![0];
    while let Some(i) = stack.pop() {
        for j in 0..rects.len() {
            if !reached[j] && rects[i].shared_edge(&rects[j]) > 0 {
                reached[j] = true;
                stack.push(j);
            }
        }
    }
    reached.iter().position(|r| !r)
}

pub fn compute(state: &State, arr: &Arrangement, origin: Origin) -> Result<Layout, LayoutError> {
    if state.enabled().next().is_none() {
        return Err(LayoutError::NoScreens);
    }
    let find = |id: &str| {
        state.screen(id).filter(|s| s.enabled).ok_or_else(|| {
            LayoutError::UnknownScreen(state.screen(id).map_or(id, |s| &s.name).to_string())
        })
    };
    let anchor = find(&arr.anchor)?;
    find(&arr.primary)?;

    let mut placements: Vec<(&Screen, Side)> = Vec::new();
    for p in &arr.placements {
        let s = find(&p.screen)?;
        if s.id != anchor.id && !placements.iter().any(|(q, _)| q.id == s.id) {
            placements.push((s, p.side));
        }
    }
    for s in state.enabled() {
        if s.id != anchor.id && !placements.iter().any(|(q, _)| q.id == s.id) {
            return Err(LayoutError::MissingScreen(s.name.clone()));
        }
    }
    for s in std::iter::once(anchor).chain(placements.iter().map(|(s, _)| *s)) {
        if s.rect.width <= 0 || s.rect.height <= 0 {
            return Err(LayoutError::NoResolution(s.name.clone()));
        }
    }

    let (aw, ah) = (anchor.rect.width, anchor.rect.height);
    let mut placed: Vec<(&Screen, Rect)> = vec![(anchor, Rect::new(0, 0, aw, ah))];
    for side in Side::ALL {
        let mut cursor = 0;
        for (s, _) in placements.iter().filter(|(_, sd)| *sd == side) {
            let (w, h) = (s.rect.width, s.rect.height);
            let rect = match side {
                Side::Left => {
                    cursor += w;
                    Rect::new(-cursor, offset(arr.align, ah, h), w, h)
                }
                Side::Right => {
                    let r = Rect::new(aw + cursor, offset(arr.align, ah, h), w, h);
                    cursor += w;
                    r
                }
                Side::Above => {
                    cursor += h;
                    Rect::new(offset(arr.align, aw, w), -cursor, w, h)
                }
                Side::Below => {
                    let r = Rect::new(offset(arr.align, aw, w), ah + cursor, w, h);
                    cursor += h;
                    r
                }
            };
            placed.push((s, rect));
        }
    }

    for (i, (a, ra)) in placed.iter().enumerate() {
        for (b, rb) in &placed[i + 1..] {
            if ra.overlaps(rb) {
                return Err(LayoutError::Overlap(a.name.clone(), b.name.clone()));
            }
        }
    }
    let rects: Vec<Rect> = placed.iter().map(|(_, r)| *r).collect();
    if let Some(i) = first_unreachable(&rects) {
        return Err(LayoutError::Disconnected(placed[i].0.name.clone()));
    }

    let (dx, dy) = match origin {
        Origin::TopLeft => (
            -rects.iter().map(|r| r.x).min().unwrap_or(0),
            -rects.iter().map(|r| r.y).min().unwrap_or(0),
        ),
        Origin::Primary => {
            let r = placed
                .iter()
                .find(|(s, _)| s.id == arr.primary)
                .map(|(_, r)| *r)
                .unwrap_or_default();
            (-r.x, -r.y)
        }
    };
    Ok(Layout {
        positions: placed
            .iter()
            .map(|(s, r)| Position {
                id: s.id.clone(),
                x: r.x + dx,
                y: r.y + dy,
            })
            .collect(),
        primary: arr.primary.clone(),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::{Identity, Rect, Screen, State};

    pub(crate) fn screen(id: &str, w: i32, h: i32) -> Screen {
        Screen {
            id: id.into(),
            connector: id.into(),
            name: id.into(),
            identity: Identity::default(),
            builtin: false,
            enabled: true,
            primary: false,
            rect: Rect::new(0, 0, w, h),
            scale: 1.0,
        }
    }

    pub(crate) fn laptop(w: i32, h: i32) -> Screen {
        Screen {
            builtin: true,
            primary: true,
            ..screen("eDP-1", w, h)
        }
    }

    pub(crate) fn state(screens: Vec<Screen>) -> State {
        State {
            backend: "fake".into(),
            screens,
        }
    }

    pub(crate) fn arr(placements: &[(&str, Side)], align: Align) -> Arrangement {
        Arrangement {
            anchor: "eDP-1".into(),
            placements: placements
                .iter()
                .map(|(id, side)| Placement {
                    screen: id.to_string(),
                    side: *side,
                })
                .collect(),
            align,
            aligned: true,
            primary: "eDP-1".into(),
        }
    }

    fn at(layout: &Layout, id: &str) -> (i32, i32) {
        let p = layout.position(id).expect("screen in layout");
        (p.x, p.y)
    }

    #[test]
    fn one_screen_left_centred_matches_v1() {
        // 1.0 put a 1440-high monitor left of an 800-high laptop at (0,0)
        // and the laptop at (2560,320).
        let s = state(vec![laptop(1280, 800), screen("HDMI-1", 2560, 1440)]);
        let l = compute(
            &s,
            &arr(&[("HDMI-1", Side::Left)], Align::Center),
            Origin::TopLeft,
        )
        .unwrap();
        assert_eq!(at(&l, "HDMI-1"), (0, 0));
        assert_eq!(at(&l, "eDP-1"), (2560, 320));
        assert_eq!(l.primary, "eDP-1");
    }

    #[test]
    fn each_side_and_alignment() {
        let s = state(vec![laptop(1280, 800), screen("HDMI-1", 1920, 1080)]);
        let cases = [
            (Side::Right, Align::Start, (0, 0), (1280, 0)),
            (Side::Right, Align::End, (0, 280), (1280, 0)),
            (Side::Above, Align::Center, (320, 1080), (0, 0)),
            (Side::Below, Align::Start, (0, 0), (0, 800)),
            (Side::Below, Align::End, (640, 0), (0, 800)),
        ];
        for (side, align, laptop_at, external_at) in cases {
            let l = compute(&s, &arr(&[("HDMI-1", side)], align), Origin::TopLeft).unwrap();
            assert_eq!(at(&l, "eDP-1"), laptop_at, "{side} {align}");
            assert_eq!(at(&l, "HDMI-1"), external_at, "{side} {align}");
        }
    }

    #[test]
    fn odd_differences_round_down() {
        // floor((801 - 1440) / 2) = -320, so the laptop sits 320 down.
        let s = state(vec![laptop(1280, 801), screen("HDMI-1", 2560, 1440)]);
        let l = compute(
            &s,
            &arr(&[("HDMI-1", Side::Right)], Align::Center),
            Origin::TopLeft,
        )
        .unwrap();
        assert_eq!(at(&l, "eDP-1"), (0, 320));
    }

    #[test]
    fn chains_go_outward_in_order() {
        let s = state(vec![
            laptop(1280, 800),
            screen("A", 1920, 1080),
            screen("B", 2560, 1440),
        ]);
        let l = compute(
            &s,
            &arr(&[("A", Side::Left), ("B", Side::Left)], Align::Start),
            Origin::TopLeft,
        )
        .unwrap();
        assert_eq!(at(&l, "B"), (0, 0));
        assert_eq!(at(&l, "A"), (2560, 0));
        assert_eq!(at(&l, "eDP-1"), (4480, 0));
    }

    #[test]
    fn mixed_sides() {
        let s = state(vec![
            laptop(1280, 800),
            screen("A", 1920, 1080),
            screen("B", 1280, 1024),
        ]);
        let l = compute(
            &s,
            &arr(&[("A", Side::Left), ("B", Side::Above)], Align::Start),
            Origin::TopLeft,
        )
        .unwrap();
        assert_eq!(at(&l, "A"), (0, 1024));
        assert_eq!(at(&l, "B"), (1920, 0));
        assert_eq!(at(&l, "eDP-1"), (1920, 1024));
    }

    #[test]
    fn perpendicular_overlap_is_refused() {
        let s = state(vec![
            laptop(1280, 800),
            screen("A", 2560, 1440),
            screen("B", 2560, 1440),
        ]);
        let err = compute(
            &s,
            &arr(&[("A", Side::Left), ("B", Side::Above)], Align::Center),
            Origin::TopLeft,
        )
        .unwrap_err();
        assert_eq!(err, LayoutError::Overlap("A".into(), "B".into()));
        assert!(err.to_string().contains("would overlap"));
        // Start alignment keeps them apart.
        assert!(compute(
            &s,
            &arr(&[("A", Side::Left), ("B", Side::Above)], Align::Start),
            Origin::TopLeft
        )
        .is_ok());
    }

    #[test]
    fn primary_origin_puts_the_primary_at_zero() {
        let s = state(vec![laptop(1280, 800), screen("HDMI-1", 2560, 1440)]);
        let mut a = arr(&[("HDMI-1", Side::Left)], Align::Center);
        let l = compute(&s, &a, Origin::Primary).unwrap();
        assert_eq!(at(&l, "eDP-1"), (0, 0));
        assert_eq!(at(&l, "HDMI-1"), (-2560, -320));
        a.primary = "HDMI-1".into();
        let l = compute(&s, &a, Origin::Primary).unwrap();
        assert_eq!(at(&l, "HDMI-1"), (0, 0));
        assert_eq!(at(&l, "eDP-1"), (2560, 320));
    }

    #[test]
    fn switched_off_screens_are_left_alone() {
        let mut lid = laptop(1280, 800);
        lid.enabled = false;
        lid.primary = false;
        let mut a = screen("A", 1920, 1080);
        a.primary = true;
        let s = state(vec![lid, a, screen("B", 1920, 1080)]);
        let mut arrangement = arr(&[("B", Side::Right)], Align::Center);
        arrangement.anchor = "A".into();
        arrangement.primary = "A".into();
        let l = compute(&s, &arrangement, Origin::TopLeft).unwrap();
        assert_eq!(l.positions.len(), 2);
        assert!(l.position("eDP-1").is_none());
        // Naming the switched-off screen is refused.
        let mut bad = arrangement.clone();
        bad.placements.push(Placement {
            screen: "eDP-1".into(),
            side: Side::Left,
        });
        assert_eq!(
            compute(&s, &bad, Origin::TopLeft).unwrap_err(),
            LayoutError::UnknownScreen("eDP-1".into())
        );
    }

    #[test]
    fn every_enabled_screen_needs_a_place() {
        let s = state(vec![
            laptop(1280, 800),
            screen("A", 1920, 1080),
            screen("B", 1920, 1080),
        ]);
        let err = compute(
            &s,
            &arr(&[("A", Side::Left)], Align::Center),
            Origin::TopLeft,
        )
        .unwrap_err();
        assert_eq!(err, LayoutError::MissingScreen("B".into()));
    }

    #[test]
    fn a_screen_without_a_resolution_is_refused() {
        let s = state(vec![laptop(1280, 800), screen("A", 0, 0)]);
        let err = compute(
            &s,
            &arr(&[("A", Side::Left)], Align::Center),
            Origin::TopLeft,
        )
        .unwrap_err();
        assert_eq!(err, LayoutError::NoResolution("A".into()));
    }

    #[test]
    fn connectivity_finds_an_island() {
        let rects = [
            Rect::new(0, 0, 10, 10),
            Rect::new(10, 0, 10, 10),
            Rect::new(30, 0, 10, 10),
        ];
        assert_eq!(first_unreachable(&rects), Some(2));
        assert_eq!(first_unreachable(&rects[..2]), None);
        let corner = [Rect::new(0, 0, 10, 10), Rect::new(10, 10, 10, 10)];
        assert_eq!(first_unreachable(&corner), Some(1));
    }

    #[test]
    fn parsing_and_labels() {
        assert_eq!("LEFT".parse::<Side>().unwrap(), Side::Left);
        assert!("sideways".parse::<Side>().is_err());
        assert_eq!("centre".parse::<Align>().unwrap(), Align::Center);
        assert_eq!("bottom".parse::<Align>().unwrap(), Align::End);
        assert_eq!(Align::Start.label(true), "top");
        assert_eq!(Align::End.label(false), "right");
        assert_eq!(Side::Above.mirrored(), Side::Below);
        assert_eq!(serde_json::to_string(&Side::Below).unwrap(), "\"below\"");
        assert_eq!(
            serde_json::to_string(&Origin::TopLeft).unwrap(),
            "\"top_left\""
        );
    }
}
