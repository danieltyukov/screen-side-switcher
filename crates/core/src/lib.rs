//! Everything Screen Side knows about screens, independent of any window.
//!
//! The command line and the app are both thin layers over this crate: they
//! pick a backend, read a [`model::State`], turn a request into a
//! [`layout::Arrangement`], compute a [`model::Layout`] and hand it back.

pub mod error;
pub mod layout;
pub mod model;

pub use error::Error;
