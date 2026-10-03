//! Everything Screen Side knows about screens, independent of any window.
//!
//! The command line and the app are both thin layers over this crate: they
//! pick a backend, read a [`model::State`], turn a request into a
//! [`layout::Arrangement`], compute a [`model::Layout`] and hand it back.

pub mod backend;
pub mod doctor;
pub mod edid;
pub mod error;
pub mod layout;
pub mod legacy;
pub mod model;
pub mod run;
pub mod shortcut;
pub mod store;
pub mod watch;

pub use error::Error;
