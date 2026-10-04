//! GNOME and other Mutter desktops over the session bus. Cinnamon's Muffin
//! offers the same interface under another name.

use std::collections::HashMap;

use zbus::blocking::Connection;
use zbus::zvariant::{OwnedValue, Value};

use super::mutter::{
    MutterLogical, MutterMode, MutterMonitor, MutterState, LAYOUT_LOGICAL, LAYOUT_PHYSICAL,
};
use super::{ApplyMode, Backend, Capabilities};
use crate::layout::Origin;
use crate::model::{Layout, State};
use crate::Error;

type Spec = (String, String, String, String);
type RawMode = (
    String,
    i32,
    i32,
    f64,
    f64,
    Vec<f64>,
    HashMap<String, OwnedValue>,
);
type RawMonitor = (Spec, Vec<RawMode>, HashMap<String, OwnedValue>);
type RawLogical = (
    i32,
    i32,
    f64,
    u32,
    bool,
    Vec<Spec>,
    HashMap<String, OwnedValue>,
);
type RawState = (
    u32,
    Vec<RawMonitor>,
    Vec<RawLogical>,
    HashMap<String, OwnedValue>,
);
type ApplyMonitor<'a> = (String, String, HashMap<&'a str, Value<'a>>);
type ApplyLogicalRaw<'a> = (i32, i32, f64, u32, bool, Vec<ApplyMonitor<'a>>);

/// True when something answers on `name` on the session bus.
pub fn bus_has_owner(name: &str) -> bool {
    let Ok(conn) = Connection::session() else {
        return false;
    };
    let Ok(proxy) = zbus::blocking::fdo::DBusProxy::new(&conn) else {
        return false;
    };
    let Ok(bus_name) = zbus::names::BusName::try_from(name) else {
        return false;
    };
    proxy.name_has_owner(bus_name).unwrap_or(false)
}

fn flag(props: &HashMap<String, OwnedValue>, key: &str) -> Option<bool> {
    props.get(key).and_then(|v| bool::try_from(v).ok())
}

fn text(props: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    props
        .get(key)
        .and_then(|v| <&str>::try_from(v).ok())
        .map(str::to_string)
}

fn builtin_connector(connector: &str) -> bool {
    let c = connector.to_uppercase();
    c.starts_with("EDP") || c.starts_with("LVDS") || c.starts_with("DSI")
}

pub struct Gnome {
    conn: Connection,
    bus: String,
    path: String,
}

impl Gnome {
    pub fn connect(bus: &str) -> Result<Gnome, Error> {
        let conn = Connection::session()
            .map_err(|e| Error::System(format!("Cannot reach the session bus: {e}")))?;
        Ok(Gnome {
            conn,
            bus: bus.to_string(),
            path: format!("/{}", bus.replace('.', "/")),
        })
    }

    fn raw(&self) -> Result<MutterState, Error> {
        let reply = self
            .conn
            .call_method(
                Some(self.bus.as_str()),
                self.path.as_str(),
                Some(self.bus.as_str()),
                "GetCurrentState",
                &(),
            )
            .map_err(|e| Error::System(format!("{} did not answer: {e}", self.bus)))?;
        let (serial, monitors, logical, props): RawState = reply
            .body()
            .deserialize()
            .map_err(|e| Error::System(format!("Unexpected reply from {}: {e}", self.bus)))?;
        Ok(MutterState {
            serial,
            layout_mode: props
                .get("layout-mode")
                .and_then(|v| u32::try_from(v).ok())
                .unwrap_or(LAYOUT_LOGICAL),
            supports_changing_layout_mode: flag(&props, "supports-changing-layout-mode")
                .unwrap_or(false),
            monitors: monitors
                .into_iter()
                .map(
                    |((connector, vendor, product, serial), modes, mprops)| MutterMonitor {
                        builtin: flag(&mprops, "is-builtin")
                            .unwrap_or_else(|| builtin_connector(&connector)),
                        display_name: text(&mprops, "display-name").unwrap_or_default(),
                        underscanning: flag(&mprops, "is-underscanning"),
                        color_mode: mprops.get("color-mode").and_then(|v| u32::try_from(v).ok()),
                        rgb_range: mprops.get("rgb-range").and_then(|v| u32::try_from(v).ok()),
                        modes: modes
                            .into_iter()
                            .map(
                                |(id, width, height, _refresh, _scale, _scales, p)| MutterMode {
                                    id,
                                    width,
                                    height,
                                    current: flag(&p, "is-current").unwrap_or(false),
                                    preferred: flag(&p, "is-preferred").unwrap_or(false),
                                },
                            )
                            .collect(),
                        connector,
                        vendor,
                        product,
                        serial,
                    },
                )
                .collect(),
            logical: logical
                .into_iter()
                .map(
                    |(x, y, scale, transform, primary, specs, _)| MutterLogical {
                        x,
                        y,
                        scale,
                        transform,
                        primary,
                        connectors: specs.into_iter().map(|s| s.0).collect(),
                    },
                )
                .collect(),
        })
    }
}

impl Backend for Gnome {
    fn name(&self) -> &'static str {
        "gnome"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            primary: true,
            temporary: true,
            verify: true,
            remembers: true,
            confirms: true,
            origin: Origin::TopLeft,
        }
    }

    fn query(&self) -> Result<State, Error> {
        self.raw().map(|raw| raw.to_state())
    }

    fn apply(&self, layout: &Layout, mode: ApplyMode) -> Result<(), Error> {
        let raw = self.raw()?;
        let config = raw.apply_config(layout)?;
        let method: u32 = match mode {
            ApplyMode::Verify => 0,
            ApplyMode::Temporary => 1,
            ApplyMode::Persistent => 2,
        };
        let logical: Vec<ApplyLogicalRaw> = config
            .into_iter()
            .map(|l| {
                let monitors = l
                    .monitors
                    .into_iter()
                    .map(|m| {
                        let mut props: HashMap<&str, Value> = HashMap::new();
                        if let Some(on) = m.underscanning {
                            // GNOME 46 and 47 read the first name, newer
                            // Mutter the second; each ignores the other.
                            props.insert("enable_underscanning", Value::from(on));
                            props.insert("underscanning", Value::from(on));
                        }
                        if let Some(mode) = m.color_mode {
                            props.insert("color-mode", Value::from(mode));
                        }
                        if let Some(range) = m.rgb_range {
                            props.insert("rgb-range", Value::from(range));
                        }
                        (m.connector, m.mode, props)
                    })
                    .collect();
                (l.x, l.y, l.scale, l.transform, l.primary, monitors)
            })
            .collect();
        let mut props: HashMap<&str, Value> = HashMap::new();
        if raw.supports_changing_layout_mode {
            props.insert("layout-mode", Value::from(raw.layout_mode));
        }
        self.conn
            .call_method(
                Some(self.bus.as_str()),
                self.path.as_str(),
                Some(self.bus.as_str()),
                "ApplyMonitorsConfig",
                &(raw.serial, method, logical, props),
            )
            .map(|_| ())
            .map_err(|e| Error::System(format!("Mutter refused the arrangement: {e}")))
    }

    fn diagnostics(&self) -> Vec<(String, String)> {
        let mut lines = vec![("bus".to_string(), self.bus.clone())];
        if let Ok(raw) = self.raw() {
            let mode = if raw.layout_mode == LAYOUT_PHYSICAL {
                "physical"
            } else {
                "logical"
            };
            lines.push(("layout mode".into(), mode.into()));
        }
        lines
    }
}
