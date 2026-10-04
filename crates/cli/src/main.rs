use std::io::Write;
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use clap::{Args, Parser, Subcommand};
use screen_side_core::backend::{arrange, detect, ApplyMode, Backend, Choice, SystemProbe};
use screen_side_core::layout::{baseline, infer, Align, Arrangement, Side};
use screen_side_core::model::State;
use screen_side_core::run::SystemRunner;
use screen_side_core::shortcut;
use screen_side_core::store::{capture, matches, resolve, Store};
use screen_side_core::watch;
use screen_side_core::Error;

mod output;
mod select;

#[derive(Parser)]
#[command(
    name = "screen-side",
    version,
    about = "Put your external screen left, right, above or below the built-in one."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Print machine-readable JSON.
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Args, Clone, Copy)]
struct ApplyArgs {
    /// Do not save the change; it reverts on the next reconnect.
    #[arg(long)]
    temporary: bool,
    /// Work out the change and check it without applying it.
    #[arg(long)]
    dry_run: bool,
}

impl ApplyArgs {
    fn mode(self) -> ApplyMode {
        if self.dry_run {
            ApplyMode::Verify
        } else if self.temporary {
            ApplyMode::Temporary
        } else {
            ApplyMode::Persistent
        }
    }
}

#[derive(Args)]
struct MoveArgs {
    /// Which screen to move: its number from status, its connector, or part of its name.
    #[arg(long)]
    screen: Option<String>,
    /// How the screens line up on the other axis: start, center or end.
    #[arg(long)]
    align: Option<Align>,
    #[command(flatten)]
    apply: ApplyArgs,
}

#[derive(Subcommand)]
enum Command {
    /// Show the screens and how they are arranged (the default).
    Status,
    /// Put the external screen left of the built-in one.
    Left(MoveArgs),
    /// Put the external screen right of the built-in one.
    Right(MoveArgs),
    /// Put the external screen above the built-in one.
    Above(MoveArgs),
    /// Put the external screen below the built-in one.
    Below(MoveArgs),
    /// Swap left with right and above with below. Meant for a keyboard shortcut.
    Toggle(ApplyArgs),
    /// Make a screen the primary one.
    Primary {
        /// Its number from status, its connector, or part of its name.
        screen: String,
        #[command(flatten)]
        apply: ApplyArgs,
    },
    /// Line the screens up at the start, centre or end of the shared edge.
    Align {
        /// start, center or end (top, centre, bottom and left, right also work).
        align: Align,
        #[command(flatten)]
        apply: ApplyArgs,
    },
    /// Save the current arrangement for the screens connected now.
    Save {
        name: String,
        /// Apply it automatically whenever these screens are connected.
        #[arg(long)]
        auto: bool,
    },
    /// Apply a saved layout.
    Apply {
        name: String,
        #[command(flatten)]
        apply: ApplyArgs,
    },
    /// List saved layouts.
    Layouts,
    /// Delete a saved layout.
    Forget { name: String },
    /// Set up a keyboard shortcut that runs `screen-side toggle`.
    Shortcut {
        #[command(subcommand)]
        action: ShortcutAction,
    },
    /// Print what Screen Side sees, for bug reports. Serial numbers are left out.
    Doctor,
    /// Keep running and put saved layouts back when their screens are
    /// connected (layouts saved with --auto).
    Watch {
        /// Seconds between checks.
        #[arg(long, default_value_t = 2.0)]
        interval: f64,
    },
}

#[derive(Subcommand)]
enum ShortcutAction {
    /// Add the shortcut (on GNOME), or print what to add elsewhere.
    Install {
        /// GNOME key combination.
        #[arg(long, default_value = shortcut::DEFAULT_GNOME_KEYS)]
        keys: String,
    },
    /// Remove the shortcut.
    Remove,
    /// Show the shortcut, or how to set one up here.
    Show,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("screen-side: {e}");
            ExitCode::from(e.exit_code() as u8)
        }
    }
}

struct Session {
    backend: Box<dyn Backend>,
    choice: Choice,
    store: Store,
    state: State,
}

impl Session {
    fn open() -> Result<Session, Error> {
        let (backend, choice) = detect()?;
        let state = backend.query()?;
        Ok(Session {
            backend,
            choice,
            store: Store::open()?,
            state,
        })
    }

    /// The saved layout these screens are in, if any. A broken layouts
    /// file never stops a status or a move.
    fn layout_in_force(&self) -> Option<String> {
        let layouts = self.store.layouts().ok()?;
        let arr = infer(&self.state)?;
        layouts
            .into_iter()
            .filter(|l| matches(l, &self.state))
            .find(|l| {
                resolve(l, &self.state)
                    .ok()
                    .is_some_and(|r| same_intent(&r, &arr))
            })
            .map(|l| l.name)
    }
}

/// Two arrangements that put the screens in the same places.
fn same_intent(a: &Arrangement, b: &Arrangement) -> bool {
    a.anchor == b.anchor
        && a.align == b.align
        && a.primary == b.primary
        && a.placements == b.placements
}

fn run(cli: Cli) -> Result<(), Error> {
    let command = cli.command.unwrap_or(Command::Status);
    // These work without screens, over SSH or on a headless machine.
    match &command {
        Command::Layouts => return list_layouts(&Store::open()?, cli.json),
        Command::Forget { name } => {
            Store::open()?.forget(name)?;
            println!("Forgot '{}'.", name.trim());
            return Ok(());
        }
        Command::Shortcut { action } => return run_shortcut(action),
        Command::Doctor => {
            let store = Store::open().ok();
            let report = screen_side_core::doctor::report(
                &SystemProbe,
                env!("CARGO_PKG_VERSION"),
                store.as_ref(),
            );
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).expect("serialisable")
                );
            } else {
                print!("{report}");
            }
            return Ok(());
        }
        Command::Watch { interval } if *interval < 0.5 || !interval.is_finite() => {
            return Err(Error::Usage(
                "--interval must be at least 0.5 seconds.".into(),
            ))
        }
        _ => {}
    }
    let session = Session::open()?;
    match command {
        Command::Status => print_status(&session, cli.json),
        Command::Left(m) => move_to(&session, Side::Left, m, cli.json),
        Command::Right(m) => move_to(&session, Side::Right, m, cli.json),
        Command::Above(m) => move_to(&session, Side::Above, m, cli.json),
        Command::Below(m) => move_to(&session, Side::Below, m, cli.json),
        Command::Toggle(a) => {
            let arr = baseline(&session.state)?.toggled();
            finish(&session, &arr, a.mode(), cli.json)
        }
        Command::Primary { screen, apply } => {
            if !session.backend.capabilities().primary {
                return Err(Error::Unsupported(format!(
                    "The {} backend has no primary screen to set.",
                    session.backend.name()
                )));
            }
            let id = select::resolve_screen(&session.state, &screen)?;
            let arr = baseline(&session.state)?.with_primary(&id);
            finish(&session, &arr, apply.mode(), cli.json)
        }
        Command::Align { align, apply } => {
            let arr = baseline(&session.state)?.with_align(align);
            finish(&session, &arr, apply.mode(), cli.json)
        }
        Command::Save { name, auto } => {
            let arr = screen_side_core::layout::to_save(&session.state)?;
            let saved = capture(&name, &session.state, &arr, auto)?;
            let shown = saved.name.clone();
            session.store.put(saved)?;
            println!(
                "Saved '{shown}' for these screens.{}",
                if auto {
                    " It will be applied whenever they are connected."
                } else {
                    ""
                }
            );
            Ok(())
        }
        Command::Apply { name, apply } => {
            let saved = session.store.find(&name)?;
            let arr = resolve(&saved, &session.state)?;
            finish(&session, &arr, apply.mode(), cli.json)
        }
        Command::Watch { interval } => {
            println!("Watching for screen changes. Press Ctrl+C to stop.");
            let stop = AtomicBool::new(false);
            watch::run(
                session.backend.as_ref(),
                &session.store,
                Duration::from_secs_f64(interval),
                || true,
                &stop,
                |event| {
                    if let Some(line) = output::event_text(&event) {
                        println!("{line}");
                        let _ = std::io::stdout().flush();
                    }
                },
            );
            Ok(())
        }
        Command::Layouts | Command::Forget { .. } | Command::Shortcut { .. } | Command::Doctor => {
            unreachable!("handled above")
        }
    }
}

fn run_shortcut(action: &ShortcutAction) -> Result<(), Error> {
    let desktop = shortcut::desktop(&SystemProbe);
    let exe = std::env::current_exe()?;
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    let command = vec![exe.to_string_lossy().into_owned(), "toggle".to_string()];
    let text = match action {
        ShortcutAction::Install { keys } => {
            shortcut::install(&SystemRunner, desktop, &command, keys)?
        }
        ShortcutAction::Remove => shortcut::remove(&SystemRunner, desktop)?,
        ShortcutAction::Show => {
            match shortcut::plan_install(desktop, &command, shortcut::DEFAULT_GNOME_KEYS, "") {
                shortcut::Plan::Run(_) => shortcut::show(&SystemRunner, desktop)?,
                shortcut::Plan::Instructions(t) | shortcut::Plan::AppManaged(t) => t,
            }
        }
    };
    println!("{}", text.trim_end());
    Ok(())
}

fn list_layouts(store: &Store, json: bool) -> Result<(), Error> {
    let layouts = store.layouts()?;
    // Which ones fit the screens connected now, when there are screens.
    let state = detect().ok().and_then(|(b, _)| b.query().ok());
    let fits = |l: &screen_side_core::store::SavedLayout| state.as_ref().map(|s| matches(l, s));
    if json {
        let list: Vec<serde_json::Value> = layouts
            .iter()
            .map(|l| {
                serde_json::json!({
                    "name": l.name,
                    "auto": l.auto,
                    "matches": fits(l),
                    "screens": l.screens.len(),
                    "summary": l.summary(),
                    "saved": l.saved,
                })
            })
            .collect();
        let value = serde_json::json!({ "schema": 1, "layouts": list });
        println!(
            "{}",
            serde_json::to_string_pretty(&value).expect("serialisable")
        );
        return Ok(());
    }
    if layouts.is_empty() {
        println!("No layouts are saved yet. Save one with: screen-side save NAME");
        return Ok(());
    }
    for l in &layouts {
        let fit = match fits(l) {
            Some(true) => "these screens",
            Some(false) => "other screens",
            None => "",
        };
        let line = format!(
            "  {:<20} {:<32} {:<5} {fit}",
            l.name,
            l.summary(),
            if l.auto { "auto" } else { "" }
        );
        println!("{}", line.trim_end());
    }
    Ok(())
}

fn move_to(session: &Session, side: Side, args: MoveArgs, json: bool) -> Result<(), Error> {
    let mut arr = baseline(&session.state)?;
    if let Some(align) = args.align {
        arr = arr.with_align(align);
    }
    arr = match &args.screen {
        Some(sel) => arr.move_screen(&select::resolve_screen(&session.state, sel)?, side)?,
        None => arr.move_all(side),
    };
    finish(session, &arr, args.apply.mode(), json)
}

fn finish(session: &Session, arr: &Arrangement, mode: ApplyMode, json: bool) -> Result<(), Error> {
    let (layout, applied) = arrange(session.backend.as_ref(), &session.state, arr, mode)?;
    if json {
        let value = if mode == ApplyMode::Verify {
            output::dry_run_json(&layout, applied)
        } else {
            let state = session.backend.query()?;
            output::status_json(
                &state,
                infer(&state).as_ref(),
                session.backend.as_ref(),
                None,
            )
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&value).expect("serialisable")
        );
    } else {
        print!(
            "{}",
            output::moved_text(
                &session.state,
                &layout,
                applied,
                session.backend.capabilities()
            )
        );
    }
    Ok(())
}

fn print_status(session: &Session, json: bool) -> Result<(), Error> {
    let arr = infer(&session.state);
    let layout = session.layout_in_force();
    if json {
        let value = output::status_json(
            &session.state,
            arr.as_ref(),
            session.backend.as_ref(),
            layout.as_deref(),
        );
        println!(
            "{}",
            serde_json::to_string_pretty(&value).expect("serialisable")
        );
    } else {
        print!(
            "{}",
            output::status_text(
                &session.state,
                arr.as_ref(),
                session.backend.name(),
                &session.choice.reason,
                layout.as_deref()
            )
        );
    }
    Ok(())
}
