use crate::model::policy::Policy;
use crate::model::terminal::TerminalSize;
use crate::model::{RunId, StepId};
use serde::{Deserialize, Serialize};

/// Scenario format version.
pub const SCENARIO_VERSION: u32 = 1;

/// Declarative scenario for driving TUI applications.
///
/// Scenarios define a sequence of steps (actions + assertions) to execute
/// against a command. They support YAML or JSON format.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scenario {
    /// Scenario format version for compatibility.
    pub scenario_version: u32,
    /// Scenario metadata (name, description).
    pub metadata: ScenarioMetadata,
    /// Command execution configuration.
    pub run: RunConfig,
    /// Ordered list of steps to execute.
    pub steps: Vec<Step>,
}

/// Scenario metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScenarioMetadata {
    /// Scenario name for identification.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
}

/// Command execution configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunConfig {
    /// Command to execute (absolute path).
    pub command: String,
    /// Command arguments.
    pub args: Vec<String>,
    /// Working directory (absolute path).
    pub cwd: Option<String>,
    /// Initial terminal size.
    pub initial_size: TerminalSize,
    /// Policy configuration (inline or file reference).
    pub policy: PolicyRef,
}

/// Policy reference - either inline or file path.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PolicyRef {
    /// Inline policy object.
    Inline(Policy),
    /// Path to policy file.
    File { path: String },
}

/// Single scenario step with action and assertions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Step {
    /// Unique step identifier within scenario.
    pub id: StepId,
    /// Human-readable step name.
    pub name: String,
    /// Action to perform.
    pub action: Action,
    /// Assertions to verify after action (all must pass).
    #[serde(default)]
    pub assert: Vec<Assertion>,
    /// Step timeout in milliseconds.
    pub timeout_ms: u64,
    /// Number of retries for flaky assertions.
    pub retries: u32,
}

/// Action to send to the terminal session.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Action {
    /// Action type.
    #[serde(rename = "type")]
    pub action_type: ActionType,
    /// Type-specific payload.
    pub payload: serde_json::Value,
}

/// Action type enumeration.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    /// Press a key (payload: `{key: "Enter"}` or `{key: "a"}`).
    Key,
    /// Type text (payload: `{text: "hello"}`).
    Text,
    /// Resize terminal (payload: `{rows: 24, cols: 80}`).
    Resize,
    /// Wait for condition (payload: `{condition: {type: "screen_contains", payload: {...}}}`).
    Wait,
    /// Terminate process.
    Terminate,
}

/// Assertion to verify terminal state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Assertion {
    /// Assertion type (e.g., `screen_contains`, `cursor_at`).
    #[serde(rename = "type")]
    pub assertion_type: String,
    /// Type-specific payload.
    pub payload: serde_json::Value,
}

/// Terminal state observation returned by the session.
///
/// Contains screen snapshot, optional transcript delta, and events.
/// This is the primary output from `Session::observe()`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation {
    /// Protocol version.
    pub protocol_version: u32,
    /// Run identifier.
    pub run_id: RunId,
    /// Session identifier.
    pub session_id: crate::model::SessionId,
    /// Monotonic timestamp in milliseconds since run start.
    pub timestamp_ms: u64,
    /// Current screen snapshot.
    pub screen: crate::model::ScreenSnapshot,
    /// Incremental terminal output since last observation.
    pub transcript_delta: Option<String>,
    /// Events captured during observation.
    pub events: Vec<Event>,
}

/// Event emitted during observation.
///
/// Events capture notable occurrences like title changes or unsupported sequences.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    /// Event type identifier.
    #[serde(rename = "type")]
    pub event_type: String,
    /// Human-readable message.
    pub message: Option<String>,
    /// Structured event details.
    pub details: Option<serde_json::Value>,
}
