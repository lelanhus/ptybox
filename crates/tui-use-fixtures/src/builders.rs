//! Fluent builder APIs for constructing test fixtures.
//!
//! These builders reduce boilerplate when creating [`Policy`], [`Scenario`],
//! and [`Step`] objects in integration tests.
//!
//! # Example
//!
//! ```ignore
//! use tui_use_fixtures::{PolicyBuilder, ScenarioBuilder, StepBuilder, temp_dir};
//!
//! let dir = temp_dir("example");
//! let policy = PolicyBuilder::test_default(&dir)
//!     .with_allowed_exec(vec!["/bin/echo".into()])
//!     .with_write_access(vec![dir.join("artifacts").display().to_string()])
//!     .build();
//!
//! let scenario = ScenarioBuilder::new("my-test", "/bin/echo")
//!     .with_args(vec!["hello".into()])
//!     .with_policy(policy)
//!     .add_step(StepBuilder::wait_for_text("wait", "hello").build())
//!     .build();
//! ```

use std::path::Path;

use tui_use::model::policy::{
    ArtifactsPolicy, Budgets, EnvPolicy, ExecPolicy, FsPolicy, NetworkPolicy, Policy,
    ReplayPolicy, SandboxMode, POLICY_VERSION,
};
use tui_use::model::scenario::PolicyRef;
use tui_use::model::{
    Action, ActionType, Assertion, RunConfig, Scenario, ScenarioMetadata, Step, StepId,
    TerminalSize,
};

// ============================================================================
// PolicyBuilder
// ============================================================================

/// Fluent builder for constructing [`Policy`] objects in tests.
///
/// Starts with sensible test defaults (no sandbox, network disabled, all unsafe
/// flags acknowledged) to minimize boilerplate.
///
/// # Example
///
/// ```ignore
/// let policy = PolicyBuilder::test_default(&work_dir)
///     .with_allowed_exec(vec!["/bin/cat".into()])
///     .with_write_access(vec![artifacts_dir.display().to_string()])
///     .with_timeout_ms(5000)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct PolicyBuilder {
    work_dir: String,
    allowed_exec: Vec<String>,
    allowed_read: Vec<String>,
    allowed_write: Vec<String>,
    max_runtime_ms: u64,
    max_steps: u64,
    max_output_bytes: u64,
    max_snapshot_bytes: u64,
    max_wait_ms: u64,
}

impl PolicyBuilder {
    /// Create a builder with test-friendly defaults.
    ///
    /// Defaults:
    /// - Sandbox: `None` (with unsafe ack)
    /// - Network: `Disabled` (with unsafe ack)
    /// - FS read: work directory
    /// - FS write: empty (no write access)
    /// - Budgets: 60s runtime, 10000 steps, 8MB output
    #[must_use]
    pub fn test_default(work_dir: &Path) -> Self {
        let work_dir_str = work_dir.display().to_string();
        Self {
            work_dir: work_dir_str.clone(),
            allowed_exec: Vec::new(),
            allowed_read: vec![work_dir_str],
            allowed_write: Vec::new(),
            max_runtime_ms: 60_000,
            max_steps: 10_000,
            max_output_bytes: 8 * 1024 * 1024,
            max_snapshot_bytes: 1024 * 1024,
            max_wait_ms: 30_000,
        }
    }

    /// Set the allowed executables.
    #[must_use]
    pub fn with_allowed_exec(mut self, execs: Vec<String>) -> Self {
        self.allowed_exec = execs;
        self
    }

    /// Add paths to the read allowlist (in addition to work directory).
    #[must_use]
    pub fn with_read_access(mut self, paths: Vec<String>) -> Self {
        self.allowed_read.extend(paths);
        self
    }

    /// Set write access paths (automatically acknowledges write access).
    #[must_use]
    pub fn with_write_access(mut self, paths: Vec<String>) -> Self {
        self.allowed_write = paths;
        self
    }

    /// Set the maximum runtime in milliseconds.
    #[must_use]
    pub fn with_timeout_ms(mut self, ms: u64) -> Self {
        self.max_runtime_ms = ms;
        self
    }

    /// Set the maximum number of steps.
    #[must_use]
    pub fn with_max_steps(mut self, steps: u64) -> Self {
        self.max_steps = steps;
        self
    }

    /// Set the maximum output bytes.
    #[must_use]
    pub fn with_max_output_bytes(mut self, bytes: u64) -> Self {
        self.max_output_bytes = bytes;
        self
    }

    /// Build the final [`Policy`].
    #[must_use]
    pub fn build(self) -> Policy {
        let has_write = !self.allowed_write.is_empty();

        Policy {
            policy_version: POLICY_VERSION,
            sandbox: SandboxMode::None,
            sandbox_unsafe_ack: true,
            network: NetworkPolicy::Disabled,
            network_unsafe_ack: true,
            fs: FsPolicy {
                allowed_read: self.allowed_read,
                allowed_write: self.allowed_write,
                working_dir: Some(self.work_dir),
            },
            fs_write_unsafe_ack: has_write,
            fs_strict_write: false,
            exec: ExecPolicy {
                allowed_executables: self.allowed_exec,
                allow_shell: false,
            },
            env: EnvPolicy {
                allowlist: Vec::new(),
                set: std::collections::BTreeMap::new(),
                inherit: false,
            },
            budgets: Budgets {
                max_runtime_ms: self.max_runtime_ms,
                max_steps: self.max_steps,
                max_output_bytes: self.max_output_bytes,
                max_snapshot_bytes: self.max_snapshot_bytes,
                max_wait_ms: self.max_wait_ms,
            },
            artifacts: ArtifactsPolicy::default(),
            replay: ReplayPolicy::default(),
        }
    }
}

// ============================================================================
// StepBuilder
// ============================================================================

/// Fluent builder for constructing [`Step`] objects.
///
/// Provides factory methods for common action types and chainable methods
/// for adding assertions and timeouts.
///
/// # Example
///
/// ```ignore
/// let step = StepBuilder::text("type-hello", "hello world")
///     .assert_screen_contains("hello")
///     .with_timeout_ms(2000)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct StepBuilder {
    name: String,
    action_type: ActionType,
    payload: serde_json::Value,
    assertions: Vec<Assertion>,
    timeout_ms: u64,
    retries: u32,
}

impl StepBuilder {
    /// Create a text input action step.
    #[must_use]
    pub fn text(name: &str, text: &str) -> Self {
        Self {
            name: name.to_string(),
            action_type: ActionType::Text,
            payload: serde_json::json!({ "text": text }),
            assertions: Vec::new(),
            timeout_ms: 1000,
            retries: 0,
        }
    }

    /// Create a key input action step.
    #[must_use]
    pub fn key(name: &str, key: &str) -> Self {
        Self {
            name: name.to_string(),
            action_type: ActionType::Key,
            payload: serde_json::json!({ "key": key }),
            assertions: Vec::new(),
            timeout_ms: 1000,
            retries: 0,
        }
    }

    /// Create a wait action that waits for text to appear on screen.
    #[must_use]
    pub fn wait_for_text(name: &str, text: &str) -> Self {
        Self {
            name: name.to_string(),
            action_type: ActionType::Wait,
            payload: serde_json::json!({
                "condition": {
                    "type": "screen_contains",
                    "payload": { "text": text }
                }
            }),
            assertions: Vec::new(),
            timeout_ms: 5000,
            retries: 0,
        }
    }

    /// Create a wait action that waits for the process to exit.
    #[must_use]
    pub fn wait_for_exit(name: &str) -> Self {
        Self {
            name: name.to_string(),
            action_type: ActionType::Wait,
            payload: serde_json::json!({
                "condition": { "type": "process_exited" }
            }),
            assertions: Vec::new(),
            timeout_ms: 5000,
            retries: 0,
        }
    }

    /// Create a resize action step.
    #[must_use]
    pub fn resize(name: &str, rows: u16, cols: u16) -> Self {
        Self {
            name: name.to_string(),
            action_type: ActionType::Resize,
            payload: serde_json::json!({ "rows": rows, "cols": cols }),
            assertions: Vec::new(),
            timeout_ms: 1000,
            retries: 0,
        }
    }

    /// Create a terminate action step.
    #[must_use]
    pub fn terminate(name: &str) -> Self {
        Self {
            name: name.to_string(),
            action_type: ActionType::Terminate,
            payload: serde_json::json!({}),
            assertions: Vec::new(),
            timeout_ms: 1000,
            retries: 0,
        }
    }

    /// Add an assertion to this step.
    #[must_use]
    pub fn with_assertion(mut self, assertion: Assertion) -> Self {
        self.assertions.push(assertion);
        self
    }

    /// Add a `screen_contains` assertion.
    #[must_use]
    pub fn assert_screen_contains(mut self, text: &str) -> Self {
        self.assertions.push(Assertion {
            assertion_type: "screen_contains".to_string(),
            payload: serde_json::json!({ "text": text }),
        });
        self
    }

    /// Add a `not_contains` assertion.
    #[must_use]
    pub fn assert_not_contains(mut self, text: &str) -> Self {
        self.assertions.push(Assertion {
            assertion_type: "not_contains".to_string(),
            payload: serde_json::json!({ "text": text }),
        });
        self
    }

    /// Set the step timeout in milliseconds.
    #[must_use]
    pub fn with_timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }

    /// Set the number of retries for this step.
    #[must_use]
    pub fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }

    /// Build the final [`Step`].
    #[must_use]
    pub fn build(self) -> Step {
        Step {
            id: StepId::new(),
            name: self.name,
            action: Action {
                action_type: self.action_type,
                payload: self.payload,
            },
            assert: self.assertions,
            timeout_ms: self.timeout_ms,
            retries: self.retries,
        }
    }
}

// ============================================================================
// ScenarioBuilder
// ============================================================================

/// Fluent builder for constructing [`Scenario`] objects.
///
/// # Example
///
/// ```ignore
/// let scenario = ScenarioBuilder::new("echo-test", "/bin/echo")
///     .with_args(vec!["hello".into()])
///     .with_policy(policy)
///     .add_step(StepBuilder::wait_for_exit("wait").build())
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct ScenarioBuilder {
    name: String,
    description: Option<String>,
    command: String,
    args: Vec<String>,
    cwd: Option<String>,
    initial_size: TerminalSize,
    policy: Option<Policy>,
    steps: Vec<Step>,
}

impl ScenarioBuilder {
    /// Create a new scenario builder.
    ///
    /// # Arguments
    ///
    /// * `name` - The scenario name (for identification)
    /// * `command` - The command to execute
    #[must_use]
    pub fn new(name: &str, command: &str) -> Self {
        Self {
            name: name.to_string(),
            description: None,
            command: command.to_string(),
            args: Vec::new(),
            cwd: None,
            initial_size: TerminalSize::default(),
            policy: None,
            steps: Vec::new(),
        }
    }

    /// Set the scenario description.
    #[must_use]
    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    /// Set the command arguments.
    #[must_use]
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Set the working directory.
    #[must_use]
    pub fn with_cwd(mut self, cwd: &Path) -> Self {
        self.cwd = Some(cwd.display().to_string());
        self
    }

    /// Set the initial terminal size.
    #[must_use]
    pub fn with_size(mut self, rows: u16, cols: u16) -> Self {
        self.initial_size = TerminalSize { rows, cols };
        self
    }

    /// Set the policy for this scenario.
    #[must_use]
    pub fn with_policy(mut self, policy: Policy) -> Self {
        self.policy = Some(policy);
        self
    }

    /// Add a step to the scenario.
    #[must_use]
    pub fn add_step(mut self, step: Step) -> Self {
        self.steps.push(step);
        self
    }

    /// Add a text input step.
    #[must_use]
    pub fn add_text(mut self, name: &str, text: &str) -> Self {
        self.steps.push(StepBuilder::text(name, text).build());
        self
    }

    /// Add a key input step.
    #[must_use]
    pub fn add_key(mut self, name: &str, key: &str) -> Self {
        self.steps.push(StepBuilder::key(name, key).build());
        self
    }

    /// Add a wait-for-text step.
    #[must_use]
    pub fn add_wait_for_text(mut self, name: &str, text: &str) -> Self {
        self.steps.push(StepBuilder::wait_for_text(name, text).build());
        self
    }

    /// Add a wait-for-exit step.
    #[must_use]
    pub fn add_wait_for_exit(mut self, name: &str) -> Self {
        self.steps.push(StepBuilder::wait_for_exit(name).build());
        self
    }

    /// Add a terminate step.
    #[must_use]
    pub fn add_terminate(mut self, name: &str) -> Self {
        self.steps.push(StepBuilder::terminate(name).build());
        self
    }

    /// Build the final [`Scenario`].
    ///
    /// # Panics
    ///
    /// Panics if no policy was set.
    #[must_use]
    pub fn build(self) -> Scenario {
        #[allow(clippy::expect_used)]
        let policy = self.policy.expect("ScenarioBuilder requires a policy");

        Scenario {
            scenario_version: 1,
            metadata: ScenarioMetadata {
                name: self.name,
                description: self.description,
            },
            run: RunConfig {
                command: self.command,
                args: self.args,
                cwd: self.cwd,
                initial_size: self.initial_size,
                policy: PolicyRef::Inline(policy),
            },
            steps: self.steps,
        }
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn policy_builder_creates_valid_policy() {
        let dir = std::env::temp_dir();
        let policy = PolicyBuilder::test_default(&dir)
            .with_allowed_exec(vec!["/bin/echo".to_string()])
            .build();

        assert_eq!(policy.policy_version, POLICY_VERSION);
        assert!(matches!(policy.sandbox, SandboxMode::None));
        assert!(policy.sandbox_unsafe_ack);
        assert_eq!(policy.exec.allowed_executables, vec!["/bin/echo"]);
    }

    #[test]
    fn policy_builder_write_access_sets_ack() {
        let dir = std::env::temp_dir();
        let policy = PolicyBuilder::test_default(&dir)
            .with_write_access(vec!["/tmp/test".to_string()])
            .build();

        assert!(policy.fs_write_unsafe_ack);
        assert_eq!(policy.fs.allowed_write, vec!["/tmp/test"]);
    }

    #[test]
    fn step_builder_creates_text_step() {
        let step = StepBuilder::text("type", "hello").build();

        assert_eq!(step.name, "type");
        assert!(matches!(step.action.action_type, ActionType::Text));
    }

    #[test]
    fn step_builder_with_assertions() {
        let step = StepBuilder::text("type", "hello")
            .assert_screen_contains("hello")
            .assert_not_contains("goodbye")
            .build();

        assert_eq!(step.assert.len(), 2);
        assert_eq!(step.assert[0].assertion_type, "screen_contains");
        assert_eq!(step.assert[1].assertion_type, "not_contains");
    }

    #[test]
    fn scenario_builder_creates_scenario() {
        let dir = std::env::temp_dir();
        let policy = PolicyBuilder::test_default(&dir)
            .with_allowed_exec(vec!["/bin/echo".to_string()])
            .build();

        let scenario = ScenarioBuilder::new("test", "/bin/echo")
            .with_args(vec!["hello".to_string()])
            .with_policy(policy)
            .add_wait_for_exit("wait")
            .build();

        assert_eq!(scenario.metadata.name, "test");
        assert_eq!(scenario.run.command, "/bin/echo");
        assert_eq!(scenario.run.args, vec!["hello"]);
        assert_eq!(scenario.steps.len(), 1);
    }
}
