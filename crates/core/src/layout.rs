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

/// The arrangement in force. None with fewer than two screens on, or when
/// a screen overlaps the anchor (a layout made by hand that no side
/// describes).
pub fn infer(state: &State) -> Option<Arrangement> {
    if state.enabled().count() < 2 {
        return None;
    }
    let anchor = anchor(state)?;
    let a = anchor.rect;
    let mut found: Vec<(Side, i32, &Screen)> = Vec::new();
    for s in state.enabled().filter(|s| s.id != anchor.id) {
        let r = s.rect;
        let gaps = [
            (Side::Left, a.x - r.right()),
            (Side::Right, r.x - a.right()),
            (Side::Above, a.y - r.bottom()),
            (Side::Below, r.y - a.bottom()),
        ];
        let (side, gap) = gaps
            .into_iter()
            .filter(|(_, g)| *g >= 0)
            .max_by_key(|(_, g)| *g)?;
        found.push((side, gap, s));
    }
    found.sort_by_key(|(side, gap, s)| {
        (
            Side::ALL.iter().position(|x| x == side),
            *gap,
            s.connector.clone(),
        )
    });

    let matches = |align: Align| {
        found.iter().all(|(side, _, s)| {
            let (anchor_len, len, actual) = if side.horizontal() {
                (a.height, s.rect.height, s.rect.y - a.y)
            } else {
                (a.width, s.rect.width, s.rect.x - a.x)
            };
            (actual - offset(align, anchor_len, len)).abs() <= 1
        })
    };
    let align = [Align::Center, Align::Start, Align::End]
        .into_iter()
        .find(|al| matches(*al));

    Some(Arrangement {
        anchor: anchor.id.clone(),
        placements: found
            .iter()
            .map(|(side, _, s)| Placement {
                screen: s.id.clone(),
                side: *side,
            })
            .collect(),
        align: align.unwrap_or_default(),
        aligned: align.is_some(),
        primary: state
            .primary()
            .map_or_else(|| anchor.id.clone(), |s| s.id.clone()),
    })
}

/// The arrangement operations start from: the one in force, or every other
/// screen to the right of the anchor in left-to-right order.
pub fn baseline(state: &State) -> Result<Arrangement, LayoutError> {
    match state.enabled().count() {
        0 => return Err(LayoutError::NoScreens),
        1 => return Err(LayoutError::OnlyOneScreen),
        _ => {}
    }
    if let Some(arr) = infer(state) {
        return Ok(arr);
    }
    let anchor = anchor(state).ok_or(LayoutError::NoScreens)?;
    let mut others: Vec<&Screen> = state.enabled().filter(|s| s.id != anchor.id).collect();
    others.sort_by_key(|s| (s.rect.x, s.rect.y));
    Ok(Arrangement {
        anchor: anchor.id.clone(),
        placements: others
            .iter()
            .map(|s| Placement {
                screen: s.id.clone(),
                side: Side::Right,
            })
            .collect(),
        align: Align::Center,
        aligned: true,
        primary: state
            .primary()
            .map_or_else(|| anchor.id.clone(), |s| s.id.clone()),
    })
}

/// The arrangement to save: the one in force, which must be one Screen Side
/// can describe. Saving a fallback instead would put back something other
/// than what was on screen.
pub fn to_save(state: &State) -> Result<Arrangement, crate::Error> {
    match state.enabled().count() {
        0 => return Err(LayoutError::NoScreens.into()),
        1 => return Err(LayoutError::OnlyOneScreen.into()),
        _ => {}
    }
    match infer(state) {
        Some(arr) if arr.aligned => Ok(arr),
        _ => Err(crate::Error::Usage(
            "The screens overlap or line up in no named way, so there is nothing Screen Side can save. Pick a side and an alignment first, then save."
                .into(),
        )),
    }
}

impl Arrangement {
    pub fn side_of(&self, id: &str) -> Option<Side> {
        self.placements
            .iter()
            .find(|p| p.screen == id)
            .map(|p| p.side)
    }

    /// The side every other screen is on, if they share one.
    pub fn common_side(&self) -> Option<Side> {
        let first = self.placements.first()?.side;
        self.placements
            .iter()
            .all(|p| p.side == first)
            .then_some(first)
    }

    /// Every screen to `side`. Screens nearest the anchor stay nearest, so
    /// moving a whole row is a mirror image rather than a reshuffle.
    pub fn move_all(&self, side: Side) -> Arrangement {
        let mut ranked: Vec<(usize, usize, &Placement)> = self
            .placements
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let rank = self.placements[..i]
                    .iter()
                    .filter(|q| q.side == p.side)
                    .count();
                (rank, i, p)
            })
            .collect();
        ranked.sort_by_key(|(rank, i, _)| (*rank, *i));
        Arrangement {
            placements: ranked
                .into_iter()
                .map(|(_, _, p)| Placement {
                    screen: p.screen.clone(),
                    side,
                })
                .collect(),
            aligned: true,
            ..self.clone()
        }
    }

    /// One screen to `side`, as the farthest on that side. Choosing the
    /// anchor with exactly one other screen means "put the anchor there",
    /// which moves the other screen to the opposite side.
    pub fn move_screen(&self, id: &str, side: Side) -> Result<Arrangement, LayoutError> {
        if id == self.anchor {
            return match self.placements.as_slice() {
                [only] => self.move_screen(&only.screen.clone(), side.mirrored()),
                _ => Err(LayoutError::AnchorSelected),
            };
        }
        let current = self
            .side_of(id)
            .ok_or_else(|| LayoutError::UnknownScreen(id.to_string()))?;
        if current == side {
            return Ok(self.clone());
        }
        let mut placements: Vec<Placement> = self
            .placements
            .iter()
            .filter(|p| p.screen != id)
            .cloned()
            .collect();
        placements.push(Placement {
            screen: id.to_string(),
            side,
        });
        Ok(Arrangement {
            placements,
            aligned: true,
            ..self.clone()
        })
    }

    /// Left with right and above with below, for every screen. When screens
    /// sit on both axes this is a point reflection, so the alignment flips
    /// too (start with end) and the shape stays the same; on one axis the
    /// alignment stays, so a top-aligned monitor stays top-aligned.
    pub fn toggled(&self) -> Arrangement {
        let horizontal = self.placements.iter().any(|p| p.side.horizontal());
        let vertical = self.placements.iter().any(|p| !p.side.horizontal());
        let align = match (horizontal && vertical, self.align) {
            (true, Align::Start) => Align::End,
            (true, Align::End) => Align::Start,
            (_, align) => align,
        };
        Arrangement {
            align,
            placements: self
                .placements
                .iter()
                .map(|p| Placement {
                    screen: p.screen.clone(),
                    side: p.side.mirrored(),
                })
                .collect(),
            aligned: true,
            ..self.clone()
        }
    }

    pub fn with_align(&self, align: Align) -> Arrangement {
        Arrangement {
            align,
            aligned: true,
            ..self.clone()
        }
    }

    pub fn with_primary(&self, id: &str) -> Arrangement {
        Arrangement {
            primary: id.to_string(),
            ..self.clone()
        }
    }
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

    fn placed(mut s: Screen, x: i32, y: i32) -> Screen {
        s.rect.x = x;
        s.rect.y = y;
        s
    }

    #[test]
    fn infer_reads_back_what_compute_wrote() {
        let base = state(vec![
            laptop(1280, 800),
            screen("A", 1920, 1080),
            screen("B", 2560, 1440),
        ]);
        for align in [Align::Start, Align::Center, Align::End] {
            let wanted = arr(&[("A", Side::Left), ("B", Side::Left)], align);
            let layout = compute(&base, &wanted, Origin::TopLeft).unwrap();
            let mut moved = base.clone();
            for s in &mut moved.screens {
                let p = layout.position(&s.id).unwrap();
                s.rect.x = p.x;
                s.rect.y = p.y;
            }
            assert_eq!(infer(&moved).unwrap(), wanted, "{align}");
        }
    }

    #[test]
    fn infer_takes_the_larger_gap_for_a_diagonal_screen() {
        let s = state(vec![
            placed(laptop(1280, 800), 0, 0),
            placed(screen("A", 100, 100), 2000, 900),
        ]);
        assert_eq!(infer(&s).unwrap().side_of("A"), Some(Side::Right));
    }

    #[test]
    fn infer_gives_up_when_a_screen_overlaps_the_anchor() {
        let s = state(vec![
            placed(laptop(1280, 800), 0, 0),
            placed(screen("A", 1920, 1080), 600, 0),
        ]);
        assert_eq!(infer(&s), None);
    }

    #[test]
    fn infer_reports_custom_alignment() {
        let s = state(vec![
            placed(laptop(1280, 800), 0, 100),
            placed(screen("A", 1920, 1080), 1280, 0),
        ]);
        let a = infer(&s).unwrap();
        assert!(!a.aligned);
        assert_eq!(a.align, Align::Center);
    }

    #[test]
    fn infer_tolerates_one_pixel_of_centring() {
        // 1.0 centred against the taller screen, which can differ by one.
        let s = state(vec![
            placed(laptop(1280, 801), 2560, 319),
            placed(screen("A", 2560, 1440), 0, 0),
        ]);
        let a = infer(&s).unwrap();
        assert!(a.aligned);
        assert_eq!(a.align, Align::Center);
    }

    #[test]
    fn infer_ignores_switched_off_screens() {
        let mut lid = laptop(1280, 800);
        lid.enabled = false;
        let mut a = placed(screen("A", 1920, 1080), 0, 0);
        a.primary = true;
        let s = state(vec![lid, a, placed(screen("B", 1920, 1080), 1920, 0)]);
        let inferred = infer(&s).unwrap();
        assert_eq!(inferred.anchor, "A");
        assert_eq!(
            inferred.placements,
            vec![Placement {
                screen: "B".into(),
                side: Side::Right
            }]
        );
    }

    #[test]
    fn baseline_needs_two_screens_and_falls_back_to_the_right() {
        assert_eq!(
            baseline(&state(vec![laptop(1280, 800)])).unwrap_err(),
            LayoutError::OnlyOneScreen
        );
        let overlapping = state(vec![
            placed(laptop(1280, 800), 0, 0),
            placed(screen("A", 1920, 1080), 600, 0),
        ]);
        let b = baseline(&overlapping).unwrap();
        assert_eq!(
            b.placements,
            vec![Placement {
                screen: "A".into(),
                side: Side::Right
            }]
        );
    }

    #[test]
    fn move_all_keeps_the_nearest_screen_nearest() {
        let a = arr(
            &[("A", Side::Right), ("B", Side::Right), ("C", Side::Above)],
            Align::Center,
        );
        let moved = a.move_all(Side::Left);
        let order: Vec<(&str, Side)> = moved
            .placements
            .iter()
            .map(|p| (p.screen.as_str(), p.side))
            .collect();
        assert_eq!(
            order,
            [("A", Side::Left), ("C", Side::Left), ("B", Side::Left)]
        );
    }

    #[test]
    fn move_screen_moves_one_and_puts_it_last() {
        let a = arr(
            &[("A", Side::Left), ("B", Side::Right), ("C", Side::Right)],
            Align::Center,
        );
        let moved = a.move_screen("A", Side::Right).unwrap();
        assert_eq!(moved.side_of("A"), Some(Side::Right));
        assert_eq!(moved.placements.last().unwrap().screen, "A");
        assert_eq!(
            a.move_screen("B", Side::Right).unwrap(),
            a,
            "same side is a no-op"
        );
        assert_eq!(
            a.move_screen("Z", Side::Left).unwrap_err(),
            LayoutError::UnknownScreen("Z".into())
        );
    }

    #[test]
    fn moving_the_anchor_moves_the_only_other_screen_the_other_way() {
        let one = arr(&[("A", Side::Right)], Align::Center);
        assert_eq!(
            one.move_screen("eDP-1", Side::Right).unwrap().side_of("A"),
            Some(Side::Left)
        );
        let two = arr(&[("A", Side::Right), ("B", Side::Left)], Align::Center);
        assert_eq!(
            two.move_screen("eDP-1", Side::Left).unwrap_err(),
            LayoutError::AnchorSelected
        );
    }

    #[test]
    fn toggle_mirrors_every_side() {
        let a = arr(&[("A", Side::Left), ("B", Side::Above)], Align::Center);
        let t = a.toggled();
        assert_eq!(t.side_of("A"), Some(Side::Right));
        assert_eq!(t.side_of("B"), Some(Side::Below));
        assert_eq!(t.toggled(), a);
    }

    #[test]
    fn common_side_only_when_all_agree() {
        assert_eq!(
            arr(&[("A", Side::Left), ("B", Side::Left)], Align::Center).common_side(),
            Some(Side::Left)
        );
        assert_eq!(
            arr(&[("A", Side::Left), ("B", Side::Above)], Align::Center).common_side(),
            None
        );
    }

    #[test]
    fn toggling_both_axes_is_a_true_mirror() {
        // Left and above, top-aligned, mirrors to right and below; keeping
        // "start" would make the two meet in the corner, so it flips too.
        let s = state(vec![
            laptop(1280, 800),
            screen("A", 2560, 1440),
            screen("B", 1920, 1080),
        ]);
        let a = arr(&[("A", Side::Left), ("B", Side::Above)], Align::Start);
        assert!(compute(&s, &a, Origin::TopLeft).is_ok());
        let t = a.toggled();
        assert_eq!(t.align, Align::End);
        assert!(compute(&s, &t, Origin::TopLeft).is_ok());
        // One axis only: the alignment stays, so a top-aligned monitor stays
        // top-aligned when it moves from left to right.
        assert_eq!(
            arr(&[("A", Side::Left)], Align::Start).toggled().align,
            Align::Start
        );
    }

    #[test]
    fn infer_needs_two_screens() {
        assert_eq!(infer(&state(vec![laptop(1280, 800)])), None);
        let mut lid = laptop(1280, 800);
        lid.enabled = false;
        assert_eq!(infer(&state(vec![lid, screen("A", 1920, 1080)])), None);
    }

    #[test]
    fn only_an_arrangement_screen_side_can_describe_is_saved() {
        let side_by_side = state(vec![
            placed(laptop(1280, 800), 2560, 320),
            placed(screen("A", 2560, 1440), 0, 0),
        ]);
        assert!(to_save(&side_by_side).is_ok());
        let overlapping = state(vec![
            placed(laptop(1280, 800), 0, 0),
            placed(screen("A", 1920, 1080), 0, 0),
        ]);
        assert!(to_save(&overlapping).is_err());
        let custom = state(vec![
            placed(laptop(1280, 800), 0, 100),
            placed(screen("A", 1920, 1080), 1280, 0),
        ]);
        let message = to_save(&custom).unwrap_err().to_string();
        assert!(message.contains("Pick a side"), "{message}");
    }
}
