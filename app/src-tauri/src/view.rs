//! The state the window draws, built from core types. Serialised field for
//! field as `AppState` in app/src/backend/types.ts.

use serde::Serialize;

use screen_side_core::backend::{Backend, Capabilities};
use screen_side_core::layout::{infer, Arrangement};
use screen_side_core::model::State;
use screen_side_core::shortcut::{plan_install, Desktop, Plan, DEFAULT_GNOME_KEYS};
use screen_side_core::store::{matches, resolve, SavedLayout, Settings};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenView {
    pub number: usize,
    pub id: String,
    pub connector: String,
    pub name: String,
    pub builtin: bool,
    pub enabled: bool,
    pub primary: bool,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutView {
    pub name: String,
    pub auto: bool,
    pub matches: bool,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub background: bool,
    pub auto_start: bool,
    pub auto_apply: bool,
    pub shortcut: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ShortcutSupport {
    /// The app registers a global shortcut while it runs.
    App,
    /// A GNOME custom keybinding the app can add or remove.
    Gnome { installed: bool, keys: String },
    /// The desktop owns shortcuts; these are the words to add one.
    Manual { instructions: String },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    pub version: String,
    pub platform: String,
    pub backend: Option<String>,
    pub capabilities: Option<Capabilities>,
    pub screens: Vec<ScreenView>,
    pub arrangement: Option<Arrangement>,
    pub layouts: Vec<LayoutView>,
    pub active_layout: Option<String>,
    pub settings: SettingsView,
    pub shortcut: ShortcutSupport,
    pub cli_path: Option<String>,
    pub error: Option<String>,
}

pub struct ViewInput<'a> {
    pub platform: &'a str,
    /// The backend and what it reports, or None when there is no backend.
    pub backend: Option<(&'a dyn Backend, &'a State)>,
    pub layouts: Result<Vec<SavedLayout>, String>,
    pub settings: &'a Settings,
    pub auto_start: bool,
    pub shortcut: ShortcutSupport,
    pub cli_path: Option<String>,
    pub error: Option<String>,
}

/// `linux`, `windows` or `macos`, as the interface knows them.
pub fn platform() -> &'static str {
    match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "macos",
        _ => "linux",
    }
}

pub fn shortcut_support(desktop: Desktop, installed: bool, command: &[String]) -> ShortcutSupport {
    match plan_install(desktop, command, DEFAULT_GNOME_KEYS, "") {
        Plan::Run(_) => ShortcutSupport::Gnome {
            installed,
            keys: DEFAULT_GNOME_KEYS.into(),
        },
        Plan::Instructions(instructions) => ShortcutSupport::Manual { instructions },
        Plan::AppManaged(_) => ShortcutSupport::App,
    }
}

pub fn app_state(input: ViewInput) -> AppState {
    let settings = SettingsView {
        background: input.settings.background,
        auto_start: input.auto_start,
        auto_apply: input.settings.auto_apply,
        shortcut: input.settings.shortcut.clone(),
    };
    let mut error = input.error;
    let layouts = match input.layouts {
        Ok(layouts) => layouts,
        Err(e) => {
            error.get_or_insert(e);
            Vec::new()
        }
    };
    let Some((backend, state)) = input.backend else {
        return AppState {
            version: env!("CARGO_PKG_VERSION").into(),
            platform: input.platform.into(),
            backend: None,
            capabilities: None,
            screens: Vec::new(),
            arrangement: None,
            layouts: layouts
                .iter()
                .map(|l| LayoutView {
                    name: l.name.clone(),
                    auto: l.auto,
                    matches: false,
                    summary: l.summary(),
                })
                .collect(),
            active_layout: None,
            settings,
            shortcut: input.shortcut,
            cli_path: input.cli_path,
            error,
        };
    };

    let arrangement = if state.enabled().count() >= 2 {
        infer(state)
    } else {
        None
    };
    let active_layout = arrangement.as_ref().and_then(|arr| {
        layouts
            .iter()
            .filter(|l| matches(l, state))
            .find(|l| {
                resolve(l, state).is_ok_and(|r| {
                    r.anchor == arr.anchor
                        && r.align == arr.align
                        && r.primary == arr.primary
                        && r.placements == arr.placements
                })
            })
            .map(|l| l.name.clone())
    });
    let mut layout_views: Vec<LayoutView> = layouts
        .iter()
        .map(|l| LayoutView {
            name: l.name.clone(),
            auto: l.auto,
            matches: matches(l, state),
            summary: l.summary(),
        })
        .collect();
    layout_views.sort_by_key(|l| !l.matches);

    AppState {
        version: env!("CARGO_PKG_VERSION").into(),
        platform: input.platform.into(),
        backend: Some(backend.name().into()),
        capabilities: Some(backend.capabilities()),
        screens: state
            .listed()
            .iter()
            .enumerate()
            .map(|(i, s)| ScreenView {
                number: i + 1,
                id: s.id.clone(),
                connector: s.connector.clone(),
                name: s.name.clone(),
                builtin: s.builtin,
                enabled: s.enabled,
                primary: s.primary,
                x: s.rect.x,
                y: s.rect.y,
                width: s.rect.width,
                height: s.rect.height,
                scale: s.scale,
            })
            .collect(),
        arrangement,
        layouts: layout_views,
        active_layout,
        settings,
        shortcut: input.shortcut,
        cli_path: input.cli_path,
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use screen_side_core::backend::fake::{Fake, FakeFile};
    use screen_side_core::backend::Backend;
    use screen_side_core::layout::{baseline, Side};
    use screen_side_core::store::{capture, Settings};

    fn sample() -> (Fake, State) {
        let fake = Fake::in_memory(FakeFile::sample());
        let state = fake.query().unwrap();
        (fake, state)
    }

    #[test]
    fn serialises_field_for_field_like_types_ts() {
        let (fake, state) = sample();
        let office = capture("office", &state, &baseline(&state).unwrap(), true).unwrap();
        let other = SavedLayout {
            name: "home".into(),
            screens: vec!["x:y:z".into()],
            ..office.clone()
        };
        let view = app_state(ViewInput {
            platform: "linux",
            backend: Some((&fake as &dyn Backend, &state)),
            layouts: Ok(vec![other, office]),
            settings: &Settings::default(),
            auto_start: false,
            shortcut: ShortcutSupport::App,
            cli_path: Some("/usr/bin/screen-side".into()),
            error: None,
        });
        let json = serde_json::to_value(&view).unwrap();
        for key in [
            "version",
            "platform",
            "backend",
            "capabilities",
            "screens",
            "arrangement",
            "layouts",
            "activeLayout",
            "settings",
            "shortcut",
            "cliPath",
            "error",
        ] {
            assert!(json.get(key).is_some(), "missing {key}");
        }
        assert_eq!(json["activeLayout"], "office");
        assert_eq!(json["settings"]["autoStart"], false);
        assert_eq!(json["settings"]["autoApply"], true);
        assert_eq!(json["shortcut"]["kind"], "app");
        assert_eq!(json["screens"][0]["id"], "HDMI-1");
        assert_eq!(json["screens"][0]["number"], 1);
        assert_eq!(json["arrangement"]["placements"][0]["side"], "left");
        assert_eq!(json["capabilities"]["origin"], "top_left");
        // Layouts for the screens connected now come first.
        assert_eq!(json["layouts"][0]["name"], "office");
        assert_eq!(json["layouts"][0]["matches"], true);
        assert_eq!(json["layouts"][1]["matches"], false);
        assert!(!json.to_string().contains("FAKE0001"));
    }

    #[test]
    fn no_backend_is_an_error_with_no_screens() {
        let view = app_state(ViewInput {
            platform: "linux",
            backend: None,
            layouts: Ok(vec![]),
            settings: &Settings::default(),
            auto_start: false,
            shortcut: ShortcutSupport::App,
            cli_path: None,
            error: Some("No graphical session".into()),
        });
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["backend"], serde_json::Value::Null);
        assert_eq!(json["screens"], serde_json::json!([]));
        assert_eq!(json["error"], "No graphical session");
    }

    #[test]
    fn a_moved_screen_is_no_longer_the_saved_layout() {
        let (fake, state) = sample();
        let office = capture("office", &state, &baseline(&state).unwrap(), false).unwrap();
        let moved = baseline(&state).unwrap().move_all(Side::Right);
        screen_side_core::backend::arrange(
            &fake,
            &state,
            &moved,
            screen_side_core::backend::ApplyMode::Persistent,
        )
        .unwrap();
        let now = fake.query().unwrap();
        let view = app_state(ViewInput {
            platform: "windows",
            backend: Some((&fake as &dyn Backend, &now)),
            layouts: Ok(vec![office]),
            settings: &Settings::default(),
            auto_start: true,
            shortcut: ShortcutSupport::Gnome {
                installed: false,
                keys: "<Super><Alt>s".into(),
            },
            cli_path: None,
            error: None,
        });
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["activeLayout"], serde_json::Value::Null);
        assert_eq!(json["platform"], "windows");
        assert_eq!(
            json["shortcut"],
            serde_json::json!({"kind": "gnome", "installed": false, "keys": "<Super><Alt>s"})
        );
    }

    #[test]
    fn shortcut_support_per_desktop() {
        let cmd = vec![
            "/opt/Screen Side/screen-side-gui".to_string(),
            "--toggle".to_string(),
        ];
        assert_eq!(
            shortcut_support(Desktop::Windows, false, &cmd),
            ShortcutSupport::App
        );
        assert_eq!(
            shortcut_support(Desktop::OtherX11, false, &cmd),
            ShortcutSupport::App
        );
        assert!(matches!(
            shortcut_support(Desktop::Gnome, true, &cmd),
            ShortcutSupport::Gnome {
                installed: true,
                ..
            }
        ));
        match shortcut_support(Desktop::Sway, false, &cmd) {
            ShortcutSupport::Manual { instructions } => {
                assert!(
                    instructions.contains("bindsym")
                        && instructions.contains("'/opt/Screen Side/screen-side-gui' '--toggle'")
                );
            }
            other => panic!("expected manual, got {other:?}"),
        }
    }
}
