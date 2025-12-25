use crate::model::policy::Policy;
use crate::model::scenario::Action;
use crate::model::{Observation, RunId};
use serde::{Deserialize, Serialize};

/// Summary of a completed run, suitable for JSON output.
///
/// Contains the effective policy, step results (if scenario mode),
/// final observation, exit status, and any error information.
/// This is the primary output type for both `run` and `exec` commands.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunResult {
    /// Run result format version.
    pub run_result_version: u32,
    /// Protocol version for output format.
    pub protocol_version: u32,
    /// Unique identifier for this run.
    pub run_id: RunId,
    /// Overall run status.
    pub status: RunStatus,
    /// Monotonic timestamp when run started (ms since process start).
    pub started_at_ms: u64,
    /// Monotonic timestamp when run ended.
    pub ended_at_ms: u64,
    /// Command that was executed.
    pub command: String,
    /// Arguments passed to command.
    pub args: Vec<String>,
    /// Working directory for execution.
    pub cwd: String,
    /// Effective policy used for this run.
    pub policy: Policy,
    /// Resolved scenario (present in scenario mode).
    pub scenario: Option<crate::model::Scenario>,
    /// Step results (present in scenario mode).
    pub steps: Option<Vec<StepResult>>,
    /// Final terminal observation before exit.
    pub final_observation: Option<Observation>,
    /// Process exit status.
    pub exit_status: Option<ExitStatus>,
    /// Error information (present when status is not Passed).
    pub error: Option<ErrorInfo>,
}

/// Overall run status.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// All steps passed or command exited successfully.
    Passed,
    /// One or more assertions failed.
    Failed,
    /// An error occurred during execution.
    Errored,
    /// Run was canceled.
    Canceled,
}

/// Result of a single scenario step.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StepResult {
    /// Step identifier from scenario.
    pub step_id: crate::model::StepId,
    /// Human-readable step name.
    pub name: String,
    /// Step outcome.
    pub status: StepStatus,
    /// Number of attempts made (including retries).
    pub attempts: u32,
    /// When step started (ms since run start).
    pub started_at_ms: u64,
    /// When step ended.
    pub ended_at_ms: u64,
    /// Action that was performed.
    pub action: Action,
    /// Results of each assertion.
    pub assertions: Vec<AssertionResult>,
    /// Error information if step failed.
    pub error: Option<ErrorInfo>,
}

/// Individual step status.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    /// Step completed with all assertions passing.
    Passed,
    /// One or more assertions failed.
    Failed,
    /// An error occurred during step execution.
    Errored,
    /// Step was skipped (e.g., after earlier failure).
    Skipped,
}

/// Result of evaluating a single assertion.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssertionResult {
    /// Assertion type (e.g., `screen_contains`).
    #[serde(rename = "type")]
    pub assertion_type: String,
    /// Whether the assertion passed.
    pub passed: bool,
    /// Human-readable message (typically on failure).
    pub message: Option<String>,
    /// Structured diagnostic details.
    pub details: Option<serde_json::Value>,
}

/// Process exit status.
///
/// `terminated_by_harness` indicates whether tui-use forcibly killed the process
/// (e.g., due to timeout or error recovery) rather than natural exit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExitStatus {
    /// Whether the process exited successfully (code 0).
    pub success: bool,
    /// Exit code (when exited normally).
    pub exit_code: Option<i32>,
    /// Signal number (when terminated by signal).
    pub signal: Option<i32>,
    /// True when tui-use killed the process (timeout, error recovery).
    pub terminated_by_harness: bool,
}

/// Error information with stable code for automation.
///
/// Error codes are stable and can be used for programmatic error handling.
/// See `spec/data-model.md` for the complete list of error codes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErrorInfo {
    /// Stable error code (e.g., `E_POLICY_DENIED`, `E_TIMEOUT`).
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Structured context (step details, policy info, etc.).
    pub context: Option<serde_json::Value>,
}

/// Protocol version for JSON/NDJSON output format.
pub const PROTOCOL_VERSION: u32 = 1;
/// Run result format version.
pub const RUN_RESULT_VERSION: u32 = 1;
