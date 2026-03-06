//! Typed UUID identifiers for run, session, step, and snapshot entities.
//!
//! Each ID type is a newtype around [`Uuid`](uuid::Uuid) with [`Display`](std::fmt::Display),
//! `Serialize`/`Deserialize` (transparent), and `new()` constructor.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Create a new unique ID.
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

define_id! {
    /// Unique identifier for a run execution.
    ///
    /// Generated once per `run_scenario` or `run_exec` invocation.
    /// Serializes as a plain UUID string (e.g., `"550e8400-e29b-41d4-a716-446655440000"`).
    RunId
}

define_id! {
    /// Unique identifier for a PTY session.
    ///
    /// Generated once per [`Session::spawn`](crate::session::Session::spawn) call.
    SessionId
}

define_id! {
    /// Unique identifier for a scenario step.
    ///
    /// Generated for each step in a scenario or driver action sequence.
    StepId
}

define_id! {
    /// Unique identifier for a screen snapshot.
    ///
    /// Generated each time a snapshot is captured from the terminal emulator.
    SnapshotId
}
