mod common;

use common::{at, monitor, Env};
use predicates::prelude::*;
use screen_side_core::backend::fake::FakeFile;
use screen_side_core::backend::ApplyMode;
use screen_side_core::model::Rect;

fn sample() -> Env {
    Env::new(FakeFile::sample())
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

#[test]
fn status_describes_the_sample() {
    let env = sample();
    env.cmd().arg("status").assert().success().stdout(
        predicate::str::contains("External screen is left of the built-in screen.")
            .and(predicate::str::contains("Alignment: centred."))
            .and(predicate::str::contains("HDMI-1"))
            .and(predicate::str::contains("DELL U2723QE"))
            .and(predicate::str::contains("2560x1440"))
            .and(predicate::str::contains("primary"))
            .and(predicate::str::contains("Backend: fake")),
    );
}

#[test]
fn no_command_means_status() {
    let env = sample();
    env.cmd()
        .assert()
        .success()
        .stdout(predicate::str::contains("External screen is left"));
}

#[test]
fn status_json_has_schema_and_numbers() {
    let env = sample();
    let out = env.cmd().args(["status", "--json"]).output().unwrap();
    assert!(out.status.success());
    let text = stdout(&out);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["schema"], 1);
    assert_eq!(v["screens"][0]["number"], 1);
    assert_eq!(v["screens"][0]["id"], "HDMI-1");
    assert_eq!(v["arrangement"]["placements"][0]["side"], "left");
    assert_eq!(v["capabilities"]["origin"], "top_left");
    assert!(!text.contains("serial") && !text.contains("FAKE0001"));
}

#[test]
fn right_moves_the_external_screen() {
    let env = sample();
    env.cmd()
        .arg("right")
        .assert()
        .success()
        .stdout(predicate::str::contains("right of the built-in screen"));
    let file = env.read();
    assert_eq!(at(&file, "HDMI-1"), (1280, 0));
    assert_eq!(at(&file, "eDP-1"), (0, 320));
    assert_eq!(file.last_apply, Some(ApplyMode::Persistent));
}

#[test]
fn temporary_and_dry_run() {
    let env = sample();
    env.cmd().args(["above", "--temporary"]).assert().success();
    assert_eq!(env.read().last_apply, Some(ApplyMode::Temporary));
    let before = env.raw();
    env.cmd()
        .args(["right", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run").and(predicate::str::contains("checked")));
    assert_eq!(env.raw(), before);
}

#[test]
fn toggle_flips_and_flips_back() {
    let env = sample();
    let start = env.read();
    env.cmd().arg("toggle").assert().success();
    assert_eq!(at(&env.read(), "HDMI-1"), (1280, 0));
    env.cmd().arg("toggle").assert().success();
    let end = env.read();
    assert_eq!(at(&end, "HDMI-1"), at(&start, "HDMI-1"));
    assert_eq!(at(&end, "eDP-1"), at(&start, "eDP-1"));
}

#[test]
fn align_end_and_primary() {
    let env = sample();
    env.cmd().args(["align", "end"]).assert().success();
    let file = env.read();
    let bottom = |id: &str| {
        let s = file.screens.iter().find(|s| s.id == id).unwrap();
        s.rect.y + s.rect.height
    };
    assert_eq!(bottom("HDMI-1"), bottom("eDP-1"));
    env.cmd().args(["primary", "1"]).assert().success();
    let file = env.read();
    assert!(
        file.screens
            .iter()
            .find(|s| s.id == "HDMI-1")
            .unwrap()
            .primary
    );
    assert!(
        !file
            .screens
            .iter()
            .find(|s| s.id == "eDP-1")
            .unwrap()
            .primary
    );
}

fn three_screens() -> Env {
    let mut file = FakeFile::sample();
    file.screens.push(monitor(
        "DP-1",
        "DELL P2422H",
        Rect::new(3840, 320, 1920, 1080),
    ));
    Env::new(file)
}

#[test]
fn screen_selector() {
    let env = three_screens();
    env.cmd()
        .args(["left", "--screen", "dell"])
        .assert()
        .code(2)
        .stderr(
            predicate::str::contains("DELL U2723QE").and(predicate::str::contains("DELL P2422H")),
        );
    env.cmd()
        .args(["left", "--screen", "DP-1"])
        .assert()
        .success();
    let file = env.read();
    let x = |id: &str| at(&file, id).0;
    assert!(x("DP-1") < x("HDMI-1") && x("HDMI-1") < x("eDP-1"));
    env.cmd().args(["left", "--screen", "9"]).assert().code(2);
}

#[test]
fn switched_off_screen_is_listed_and_refused() {
    let mut file = FakeFile::sample();
    file.screens[0].enabled = false;
    file.screens[0].primary = false;
    file.screens[1].primary = true;
    file.screens
        .push(monitor("DP-1", "LG HDR 4K", Rect::new(2560, 0, 1920, 1080)));
    let env = Env::new(file);
    env.cmd()
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"eDP-1 +Built-in display +off").unwrap());
    env.cmd()
        .args(["left", "--screen", "eDP-1"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("switched off"));
}

#[test]
fn one_screen_is_a_clear_error() {
    let mut file = FakeFile::sample();
    file.screens[0].enabled = false;
    let env = Env::new(file);
    env.cmd()
        .arg("left")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("Only one screen is switched on"));
}

#[test]
fn overlap_is_refused_without_changes() {
    let mut file = FakeFile::sample();
    file.screens[1].name = "Left monitor".into();
    file.screens.push(monitor(
        "DP-1",
        "Right monitor",
        Rect::new(3840, 0, 2560, 1440),
    ));
    let env = Env::new(file);
    let before = env.raw();
    env.cmd()
        .args(["above", "--screen", "DP-1", "--align", "center"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("would overlap"));
    assert_eq!(env.raw(), before);
}

#[test]
fn primary_unsupported() {
    let mut file = FakeFile::sample();
    file.capabilities.primary = false;
    let env = Env::new(file);
    env.cmd()
        .args(["primary", "1"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no primary screen"));
}

#[test]
fn temporary_refused_where_the_system_saves() {
    let mut file = FakeFile::sample();
    file.capabilities.temporary = false;
    file.capabilities.remembers = true;
    let env = Env::new(file);
    env.cmd().args(["left", "--temporary"]).assert().code(2);
}

#[test]
fn version_and_help() {
    let env = sample();
    env.cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("2.0.0"));
    env.cmd().arg("--help").assert().success().stdout(
        predicate::str::contains("toggle")
            .and(predicate::str::contains("primary"))
            .and(predicate::str::contains("align")),
    );
}
