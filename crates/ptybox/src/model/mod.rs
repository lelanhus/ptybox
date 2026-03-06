//! Domain types for the ptybox protocol.
//!
//! This module defines all public types used across the library:
//! policies, scenarios, run results, observations, terminal state,
//! and the driver protocol.
//!
//! # Submodules
//!
//! - [`policy`] — Security policy types (`Policy`, `SandboxMode`, `NetworkPolicy`, etc.)
//! - [`scenario`] — Scenario definition types (`Scenario`, `Step`, `Action`, `Assertion`)
//! - [`run`] — Run result types (`RunResult`, `RunStatus`, `StepResult`, `ExitStatus`)
//! - [`terminal`] — Terminal display types (`ScreenSnapshot`, `Cursor`, `Cell`, `Style`)
//! - [`ids`] — Typed UUID identifiers (`RunId`, `SessionId`, `StepId`, `SnapshotId`)
//! - [`driver`] — Driver protocol v2 types (`DriverRequestV2`, `DriverResponseV2`)
//! - [`normalization`] — Normalization filter and rule types for replay

/// Driver protocol v2 request/response types.
pub mod driver;
/// Typed UUID identifiers for runs, sessions, steps, and snapshots.
pub mod ids;
/// Normalization filters and rules for replay comparison.
pub mod normalization;
/// Security policy types with deny-by-default model.
pub mod policy;
/// Run result, step result, and exit status types.
pub mod run;
/// Scenario, step, action, and assertion definition types.
pub mod scenario;
/// Terminal display types: snapshots, cursors, cells, and styles.
pub mod terminal;

pub use driver::*;
pub use ids::{RunId, SessionId, SnapshotId, StepId};
pub use normalization::*;
pub use policy::*;
pub use run::*;
pub use scenario::*;
pub use terminal::*;

/// Maximum length for user-supplied regex patterns to prevent `ReDoS` attacks.
pub const MAX_REGEX_PATTERN_LEN: usize = 1000;
