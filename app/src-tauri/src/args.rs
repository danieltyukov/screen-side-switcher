//! What the app was launched to do.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Open the window.
    Show,
    /// Start hidden, with the tray and the watcher (autostart uses this).
    Background,
    /// Swap the sides and exit, for keyboard shortcuts.
    Toggle,
    /// Apply a saved layout and exit.
    Apply(String),
}

/// Reads the arguments after the program name. Unknown ones are ignored,
/// because systems add their own (macOS can pass `-psn_...`).
pub fn parse(args: &[String]) -> Action {
    let mut action = Action::Show;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--background" => action = Action::Background,
            "--toggle" => return Action::Toggle,
            "--apply" => {
                if let Some(name) = iter.next() {
                    return Action::Apply(name.clone());
                }
            }
            _ => {}
        }
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_all(args: &[&str]) -> Action {
        parse(&args.iter().map(|a| a.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn launch_arguments() {
        assert_eq!(parse_all(&[]), Action::Show);
        assert_eq!(parse_all(&["--background"]), Action::Background);
        assert_eq!(parse_all(&["--toggle"]), Action::Toggle);
        assert_eq!(
            parse_all(&["--apply", "office"]),
            Action::Apply("office".into())
        );
        assert_eq!(parse_all(&["--apply"]), Action::Show, "nothing to apply");
        // macOS adds its own process serial number on some launches.
        assert_eq!(parse_all(&["-psn_0_12345"]), Action::Show);
        assert_eq!(parse_all(&["-psn_0_1", "--background"]), Action::Background);
    }
}
