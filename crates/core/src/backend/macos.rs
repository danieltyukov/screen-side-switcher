//! macOS through Quartz Display Services. Names come from NSScreen, which
//! may only be asked on the main thread; elsewhere a generic name is used.

use core_graphics::display::{CGConfigureOption, CGDisplay};
use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_foundation::{NSNumber, NSString};

use super::quartz::{to_state, QuartzDisplay};
use super::{ApplyMode, Backend, Capabilities};
use crate::layout::Origin;
use crate::model::{Layout, State};
use crate::Error;

pub struct Macos;

/// (CGDirectDisplayID, localized name) for every screen AppKit knows.
fn names() -> Vec<(u32, String)> {
    let Some(mtm) = MainThreadMarker::new() else {
        return Vec::new();
    };
    let key = NSString::from_str("NSScreenNumber");
    NSScreen::screens(mtm)
        .iter()
        .filter_map(|screen| {
            let description = screen.deviceDescription();
            let number = description
                .objectForKey(&key)?
                .downcast::<NSNumber>()
                .ok()?;
            Some((
                number.unsignedIntValue(),
                screen.localizedName().to_string(),
            ))
        })
        .collect()
}

fn displays() -> Result<Vec<QuartzDisplay>, Error> {
    let ids = CGDisplay::active_displays()
        .map_err(|e| Error::System(format!("Quartz could not list the displays ({e}).")))?;
    let names = names();
    Ok(ids
        .into_iter()
        .map(|id| {
            let d = CGDisplay::new(id);
            let b = d.bounds();
            QuartzDisplay {
                id,
                x: b.origin.x,
                y: b.origin.y,
                width: b.size.width,
                height: b.size.height,
                builtin: d.is_builtin(),
                main: d.is_main(),
                vendor: d.vendor_number(),
                model: d.model_number(),
                serial: d.serial_number(),
                pixels_wide: d.pixels_wide(),
                mirror_of: d.mirrors_display(),
                name: names
                    .iter()
                    .find(|(n, _)| *n == id)
                    .map(|(_, name)| name.clone()),
            }
        })
        .collect())
}

impl Backend for Macos {
    fn name(&self) -> &'static str {
        "macos"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            primary: true,
            temporary: true,
            verify: false,
            remembers: true,
            confirms: false,
            origin: Origin::Primary,
        }
    }

    fn query(&self) -> Result<State, Error> {
        Ok(to_state(&displays()?))
    }

    fn apply(&self, layout: &Layout, mode: ApplyMode) -> Result<(), Error> {
        let state = to_state(&displays()?);
        super::ensure_same_screens(&state, layout)?;
        let main = CGDisplay::main();
        let config = main
            .begin_configuration()
            .map_err(|e| Error::System(format!("Quartz error {e}.")))?;
        for p in &layout.positions {
            let id: u32 = p.id.parse().map_err(|_| Error::Changed)?;
            if let Err(e) = CGDisplay::new(id).configure_display_origin(&config, p.x, p.y) {
                let _ = main.cancel_configuration(&config);
                return Err(Error::System(format!(
                    "macOS refused the position of display {id} ({e})."
                )));
            }
        }
        let option = match mode {
            ApplyMode::Persistent => CGConfigureOption::ConfigurePermanently,
            _ => CGConfigureOption::ConfigureForSession,
        };
        main.complete_configuration(&config, option)
            .map_err(|e| Error::System(format!("macOS refused the arrangement ({e}).")))
    }
}
