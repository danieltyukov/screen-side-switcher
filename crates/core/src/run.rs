//! Running the desktop's own tools (kscreen-doctor, wlr-randr, xrandr,
//! gsettings). Backends take a `Runner` so tests can script the output and
//! check the exact command lines.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::Error;

pub trait Runner: Send + Sync {
    /// Runs `program` with `args` and returns its standard output. A
    /// non-zero exit is an error carrying standard error.
    fn run(&self, program: &str, args: &[String]) -> Result<String, Error>;
}

pub struct SystemRunner;

impl Runner for SystemRunner {
    fn run(&self, program: &str, args: &[String]) -> Result<String, Error> {
        let out = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| Error::Tool {
                program: program.to_string(),
                message: e.to_string(),
            })?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(Error::Tool {
                program: program.to_string(),
                message: if stderr.is_empty() {
                    format!("exited with {}", out.status)
                } else {
                    stderr
                },
            });
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

/// True when `program` is an executable file in a directory on PATH.
pub fn on_path(program: &str) -> bool {
    which(program).is_some()
}

/// Where `program` is on PATH. The app shows this for the CLI.
pub fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT".into())
            .split(';')
            .map(|e| e.to_string())
            .chain(std::iter::once(String::new()))
            .collect()
    } else {
        vec![String::new()]
    };
    std::env::split_paths(&path).find_map(|dir| {
        exts.iter()
            .map(|ext| dir.join(format!("{program}{ext}")))
            .find(|p| is_executable(p))
    })
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
pub(crate) mod testing {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use super::Runner;
    use crate::Error;

    type Reply = (String, Result<String, String>);

    /// Replies with canned output in order and remembers every call.
    pub struct Scripted {
        replies: Mutex<VecDeque<Reply>>,
        calls: Mutex<Vec<Vec<String>>>,
    }

    impl Scripted {
        pub fn new(replies: Vec<(&str, Result<&str, &str>)>) -> Self {
            Scripted {
                replies: Mutex::new(
                    replies
                        .into_iter()
                        .map(|(p, r)| {
                            (p.to_string(), r.map(str::to_string).map_err(str::to_string))
                        })
                        .collect(),
                ),
                calls: Mutex::new(Vec::new()),
            }
        }

        pub fn calls(&self) -> Vec<Vec<String>> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl Runner for Scripted {
        fn run(&self, program: &str, args: &[String]) -> Result<String, Error> {
            let mut call = vec![program.to_string()];
            call.extend(args.iter().cloned());
            self.calls.lock().unwrap().push(call);
            let (expected, reply) = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| panic!("unexpected call to {program}"));
            assert_eq!(expected, program, "called the wrong program");
            reply.map_err(|message| Error::Tool {
                program: program.to_string(),
                message,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::Scripted;
    use super::*;

    #[test]
    fn scripted_replies_in_order_and_records_calls() {
        let r = Scripted::new(vec![("xrandr", Ok("one")), ("xrandr", Err("boom"))]);
        assert_eq!(r.run("xrandr", &["--current".into()]).unwrap(), "one");
        let err = r.run("xrandr", &[]).unwrap_err();
        assert!(matches!(err, Error::Tool { ref message, .. } if message == "boom"));
        assert_eq!(
            r.calls(),
            vec![
                vec!["xrandr".to_string(), "--current".to_string()],
                vec!["xrandr".to_string()]
            ]
        );
    }

    #[test]
    fn a_missing_program_is_a_tool_error() {
        let err = SystemRunner
            .run("definitely-not-a-program-xyz", &[])
            .unwrap_err();
        assert!(
            matches!(err, Error::Tool { ref program, .. } if program == "definitely-not-a-program-xyz")
        );
    }

    #[cfg(unix)]
    #[test]
    fn finds_programs_on_path() {
        assert!(on_path("sh"));
        assert!(which("sh").is_some_and(|p| p.is_absolute()));
        assert!(!on_path("definitely-not-a-program-xyz"));
    }

    #[cfg(unix)]
    #[test]
    fn a_failing_program_reports_stderr() {
        let err = SystemRunner
            .run("sh", &["-c".into(), "echo nope >&2; exit 3".into()])
            .unwrap_err();
        assert!(matches!(err, Error::Tool { ref message, .. } if message == "nope"));
    }
}
