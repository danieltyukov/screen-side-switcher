//! Saved layouts and app settings, in the OS config directory.
//!
//! A layout is saved as intent (sides, order, alignment, primary) against
//! the identities of the screens connected when it was saved, never as
//! coordinates, so it survives a resolution or scale change.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::layout::{Align, Arrangement, Placement, Side};
use crate::model::State;
use crate::Error;

const LAYOUTS: &str = "layouts.json";
const SETTINGS: &str = "settings.json";
const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedPlacement {
    pub screen: String,
    pub side: Side,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedLayout {
    pub name: String,
    pub screens: Vec<String>,
    pub anchor: String,
    pub placements: Vec<SavedPlacement>,
    pub align: Align,
    pub primary: String,
    #[serde(default)]
    pub auto: bool,
    #[serde(default)]
    pub saved: String,
}

impl SavedLayout {
    /// One line for lists: "2 screens: 1 left; bottom".
    pub fn summary(&self) -> String {
        let mut counts: Vec<(Side, usize)> = Vec::new();
        for p in &self.placements {
            match counts.iter_mut().find(|(s, _)| *s == p.side) {
                Some((_, n)) => *n += 1,
                None => counts.push((p.side, 1)),
            }
        }
        let sides: Vec<String> = counts.iter().map(|(s, n)| format!("{n} {s}")).collect();
        let horizontal = self.placements.first().is_none_or(|p| p.side.horizontal());
        format!(
            "{} screens: {}; {}",
            self.screens.len(),
            sides.join(", "),
            self.align.label(horizontal)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub background: bool,
    pub auto_apply: bool,
    pub shortcut: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            background: cfg!(any(windows, target_os = "macos")),
            auto_apply: true,
            shortcut: None,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct LayoutsFile {
    version: u32,
    layouts: Vec<SavedLayout>,
}

#[derive(Serialize, Deserialize)]
struct SettingsFile {
    version: u32,
    #[serde(flatten)]
    settings: Settings,
}

/// Sorted identity keys of the enabled screens: which desk this is.
pub fn fingerprint(state: &State) -> Vec<String> {
    let mut keys: Vec<String> = state.enabled().map(|s| s.key()).collect();
    keys.sort();
    keys
}

/// Each enabled screen's key, made unique: the second identical monitor
/// (by connector) gets `#2`, the third `#3`.
pub fn keyed(state: &State) -> Vec<(String, String)> {
    let mut screens: Vec<_> = state.enabled().collect();
    screens.sort_by(|a, b| a.connector.cmp(&b.connector));
    let mut seen: HashMap<String, usize> = HashMap::new();
    screens
        .into_iter()
        .map(|s| {
            let key = s.key();
            let n = seen.entry(key.clone()).or_insert(0);
            *n += 1;
            let unique = if *n == 1 { key } else { format!("{key}#{n}") };
            (unique, s.id.clone())
        })
        .collect()
}

fn clean_name(name: &str) -> Result<String, Error> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) {
        return Err(Error::Usage(
            "A layout name needs 1 to 64 characters and no control characters.".into(),
        ));
    }
    Ok(name.to_string())
}

pub fn capture(
    name: &str,
    state: &State,
    arr: &Arrangement,
    auto: bool,
) -> Result<SavedLayout, Error> {
    let name = clean_name(name)?;
    let keys = keyed(state);
    let key_of = |id: &str| {
        keys.iter()
            .find(|(_, i)| i == id)
            .map(|(k, _)| k.clone())
            .ok_or_else(|| Error::Usage(format!("{id} is not switched on.")))
    };
    Ok(SavedLayout {
        name,
        screens: fingerprint(state),
        anchor: key_of(&arr.anchor)?,
        placements: arr
            .placements
            .iter()
            .map(|p| {
                Ok(SavedPlacement {
                    screen: key_of(&p.screen)?,
                    side: p.side,
                })
            })
            .collect::<Result<_, Error>>()?,
        align: arr.align,
        primary: key_of(&arr.primary)?,
        auto,
        saved: humantime::format_rfc3339_seconds(SystemTime::now()).to_string(),
    })
}

pub fn matches(saved: &SavedLayout, state: &State) -> bool {
    saved.screens == fingerprint(state)
}

pub fn resolve(saved: &SavedLayout, state: &State) -> Result<Arrangement, Error> {
    if !matches(saved, state) {
        return Err(Error::Usage(format!(
            "'{}' was saved for other screens than the ones connected now.",
            saved.name
        )));
    }
    let keys = keyed(state);
    let id_of = |key: &str| {
        keys.iter()
            .find(|(k, _)| k == key)
            .map(|(_, id)| id.clone())
            .ok_or_else(|| {
                Error::Usage(format!(
                    "'{}' names a screen that is not connected.",
                    saved.name
                ))
            })
    };
    Ok(Arrangement {
        anchor: id_of(&saved.anchor)?,
        placements: saved
            .placements
            .iter()
            .map(|p| {
                Ok(Placement {
                    screen: id_of(&p.screen)?,
                    side: p.side,
                })
            })
            .collect::<Result<_, Error>>()?,
        align: saved.align,
        aligned: true,
        primary: id_of(&saved.primary)?,
    })
}

pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn open() -> Result<Store, Error> {
        if let Some(dir) = std::env::var_os("SCREEN_SIDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
            return Ok(Store::at(PathBuf::from(dir)));
        }
        let dirs = directories::ProjectDirs::from("io.github", "danieltyukov", "ScreenSide")
            .ok_or_else(|| {
                Error::Config("There is no home directory to keep settings in.".into())
            })?;
        Ok(Store::at(dirs.config_dir()))
    }

    pub fn at(dir: impl Into<PathBuf>) -> Store {
        Store { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn read<T: for<'de> Deserialize<'de>>(
        &self,
        file: &str,
        version: impl Fn(&T) -> u32,
    ) -> Result<Option<T>, Error> {
        let path = self.dir.join(file);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let value: T = serde_json::from_str(&text).map_err(|e| {
            Error::Config(format!(
                "{} could not be read ({e}). Fix or remove it; it has not been changed.",
                path.display()
            ))
        })?;
        if version(&value) > VERSION {
            return Err(Error::Config(format!(
                "{} was written by a newer Screen Side. Update Screen Side to use it; it has not been changed.",
                path.display()
            )));
        }
        Ok(Some(value))
    }

    fn write(&self, file: &str, text: String) -> Result<(), Error> {
        std::fs::create_dir_all(&self.dir)?;
        let path = self.dir.join(file);
        let tmp = self.dir.join(format!("{file}.tmp"));
        std::fs::write(&tmp, text + "\n")?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    pub fn layouts(&self) -> Result<Vec<SavedLayout>, Error> {
        Ok(self
            .read::<LayoutsFile>(LAYOUTS, |f| f.version)?
            .map(|f| f.layouts)
            .unwrap_or_default())
    }

    fn put_all(&self, layouts: Vec<SavedLayout>) -> Result<(), Error> {
        let text = serde_json::to_string_pretty(&LayoutsFile {
            version: VERSION,
            layouts,
        })
        .expect("serialisable");
        self.write(LAYOUTS, text)
    }

    pub fn find(&self, name: &str) -> Result<SavedLayout, Error> {
        let layouts = self.layouts()?;
        layouts
            .iter()
            .find(|l| l.name.eq_ignore_ascii_case(name.trim()))
            .cloned()
            .ok_or_else(|| {
                let names: Vec<&str> = layouts.iter().map(|l| l.name.as_str()).collect();
                Error::Usage(if names.is_empty() {
                    format!(
                        "There is no saved layout called '{}'. No layouts are saved yet.",
                        name.trim()
                    )
                } else {
                    format!(
                        "There is no saved layout called '{}'. Saved: {}.",
                        name.trim(),
                        names.join(", ")
                    )
                })
            })
    }

    pub fn put(&self, layout: SavedLayout) -> Result<(), Error> {
        let mut layouts = self.layouts()?;
        match layouts
            .iter_mut()
            .find(|l| l.name.eq_ignore_ascii_case(&layout.name))
        {
            Some(existing) => *existing = layout,
            None => layouts.push(layout),
        }
        self.put_all(layouts)
    }

    pub fn forget(&self, name: &str) -> Result<(), Error> {
        let found = self.find(name)?;
        let layouts = self
            .layouts()?
            .into_iter()
            .filter(|l| l.name != found.name)
            .collect();
        self.put_all(layouts)
    }

    pub fn set_auto(&self, name: &str, auto: bool) -> Result<(), Error> {
        let mut found = self.find(name)?;
        found.auto = auto;
        self.put(found)
    }

    pub fn settings(&self) -> Result<Settings, Error> {
        Ok(self
            .read::<SettingsFile>(SETTINGS, |f| f.version)?
            .map(|f| f.settings)
            .unwrap_or_default())
    }

    pub fn put_settings(&self, settings: &Settings) -> Result<(), Error> {
        let text = serde_json::to_string_pretty(&SettingsFile {
            version: VERSION,
            settings: settings.clone(),
        })
        .expect("serialisable");
        self.write(SETTINGS, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::tests::{laptop, screen, state};
    use crate::model::{Identity, Screen};

    fn monitor(id: &str, vendor: &str, product: &str, serial: &str) -> Screen {
        let mut s = screen(id, 1920, 1080);
        s.identity = Identity {
            vendor: vendor.into(),
            product: product.into(),
            serial: serial.into(),
        };
        s
    }

    fn desk() -> State {
        let mut l = laptop(1280, 800);
        l.identity = Identity {
            vendor: "BOE".into(),
            product: "0x095f".into(),
            serial: String::new(),
        };
        state(vec![l, monitor("HDMI-1", "DEL", "0x41b5", "ABC")])
    }

    fn left_of() -> Arrangement {
        Arrangement {
            anchor: "eDP-1".into(),
            placements: vec![Placement {
                screen: "HDMI-1".into(),
                side: Side::Left,
            }],
            align: Align::End,
            aligned: true,
            primary: "HDMI-1".into(),
        }
    }

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::at(dir.path().join("config"));
        (dir, store)
    }

    #[test]
    fn capture_and_resolve_round_trip() {
        let saved = capture("office", &desk(), &left_of(), false).unwrap();
        assert_eq!(saved.screens, ["boe:0x095f:", "del:0x41b5:abc"]);
        assert_eq!(saved.anchor, "boe:0x095f:");
        assert_eq!(saved.primary, "del:0x41b5:abc");
        assert!(matches(&saved, &desk()));
        assert_eq!(resolve(&saved, &desk()).unwrap(), left_of());
    }

    #[test]
    fn resolve_on_other_screens_is_a_usage_error() {
        let saved = capture("office", &desk(), &left_of(), false).unwrap();
        let mut elsewhere = desk();
        elsewhere.screens[1].identity.serial = "XYZ".into();
        assert!(!matches(&saved, &elsewhere));
        match resolve(&saved, &elsewhere) {
            Err(Error::Usage(m)) => {
                assert!(m.contains("office"));
                assert!(m.contains("other screens"));
            }
            other => panic!("expected a usage error, got {other:?}"),
        }
    }

    #[test]
    fn identical_monitors_resolve_in_connector_order() {
        let s = state(vec![
            laptop(1280, 800),
            monitor("DP-2", "DEL", "0x41b5", ""),
            monitor("DP-1", "DEL", "0x41b5", ""),
        ]);
        let arr = Arrangement {
            anchor: "eDP-1".into(),
            placements: vec![
                Placement {
                    screen: "DP-1".into(),
                    side: Side::Left,
                },
                Placement {
                    screen: "DP-2".into(),
                    side: Side::Right,
                },
            ],
            align: Align::Center,
            aligned: true,
            primary: "eDP-1".into(),
        };
        let saved = capture("twins", &s, &arr, false).unwrap();
        let keys: Vec<&str> = saved.placements.iter().map(|p| p.screen.as_str()).collect();
        assert_eq!(keys, ["del:0x41b5:", "del:0x41b5:#2"]);
        assert_eq!(
            fingerprint(&s)
                .iter()
                .filter(|k| *k == "del:0x41b5:")
                .count(),
            2
        );
        assert_eq!(resolve(&saved, &s).unwrap(), arr);
    }

    #[test]
    fn fingerprint_ignores_switched_off_screens() {
        let mut s = desk();
        s.screens[0].enabled = false;
        assert_eq!(fingerprint(&s), ["del:0x41b5:abc"]);
    }

    #[test]
    fn names_are_checked() {
        assert!(matches!(
            capture("", &desk(), &left_of(), false),
            Err(Error::Usage(_))
        ));
        assert!(matches!(
            capture(&"x".repeat(65), &desk(), &left_of(), false),
            Err(Error::Usage(_))
        ));
        let saved = capture("  Mum's desk  ", &desk(), &left_of(), false).unwrap();
        assert_eq!(saved.name, "Mum's desk");
    }

    #[test]
    fn store_round_trip_and_replace_by_name() {
        let (_dir, store) = store();
        store
            .put(capture("office", &desk(), &left_of(), false).unwrap())
            .unwrap();
        store
            .put(capture("Office", &desk(), &left_of(), true).unwrap())
            .unwrap();
        let layouts = store.layouts().unwrap();
        assert_eq!(layouts.len(), 1);
        assert!(layouts[0].auto);
        assert_eq!(store.find("OFFICE").unwrap().name, "Office");
        store.set_auto("office", false).unwrap();
        assert!(!store.find("office").unwrap().auto);
        store.forget("office").unwrap();
        assert!(store.layouts().unwrap().is_empty());
        match store.forget("nope") {
            Err(Error::Usage(m)) => assert!(m.contains("No layouts are saved yet")),
            other => panic!("expected a usage error, got {other:?}"),
        }
    }

    #[test]
    fn missing_file_means_no_layouts() {
        let (_dir, store) = store();
        assert!(store.layouts().unwrap().is_empty());
    }

    #[test]
    fn corrupt_file_is_refused_and_kept() {
        let (_dir, store) = store();
        std::fs::create_dir_all(store.dir()).unwrap();
        let path = store.dir().join("layouts.json");
        std::fs::write(&path, "{ not json").unwrap();
        match store.layouts() {
            Err(Error::Config(m)) => {
                assert!(m.contains(&path.display().to_string()));
                assert!(m.contains("Fix or remove"));
            }
            other => panic!("expected a config error, got {other:?}"),
        }
        assert!(store
            .put(capture("office", &desk(), &left_of(), false).unwrap())
            .is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
    }

    #[test]
    fn newer_version_is_refused_and_kept() {
        let (_dir, store) = store();
        std::fs::create_dir_all(store.dir()).unwrap();
        let path = store.dir().join("layouts.json");
        let text = r#"{"version": 2, "layouts": []}"#;
        std::fs::write(&path, text).unwrap();
        match store.layouts() {
            Err(Error::Config(m)) => assert!(m.contains("newer Screen Side")),
            other => panic!("expected a config error, got {other:?}"),
        }
        assert!(store
            .put(capture("office", &desk(), &left_of(), false).unwrap())
            .is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }

    #[test]
    fn writes_are_atomic() {
        let (_dir, store) = store();
        store
            .put(capture("office", &desk(), &left_of(), false).unwrap())
            .unwrap();
        let leftovers: Vec<_> = std::fs::read_dir(store.dir())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn settings_default_and_round_trip() {
        let (_dir, store) = store();
        assert_eq!(store.settings().unwrap(), Settings::default());
        let settings = Settings {
            shortcut: Some("<Super><Alt>s".into()),
            ..Settings::default()
        };
        store.put_settings(&settings).unwrap();
        assert_eq!(store.settings().unwrap(), settings);
    }

    #[test]
    fn summary_describes_the_layout() {
        let summary = capture("office", &desk(), &left_of(), false)
            .unwrap()
            .summary();
        assert!(summary.contains("left"), "{summary}");
        assert!(summary.contains("bottom"), "{summary}");
    }
}
