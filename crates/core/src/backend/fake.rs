//! A backend made of a JSON file. For tests, CI runners with no screens,
//! and contributors with a single monitor.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::{ApplyMode, Backend, Capabilities};
use crate::layout::Origin;
use crate::model::{Identity, Layout, Rect, Screen, State};
use crate::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FakeFile {
    pub capabilities: Capabilities,
    pub screens: Vec<Screen>,
    #[serde(default)]
    pub last_apply: Option<ApplyMode>,
}

impl FakeFile {
    /// A 1280x800 laptop at scale 1.5 to the right of a 2560x1440 monitor.
    pub fn sample() -> FakeFile {
        FakeFile {
            capabilities: Capabilities {
                primary: true,
                temporary: true,
                verify: true,
                remembers: true,
                confirms: false,
                origin: Origin::TopLeft,
            },
            screens: vec![
                Screen {
                    id: "eDP-1".into(),
                    connector: "eDP-1".into(),
                    name: "Built-in display".into(),
                    identity: Identity {
                        vendor: "BOE".into(),
                        product: "0x095f".into(),
                        serial: String::new(),
                    },
                    builtin: true,
                    enabled: true,
                    primary: true,
                    rect: Rect::new(2560, 320, 1280, 800),
                    scale: 1.5,
                },
                Screen {
                    id: "HDMI-1".into(),
                    connector: "HDMI-1".into(),
                    name: "DELL U2723QE".into(),
                    identity: Identity {
                        vendor: "DEL".into(),
                        product: "0x41b5".into(),
                        serial: "FAKE0001".into(),
                    },
                    builtin: false,
                    enabled: true,
                    primary: false,
                    rect: Rect::new(0, 0, 2560, 1440),
                    scale: 1.0,
                },
            ],
            last_apply: None,
        }
    }
}

pub struct Fake {
    path: Option<PathBuf>,
    memory: Mutex<FakeFile>,
}

impl Fake {
    /// `SCREEN_SIDE_FAKE_STATE` if set, else the sample in memory.
    pub fn from_env() -> Result<Fake, Error> {
        Ok(match std::env::var_os("SCREEN_SIDE_FAKE_STATE") {
            Some(path) if !path.is_empty() => Fake::at(PathBuf::from(path)),
            _ => Fake::in_memory(FakeFile::sample()),
        })
    }

    pub fn at(path: PathBuf) -> Fake {
        Fake {
            path: Some(path),
            memory: Mutex::new(FakeFile::sample()),
        }
    }

    pub fn in_memory(file: FakeFile) -> Fake {
        Fake {
            path: None,
            memory: Mutex::new(file),
        }
    }

    /// Swaps the screens, as a hotplug would.
    pub fn replace(&self, file: FakeFile) {
        match &self.path {
            Some(_) => {
                let _ = self.store(&file);
            }
            None => *self.memory.lock().unwrap() = file,
        }
    }

    pub(crate) fn load(&self) -> Result<FakeFile, Error> {
        let Some(path) = &self.path else {
            return Ok(self.memory.lock().unwrap().clone());
        };
        if !path.exists() {
            let sample = FakeFile::sample();
            self.store(&sample)?;
            return Ok(sample);
        }
        let text = std::fs::read_to_string(path)?;
        serde_json::from_str(&text).map_err(|e| Error::Config(format!("{}: {e}", path.display())))
    }

    fn store(&self, file: &FakeFile) -> Result<(), Error> {
        match &self.path {
            Some(path) => {
                let text = serde_json::to_string_pretty(file).expect("serialisable");
                std::fs::write(path, text + "\n")?;
            }
            None => *self.memory.lock().unwrap() = file.clone(),
        }
        Ok(())
    }
}

impl Backend for Fake {
    fn name(&self) -> &'static str {
        "fake"
    }

    fn capabilities(&self) -> Capabilities {
        self.load()
            .map(|f| f.capabilities)
            .unwrap_or(FakeFile::sample().capabilities)
    }

    fn query(&self) -> Result<State, Error> {
        Ok(State {
            backend: "fake".into(),
            screens: self.load()?.screens,
        })
    }

    fn apply(&self, layout: &Layout, mode: ApplyMode) -> Result<(), Error> {
        let mut file = self.load()?;
        super::ensure_same_screens(
            &State {
                backend: "fake".into(),
                screens: file.screens.clone(),
            },
            layout,
        )?;
        if mode == ApplyMode::Verify {
            return Ok(());
        }
        let primary_supported = file.capabilities.primary;
        for s in &mut file.screens {
            if let Some(p) = layout.position(&s.id) {
                s.rect.x = p.x;
                s.rect.y = p.y;
            }
            if primary_supported {
                s.primary = s.enabled && s.id == layout.primary;
            }
        }
        file.last_apply = Some(mode);
        self.store(&file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Position;

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
    fn the_sample_is_a_laptop_and_a_monitor() {
        let f = FakeFile::sample();
        assert_eq!(f.screens.iter().filter(|s| s.enabled).count(), 2);
        let laptop = f.screens.iter().find(|s| s.builtin).unwrap();
        assert!(laptop.primary);
        assert_eq!(f.capabilities.origin, Origin::TopLeft);
    }

    #[test]
    fn a_missing_file_starts_as_the_sample() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let fake = Fake::at(path.clone());
        assert_eq!(fake.query().unwrap().screens, FakeFile::sample().screens);
        assert!(path.exists());
    }

    #[test]
    fn apply_writes_positions_and_primary() {
        let dir = tempfile::tempdir().unwrap();
        let fake = Fake::at(dir.path().join("state.json"));
        fake.apply(
            &layout("HDMI-1", &[("eDP-1", 0, 320), ("HDMI-1", 1280, 0)]),
            ApplyMode::Persistent,
        )
        .unwrap();
        let state = fake.query().unwrap();
        let laptop = state.screen("eDP-1").unwrap();
        assert_eq!((laptop.rect.x, laptop.rect.y), (0, 320));
        assert!(!laptop.primary);
        assert!(state.screen("HDMI-1").unwrap().primary);
        assert_eq!(fake.load().unwrap().last_apply, Some(ApplyMode::Persistent));
    }

    #[test]
    fn verify_leaves_the_file_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let fake = Fake::at(path.clone());
        fake.query().unwrap();
        let before = std::fs::read(&path).unwrap();
        fake.apply(
            &layout("eDP-1", &[("eDP-1", 0, 320), ("HDMI-1", 1280, 0)]),
            ApplyMode::Verify,
        )
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn a_layout_for_other_screens_is_changed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let fake = Fake::at(path.clone());
        fake.query().unwrap();
        let before = std::fs::read(&path).unwrap();
        let err = fake
            .apply(
                &layout("eDP-1", &[("eDP-1", 0, 0), ("DP-9", 1280, 0)]),
                ApplyMode::Persistent,
            )
            .unwrap_err();
        assert!(matches!(err, Error::Changed));
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn without_primary_support_the_flags_stay() {
        let mut file = FakeFile::sample();
        file.capabilities.primary = false;
        let fake = Fake::in_memory(file);
        fake.apply(
            &layout("HDMI-1", &[("eDP-1", 0, 320), ("HDMI-1", 1280, 0)]),
            ApplyMode::Persistent,
        )
        .unwrap();
        assert!(fake.query().unwrap().screen("eDP-1").unwrap().primary);
    }
}
