//! TUI-Use: A security-focused harness for driving terminal UI applications.
//!
//! This crate provides a stable JSON/NDJSON protocol for automated agents to interact
//! with TUI apps via keys, text, resize, and wait commands, and verify behavior via
//! deterministic terminal screen snapshots and transcripts.

#![forbid(unsafe_code)]
// Library documentation is in progress. Public API types have docs;
// internal types will be documented in future releases.
#![allow(missing_docs)]

pub mod artifacts;
pub mod assertions;
pub mod driver;
pub mod model;
pub mod policy;
pub mod replay;
pub mod runner;
pub mod scenario;
pub mod session;
pub mod terminal;

pub use crate::model::*;

pub mod run {
    use super::runner::{
        run_exec_with_options as run_exec_impl, run_scenario as run_scenario_impl, RunnerOptions,
    };
    use super::{Policy, RunResult, Scenario};

    pub fn run_scenario_with_options(
        scenario: Scenario,
        options: RunnerOptions,
    ) -> crate::runner::RunnerResult<RunResult> {
        run_scenario_impl(scenario, options)
    }

    pub fn run_scenario(scenario: Scenario) -> crate::runner::RunnerResult<RunResult> {
        run_scenario_impl(scenario, RunnerOptions::default())
    }

    pub fn run_exec(
        command: String,
        args: Vec<String>,
        cwd: Option<String>,
        policy: Policy,
    ) -> crate::runner::RunnerResult<RunResult> {
        run_exec_impl(command, args, cwd, policy, RunnerOptions::default())
    }

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
