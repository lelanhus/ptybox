//! Typed UUID identifiers for run, session, step, and snapshot entities.
//!
//! Each ID type is a newtype around [`Uuid`] with [`Display`](std::fmt::Display),
//! `Serialize`/`Deserialize` (transparent), and `new()` constructor.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Unique identifier for a run execution.
///
/// Generated once per `run_scenario` or `run_exec` invocation.
/// Serializes as a plain UUID string (e.g., `"550e8400-e29b-41d4-a716-446655440000"`).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunId(Uuid);

impl RunId {
    /// Create a new unique run ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for RunId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a PTY session.
///
/// Generated once per [`Session::spawn`](crate::session::Session::spawn) call.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(Uuid);

impl SessionId {
    /// Create a new unique session ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a scenario step.
///
/// Generated for each step in a scenario or driver action sequence.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StepId(Uuid);

impl StepId {
    /// Create a new unique step ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for StepId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for StepId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a screen snapshot.
///
/// Generated each time a snapshot is captured from the terminal emulator.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SnapshotId(Uuid);

impl SnapshotId {
    /// Create a new unique snapshot ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SnapshotId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
