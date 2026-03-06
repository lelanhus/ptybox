//! ptybox: A security-focused harness for driving terminal UI applications.
//!
//! This crate provides a stable JSON/NDJSON protocol for automated agents
//! (including LLMs) to interact with TUI apps via keys, text, resize, and
//! wait commands, and verify behavior via deterministic terminal screen
//! snapshots and transcripts.
//!
//! # Architecture
//!
//! The library is organized into focused modules:
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`run`] | High-level entry points for executing scenarios and commands |
//! | [`session`] | PTY lifecycle: spawn, read, write, resize, terminate |
//! | [`terminal`] | ANSI/VT parsing via vt100, canonical [`ScreenSnapshot`] |
//! | [`policy`] | Deny-by-default policy validation, sandbox profile generation |
//! | [`runner`] | Step execution engine, wait conditions, budget enforcement |
//! | [`driver`] | Interactive NDJSON protocol v2 for agent loops |
//! | [`artifacts`] | Transcript, snapshots, checksums, run summary to disk |
//! | [`replay`] | Replay comparison with normalization filters |
//! | [`scenario`] | Scenario/policy file parsing (JSON/YAML) |
//! | [`assertions`] | Assertion engine for screen/transcript verification |
//! | [`model`] | All domain types: `Policy`, `Scenario`, `RunResult`, `Observation` |
//!
//! # Getting Started
//!
//! Use the [`run`] module for the simplest path to execution:
//!
//! ```no_run
//! use ptybox::run::{run_scenario, run_exec};
//! use ptybox::{Scenario, Policy};
//!
//! // Execute a scenario file
//! # fn example() -> Result<(), ptybox::runner::RunnerError> {
//! let scenario: Scenario = ptybox::scenario::load_scenario_file("test.json")?;
//! let result = run_scenario(scenario)?;
//!
//! // Or run a single command under policy
//! let policy = Policy::default();
//! let result = run_exec(
//!     "/bin/echo".to_string(),
//!     vec!["hello".to_string()],
//!     None,
//!     policy,
//! )?;
//! # Ok(())
//! # }
//! ```
//!
//! For interactive agent loops, use [`driver::run_driver`] with the
//! NDJSON protocol v2.
//!
//! # Security Model
//!
//! ptybox enforces deny-by-default security:
//! - Sandbox (Seatbelt) enabled by default on macOS
//! - Network disabled by default
//! - All filesystem paths must be absolute and explicitly allowlisted
//! - Dangerous environment variables are blocked
//! - Resource budgets prevent denial-of-service
//! - Regex patterns are bounded to prevent `ReDoS`

#![forbid(unsafe_code)]
#![warn(missing_docs)]

// Legacy error constructors are deprecated but still widely used across modules.
// Allow deprecated usage during migration to the new ErrorCode-based constructors.
#[allow(deprecated)]
pub mod artifacts;
pub mod assertions;
#[allow(deprecated)]
pub mod driver;
pub mod model;
#[allow(deprecated)]
pub mod policy;
#[allow(deprecated)]
pub mod replay;
#[allow(deprecated)]
pub mod runner;
#[allow(deprecated)]
pub mod scenario;
#[allow(deprecated)]
pub mod session;
pub mod terminal;
#[allow(deprecated)]
pub mod util;

pub use crate::model::*;

/// High-level entry points for executing scenarios and commands.
///
/// This module provides convenience functions that combine policy validation,
/// session spawning, step execution, and artifact collection into a single call.
///
/// # Key Functions
///
/// - [`run::run_scenario`] — Execute a scenario with default options
/// - [`run::run_scenario_with_options`] — Execute with custom runner options
/// - [`run::run_exec`] — Run a single command under policy with defaults
/// - [`run::run_exec_with_options`] — Run a single command with custom options
///
/// # Example
///
/// ```no_run
/// use ptybox::run::run_exec;
/// use ptybox::Policy;
///
/// # fn example() -> Result<(), ptybox::runner::RunnerError> {
/// let result = run_exec(
///     "/bin/echo".to_string(),
///     vec!["hello".to_string()],
///     None,
///     Policy::default(),
/// )?;
/// assert_eq!(result.status, ptybox::RunStatus::Passed);
/// # Ok(())
/// # }
/// ```
pub mod run {
    use super::runner::{
        run_exec_with_options as run_exec_impl, run_scenario as run_scenario_impl, RunnerOptions,
    };
    use super::{Policy, RunResult, Scenario};

    /// Execute a scenario with custom runner options.
    ///
    /// This is the most flexible scenario entry point, allowing you to
    /// configure artifact output and progress callbacks.
    ///
    /// # Errors
    ///
    /// Returns [`RunnerError`](crate::runner::RunnerError) if:
    /// - Policy validation fails (`E_POLICY_DENIED`)
    /// - Sandbox is unavailable (`E_SANDBOX_UNAVAILABLE`)
    /// - A step times out (`E_TIMEOUT`)
    /// - An assertion fails (`E_ASSERTION_FAILED`)
    /// - The child process exits unexpectedly (`E_PROCESS_EXIT`)
    pub fn run_scenario_with_options(
        scenario: Scenario,
        options: RunnerOptions,
    ) -> crate::runner::RunnerResult<RunResult> {
        run_scenario_impl(scenario, options)
    }

    /// Execute a scenario with default options.
    ///
    /// Equivalent to calling [`run_scenario_with_options`] with
    /// [`RunnerOptions::default()`].
    ///
    /// # Errors
    ///
    /// See [`run_scenario_with_options`] for the full error list.
    pub fn run_scenario(scenario: Scenario) -> crate::runner::RunnerResult<RunResult> {
        run_scenario_impl(scenario, RunnerOptions::default())
    }

    /// Run a single command under a policy with default options.
    ///
    /// This wraps the command in a minimal scenario with no steps or
    /// assertions — useful for simple exec-and-capture workflows.
    ///
    /// # Errors
    ///
    /// See [`run_scenario_with_options`] for the full error list.
    pub fn run_exec(
        command: String,
        args: Vec<String>,
        cwd: Option<String>,
        policy: Policy,
    ) -> crate::runner::RunnerResult<RunResult> {
        run_exec_impl(command, args, cwd, policy, RunnerOptions::default())
    }

    /// Run a single command under a policy with custom options.
    ///
    /// # Errors
    ///
    /// See [`run_scenario_with_options`] for the full error list.
    pub fn run_exec_with_options(
        command: String,
        args: Vec<String>,
        cwd: Option<String>,
        policy: Policy,
        options: RunnerOptions,
    ) -> crate::runner::RunnerResult<RunResult> {
        run_exec_impl(command, args, cwd, policy, options)
    }
}
