//! Test utilities and fixtures for tui-use integration tests.
//!
//! This crate provides fluent builder APIs and helper functions to reduce
//! boilerplate when writing tests for tui-use. It includes:
//!
//! - [`PolicyBuilder`] - Fluent API for constructing test policies
//! - [`ScenarioBuilder`] - Fluent API for constructing test scenarios
//! - [`StepBuilder`] - Fluent API for constructing scenario steps
//! - [`temp_dir`] - Create unique temporary directories
//! - [`write_policy`] / [`write_scenario`] - Serialize to JSON files
//!
//! # Example
//!
//! ```ignore
//! use tui_use_fixtures::{PolicyBuilder, ScenarioBuilder, StepBuilder, temp_dir, write_scenario};
//!
//! // Create a temp directory for test artifacts
//! let dir = temp_dir("my-test");
//!
//! // Build a policy with test defaults
//! let policy = PolicyBuilder::test_default(&dir)
//!     .with_allowed_exec(vec!["/bin/echo".into()])
//!     .with_write_access(vec![dir.join("artifacts").display().to_string()])
//!     .build();
//!
//! // Build a scenario
//! let scenario = ScenarioBuilder::new("echo-test", "/bin/echo")
//!     .with_args(vec!["hello".into()])
//!     .with_policy(policy)
//!     .add_wait_for_exit("wait-for-exit")
//!     .build();
//!
//! // Write to file for CLI testing
//! write_scenario(&dir.join("scenario.json"), &scenario);
//! ```

// Test fixtures crate - relaxed lints for test utilities
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::missing_panics_doc)]

pub mod builders;
pub mod helpers;

// Re-export commonly used items at crate root
pub use builders::{PolicyBuilder, ScenarioBuilder, StepBuilder};
pub use helpers::{temp_dir, write_policy, write_scenario};
