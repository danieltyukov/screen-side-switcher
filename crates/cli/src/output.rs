//! What the command prints, as text for people and JSON for scripts.

use screen_side_core::backend::{Applied, Backend, Capabilities};
use screen_side_core::layout::{infer, Arrangement, Side};
use screen_side_core::model::{Layout, State};
use screen_side_core::watch::Event;
use serde_json::{json, Value};

fn relation(side: Side) -> &'static str {
    match side {
        Side::Left => "left of",
        Side::Right => "right of",
        Side::Above => "above",
        Side::Below => "below",
    }
}

fn edges(side: Side) -> (&'static str, &'static str) {
    match side {
        Side::Left => ("left", "right"),
        Side::Right => ("right", "left"),
        Side::Above => ("top", "bottom"),
        Side::Below => ("bottom", "top"),
    }
}

fn name<'a>(state: &'a State, id: &'a str) -> &'a str {
    state.screen(id).map_or(id, |s| s.name.as_str())
}

fn anchor_text(state: &State, arr: &Arrangement) -> String {
    match state.screen(&arr.anchor) {
        Some(a) if a.builtin => "the built-in screen".to_string(),
        Some(a) => a.name.clone(),
        None => arr.anchor.clone(),
    }
}

/// One sentence saying where the screens are, as `status` opens with.
pub fn headline(state: &State, arr: Option<&Arrangement>) -> String {
    match state.enabled().count() {
        0 => return "No screen is switched on.".into(),
        1 => return "Only one screen is switched on.".into(),
        _ => {}
    }
    let Some(arr) = arr else {
        return "The arrangement is not a simple side-by-side layout.".into();
    };
    let anchor = anchor_text(state, arr);
    let builtin = state.screen(&arr.anchor).is_some_and(|a| a.builtin);
    let mut text = match arr.placements.as_slice() {
        [only] if builtin => format!("External screen is {} {anchor}.", relation(only.side)),
        placements => {
            let clauses: Vec<String> = placements
                .iter()
                .map(|p| {
                    format!(
                        "{} is {} {anchor}",
                        name(state, &p.screen),
                        relation(p.side)
                    )
                })
                .collect();
            format!("{}.", clauses.join("; "))
        }
    };
    if let Some(first) = arr.placements.first() {
        let label = if arr.aligned {
            arr.align.label(first.side.horizontal())
        } else {
            "custom"
        };
        text.push_str(&format!(" Alignment: {label}."));
    }
    text
}

/// Where the pointer crosses, when one screen sits beside the anchor.
fn pointer(state: &State, arr: &Arrangement) -> Option<String> {
    let [only] = arr.placements.as_slice() else {
        return None;
    };
    let (from, to) = edges(only.side);
    Some(format!(
        "The pointer leaves the {from} edge of {} and enters the {to} edge of {}.",
        anchor_text(state, arr),
        name(state, &only.screen)
    ))
}

fn screen_lines(state: &State) -> String {
    let mut out = String::new();
    for (i, s) in state.listed().iter().enumerate() {
        let mut line = format!("  {:>2}  {:<10} {:<24}", i + 1, s.connector, s.name);
        if s.enabled {
            line.push_str(&format!(
                " {:<10} at {:<10} scale {}",
                format!("{}x{}", s.rect.width, s.rect.height),
                format!("{},{}", s.rect.x, s.rect.y),
                s.scale
            ));
            if s.primary {
                line.push_str("  primary");
            }
        } else {
            line.push_str(" off");
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

pub fn status_text(
    state: &State,
    arr: Option<&Arrangement>,
    backend: &str,
    reason: &str,
    layout: Option<&str>,
) -> String {
    let mut out = format!("{}\n{}", headline(state, arr), screen_lines(state));
    out.push_str(&format!("Backend: {backend} ({reason})."));
    if let Some(layout) = layout {
        out.push_str(&format!(" Saved layout in force: {layout}."));
    }
    out.push('\n');
    out
}

fn screens_json(state: &State) -> Value {
    Value::Array(
        state
            .listed()
            .iter()
            .enumerate()
            .map(|(i, s)| {
                json!({
                    "number": i + 1,
                    "id": s.id,
                    "connector": s.connector,
                    "name": s.name,
                    "builtin": s.builtin,
                    "enabled": s.enabled,
                    "primary": s.primary,
                    "x": s.rect.x,
                    "y": s.rect.y,
                    "width": s.rect.width,
                    "height": s.rect.height,
                    "scale": s.scale,
                })
            })
            .collect(),
    )
}

pub fn status_json(
    state: &State,
    arr: Option<&Arrangement>,
    backend: &dyn Backend,
    layout: Option<&str>,
) -> Value {
    json!({
        "schema": 1,
        "backend": backend.name(),
        "capabilities": backend.capabilities(),
        "screens": screens_json(state),
        "arrangement": arr,
        "layout": layout,
    })
}

pub fn dry_run_json(layout: &Layout, applied: Applied) -> Value {
    json!({
        "schema": 1,
        "dry_run": true,
        "checked": applied == Applied::Checked,
        "positions": layout.positions,
        "primary": layout.primary,
    })
}

/// The state as it will be once `layout` is in force.
fn after(before: &State, layout: &Layout, caps: Capabilities) -> State {
    let mut state = before.clone();
    for s in &mut state.screens {
        if let Some(p) = layout.position(&s.id) {
            s.rect.x = p.x;
            s.rect.y = p.y;
        }
        if caps.primary {
            s.primary = s.enabled && s.id == layout.primary;
        }
    }
    state
}

pub fn moved_text(before: &State, layout: &Layout, applied: Applied, caps: Capabilities) -> String {
    let now = after(before, layout, caps);
    let arr = infer(&now);
    let mut out = String::new();
    if applied != Applied::Done {
        out.push_str(match applied {
            Applied::Checked => "Dry run, checked by the system. Nothing changed.\n",
            _ => "Dry run, computed only: this system cannot check a layout without applying it. Nothing changed.\n",
        });
        for p in &layout.positions {
            out.push_str(&format!("  {:<10} at {},{}\n", p.id, p.x, p.y));
        }
    }
    out.push_str(&headline(&now, arr.as_ref()));
    out.push('\n');
    if let Some(line) = arr.as_ref().and_then(|a| pointer(&now, a)) {
        out.push_str(&line);
        out.push('\n');
    }
    let was = before.primary().map(|s| s.id.as_str());
    if caps.primary && was != Some(layout.primary.as_str()) {
        out.push_str(&format!(
            "{} is the primary screen.\n",
            name(&now, &layout.primary)
        ));
    }
    out
}

/// A line for `watch`, or nothing for events not worth a line.
pub fn event_text(event: &Event) -> Option<String> {
    match event {
        Event::ScreensChanged { screens } => {
            Some(format!("Screens changed: {} switched on.", screens.len()))
        }
        Event::Moved => None,
        Event::Applied { name } => Some(format!("Applied '{name}'.")),
        Event::Failed { message } => Some(format!("Problem: {message}")),
    }
}
