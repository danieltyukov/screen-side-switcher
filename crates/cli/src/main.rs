use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use screen_side_core::backend::{arrange, detect, ApplyMode, Backend, Choice};
use screen_side_core::layout::{baseline, infer, Align, Arrangement, Side};
use screen_side_core::model::State;
use screen_side_core::store::{matches, resolve, Store};
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
    let session = Session::open()?;
    match cli.command.unwrap_or(Command::Status) {
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
    }
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
