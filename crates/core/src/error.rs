use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// The layout maths refused the request.
    #[error(transparent)]
    Layout(#[from] crate::layout::LayoutError),
    /// No backend fits this session; the message says what is missing.
    #[error("{0}")]
    NoBackend(String),
    /// The backend cannot do what was asked (no primary on wlroots, say).
    #[error("{0}")]
    Unsupported(String),
    /// The screens were different when the change was applied.
    #[error("The screens changed while the arrangement was being applied. Try again.")]
    Changed,
    /// An external tool failed.
    #[error("{program} failed: {message}")]
    Tool { program: String, message: String },
    /// The system API refused or was unreachable.
    #[error("{0}")]
    System(String),
    /// A config file could not be read or written.
    #[error("{0}")]
    Config(String),
    /// The request names something that does not exist.
    #[error("{0}")]
    Usage(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Error {
    /// The command line's exit code for this error.
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Usage(_) | Error::Unsupported(_) => 2,
            _ => 1,
        }
    }
}
