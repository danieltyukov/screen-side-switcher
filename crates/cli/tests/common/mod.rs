use std::path::PathBuf;

use assert_cmd::Command;
use screen_side_core::backend::fake::FakeFile;
use screen_side_core::model::{Identity, Rect, Screen};
use tempfile::TempDir;

pub struct Env {
    pub dir: TempDir,
}

impl Env {
    pub fn new(file: FakeFile) -> Env {
        let env = Env {
            dir: tempfile::tempdir().unwrap(),
        };
        env.write(&file);
        env
    }

    pub fn state_path(&self) -> PathBuf {
        self.dir.path().join("state.json")
    }

    pub fn config(&self) -> PathBuf {
        self.dir.path().join("config")
    }

    pub fn write(&self, file: &FakeFile) {
        std::fs::write(
            self.state_path(),
            serde_json::to_string_pretty(file).unwrap(),
        )
        .unwrap();
    }

    pub fn read(&self) -> FakeFile {
        serde_json::from_str(&std::fs::read_to_string(self.state_path()).unwrap()).unwrap()
    }

    pub fn raw(&self) -> Vec<u8> {
        std::fs::read(self.state_path()).unwrap()
    }

    pub fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("screen-side").unwrap();
        cmd.env("SCREEN_SIDE_BACKEND", "fake")
            .env("SCREEN_SIDE_FAKE_STATE", self.state_path())
            .env("SCREEN_SIDE_CONFIG_DIR", self.config());
        cmd
    }
}

pub fn at(file: &FakeFile, id: &str) -> (i32, i32) {
    let s = file.screens.iter().find(|s| s.id == id).unwrap();
    (s.rect.x, s.rect.y)
}

pub fn monitor(id: &str, name: &str, rect: Rect) -> Screen {
    Screen {
        id: id.into(),
        connector: id.into(),
        name: name.into(),
        identity: Identity {
            vendor: "DEL".into(),
            product: format!("0x{}", id.len()),
            serial: format!("S-{id}"),
        },
        builtin: false,
        enabled: true,
        primary: false,
        rect,
        scale: 1.0,
    }
}
