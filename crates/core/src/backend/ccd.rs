//! Windows' Connecting and Configuring Displays (CCD) data as plain values.
//! Desktop coordinates are physical pixels, and Windows defines the primary
//! display as the one at (0, 0). No Windows API here, so this compiles and
//! is tested everywhere.

use crate::model::{Identity, Rect, Screen, State};

/// LVDS, DisplayPort embedded, UDI embedded and "internal" output
/// technologies (DISPLAYCONFIG_VIDEO_OUTPUT_TECHNOLOGY).
pub const BUILTIN_TECHNOLOGIES: [i32; 4] = [6, 11, 13, 0x8000_0000u32 as i32];

#[derive(Debug, Clone, PartialEq)]
pub struct CcdPath {
    pub adapter_low: u32,
    pub adapter_high: i32,
    pub source_id: u32,
    pub target_id: u32,
    pub technology: i32,
    /// The source mode's desktop rectangle.
    pub source: Option<Rect>,
    /// `\\.\DISPLAY1`
    pub gdi_name: String,
    pub friendly_name: String,
    /// (edidManufactureId as Windows stores it, edidProductCodeId)
    pub edid: Option<(u16, u16)>,
}

/// EDID keeps the PnP id big-endian; Windows hands it over byte-swapped.
pub fn pnp_id(raw: u16) -> String {
    let id = raw.swap_bytes();
    [10u16, 5, 0]
        .iter()
        .map(|s| (((id >> s) & 0x1f) as u8 + b'A' - 1) as char)
        .collect()
}

pub fn screen_id(p: &CcdPath) -> String {
    format!(
        "{:08x}{:08x}-{}",
        p.adapter_high as u32, p.adapter_low, p.target_id
    )
}

pub fn to_state(paths: &[CcdPath]) -> State {
    let mut seen_sources: Vec<(u32, i32, u32)> = Vec::new();
    let screens = paths
        .iter()
        .map(|p| {
            let source_key = (p.adapter_low, p.adapter_high, p.source_id);
            let first = !seen_sources.contains(&source_key);
            seen_sources.push(source_key);
            let builtin = BUILTIN_TECHNOLOGIES.contains(&p.technology);
            let enabled = first && p.source.is_some();
            let rect = if enabled {
                p.source.unwrap_or_default()
            } else {
                Rect::default()
            };
            let (vendor, product) = p.edid.map_or((String::new(), String::new()), |(m, prod)| {
                (pnp_id(m), format!("0x{prod:04x}"))
            });
            Screen {
                id: screen_id(p),
                connector: p.gdi_name.trim_start_matches(r"\\.\").to_string(),
                name: if !p.friendly_name.trim().is_empty() {
                    p.friendly_name.trim().to_string()
                } else if builtin {
                    "Built-in display".into()
                } else {
                    format!("Display {}", p.target_id)
                },
                identity: Identity {
                    vendor,
                    product,
                    serial: String::new(),
                },
                builtin,
                enabled,
                primary: enabled && rect.x == 0 && rect.y == 0,
                rect,
                scale: 1.0,
            }
        })
        .collect();
    State {
        backend: "windows".into(),
        screens,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn internal() -> CcdPath {
        CcdPath {
            adapter_low: 0xa1b2c,
            adapter_high: 0,
            source_id: 0,
            target_id: 4352,
            technology: 0x8000_0000u32 as i32,
            source: Some(Rect::new(0, 0, 2880, 1800)),
            gdi_name: r"\\.\DISPLAY1".into(),
            friendly_name: String::new(),
            edid: None,
        }
    }

    fn external() -> CcdPath {
        CcdPath {
            adapter_low: 0xa1b2c,
            adapter_high: 0,
            source_id: 1,
            target_id: 4357,
            technology: 5,
            source: Some(Rect::new(-2560, -200, 2560, 1440)),
            gdi_name: r"\\.\DISPLAY2".into(),
            friendly_name: "DELL U2723QE".into(),
            edid: Some((0xAC10, 0x41B5)),
        }
    }

    #[test]
    fn pnp_id_unswaps_the_bytes() {
        assert_eq!(pnp_id(0xAC10), "DEL");
        assert_eq!(pnp_id(0xE509), "BOE");
    }

    #[test]
    fn screen_id_is_adapter_and_target() {
        assert_eq!(screen_id(&external()), "00000000000a1b2c-4357");
    }

    #[test]
    fn to_state_reads_names_identities_and_the_primary() {
        let state = to_state(&[internal(), external()]);
        assert_eq!(state.backend, "windows");
        let laptop = state.screen("00000000000a1b2c-4352").unwrap();
        assert!(laptop.builtin && laptop.primary && laptop.enabled);
        assert_eq!(laptop.name, "Built-in display");
        assert_eq!(laptop.connector, "DISPLAY1");
        assert_eq!(laptop.key(), "connector:display1");
        let dell = state.screen("00000000000a1b2c-4357").unwrap();
        assert_eq!(dell.name, "DELL U2723QE");
        assert_eq!(dell.key(), "del:0x41b5:");
        assert_eq!(dell.rect, Rect::new(-2560, -200, 2560, 1440));
        assert!(!dell.primary && !dell.builtin);
    }

    #[test]
    fn cloned_sources_list_the_second_as_off() {
        let mut clone = external();
        clone.source_id = 0;
        clone.target_id = 4360;
        let state = to_state(&[internal(), clone]);
        assert!(state.screens[0].enabled);
        assert!(!state.screens[1].enabled);
    }

    #[test]
    fn every_builtin_technology_counts() {
        for technology in BUILTIN_TECHNOLOGIES {
            let p = CcdPath {
                technology,
                ..external()
            };
            assert!(to_state(&[p]).screens[0].builtin, "{technology}");
        }
    }
}
