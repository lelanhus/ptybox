use crate::model::policy::Policy;
use crate::model::terminal::TerminalSize;
use crate::model::{RunId, StepId};
use serde::{Deserialize, Serialize};

/// Scenario format version.
pub const SCENARIO_VERSION: u32 = 1;

/// Observation format version.
///
/// Observations returned by [`Session::observe()`] include `protocol_version`
/// for overall format, but this constant tracks the observation-specific schema.
pub const OBSERVATION_VERSION: u32 = 1;

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

// =============================================================================
// Action Constructors
// =============================================================================

impl Action {
    /// Create a key press action.
    ///
    /// # Examples
    /// ```ignore
    /// let enter = Action::key("Enter");
    /// let char_a = Action::key("a");
    /// ```
    #[must_use]
    pub fn key(key: &str) -> Self {
        Self {
            action_type: ActionType::Key,
            payload: serde_json::json!({"key": key}),
        }
    }

    /// Create a text input action.
    ///
    /// # Examples
    /// ```ignore
    /// let action = Action::text("hello world");
    /// ```
    #[must_use]
    pub fn text(text: &str) -> Self {
        Self {
            action_type: ActionType::Text,
            payload: serde_json::json!({"text": text}),
        }
    }

    /// Create a terminal resize action.
    ///
    /// # Examples
    /// ```ignore
    /// let action = Action::resize(24, 80);
    /// ```
    #[must_use]
    pub fn resize(rows: u16, cols: u16) -> Self {
        Self {
            action_type: ActionType::Resize,
            payload: serde_json::json!({"rows": rows, "cols": cols}),
        }
    }

    /// Create a wait action with screen contains condition.
    ///
    /// # Examples
    /// ```ignore
    /// let action = Action::wait_for_text("Ready");
    /// ```
    #[must_use]
    pub fn wait_for_text(text: &str) -> Self {
        Self {
            action_type: ActionType::Wait,
            payload: serde_json::json!({
                "condition": {
                    "type": "screen_contains",
                    "text": text
                }
            }),
        }
    }

    /// Create a wait action with regex match condition.
    ///
    /// # Examples
    /// ```ignore
    /// let action = Action::wait_for_regex(r"\d+\.\d+");
    /// ```
    #[must_use]
    pub fn wait_for_regex(pattern: &str) -> Self {
        Self {
            action_type: ActionType::Wait,
            payload: serde_json::json!({
                "condition": {
                    "type": "regex_match",
                    "pattern": pattern
                }
            }),
        }
    }

    /// Create a wait action with cursor position condition.
    ///
    /// # Examples
    /// ```ignore
    /// let action = Action::wait_for_cursor(0, 0);
    /// ```
    #[must_use]
    pub fn wait_for_cursor(row: u16, col: u16) -> Self {
        Self {
            action_type: ActionType::Wait,
            payload: serde_json::json!({
                "condition": {
                    "type": "cursor_at",
                    "row": row,
                    "col": col
                }
            }),
        }
    }

    /// Create a process termination action.
    #[must_use]
    pub fn terminate() -> Self {
        Self {
            action_type: ActionType::Terminate,
            payload: serde_json::json!({}),
        }
    }
}

// =============================================================================
// Assertion Constructors
// =============================================================================

impl Assertion {
    /// Assert that the screen contains the given text.
    ///
    /// # Examples
    /// ```ignore
    /// let assertion = Assertion::screen_contains("Welcome");
    /// ```
    #[must_use]
    pub fn screen_contains(text: &str) -> Self {
        Self {
            assertion_type: "screen_contains".to_string(),
            payload: serde_json::json!({"text": text}),
        }
    }

    /// Assert that the screen does NOT contain the given text.
    ///
    /// # Examples
    /// ```ignore
    /// let assertion = Assertion::not_contains("Error");
    /// ```
    #[must_use]
    pub fn not_contains(text: &str) -> Self {
        Self {
            assertion_type: "not_contains".to_string(),
            payload: serde_json::json!({"text": text}),
        }
    }

    /// Assert that the screen matches the given regex pattern.
    ///
    /// # Examples
    /// ```ignore
    /// let assertion = Assertion::regex_match(r"v\d+\.\d+\.\d+");
    /// ```
    #[must_use]
    pub fn regex_match(pattern: &str) -> Self {
        Self {
            assertion_type: "regex_match".to_string(),
            payload: serde_json::json!({"pattern": pattern}),
        }
    }

    /// Assert that the cursor is at the given position.
    ///
    /// # Examples
    /// ```ignore
    /// let assertion = Assertion::cursor_at(0, 0);
    /// ```
    #[must_use]
    pub fn cursor_at(row: u16, col: u16) -> Self {
        Self {
            assertion_type: "cursor_at".to_string(),
            payload: serde_json::json!({"row": row, "col": col}),
        }
    }

    /// Assert that a specific line equals the given text.
    ///
    /// # Examples
    /// ```ignore
    /// let assertion = Assertion::line_equals(0, "Hello World");
    /// ```
    #[must_use]
    pub fn line_equals(line: usize, text: &str) -> Self {
        Self {
            assertion_type: "line_equals".to_string(),
            payload: serde_json::json!({"line": line, "text": text}),
        }
    }

    /// Assert that a specific line contains the given text.
    ///
    /// # Examples
    /// ```ignore
    /// let assertion = Assertion::line_contains(0, "Hello");
    /// ```
    #[must_use]
    pub fn line_contains(line: usize, text: &str) -> Self {
        Self {
            assertion_type: "line_contains".to_string(),
            payload: serde_json::json!({"line": line, "text": text}),
        }
    }

    /// Assert that a specific line matches the given regex pattern.
    ///
    /// # Examples
    /// ```ignore
    /// let assertion = Assertion::line_matches(0, r"^\d+$");
    /// ```
    #[must_use]
    pub fn line_matches(line: usize, pattern: &str) -> Self {
        Self {
            assertion_type: "line_matches".to_string(),
            payload: serde_json::json!({"line": line, "pattern": pattern}),
        }
    }

    /// Assert that the screen is empty (all whitespace).
    #[must_use]
    pub fn screen_empty() -> Self {
        Self {
            assertion_type: "screen_empty".to_string(),
            payload: serde_json::json!({}),
        }
    }

    /// Assert that the cursor is visible.
    #[must_use]
    pub fn cursor_visible() -> Self {
        Self {
            assertion_type: "cursor_visible".to_string(),
            payload: serde_json::json!({}),
        }
    }

    /// Assert that the cursor is hidden.
    #[must_use]
    pub fn cursor_hidden() -> Self {
        Self {
            assertion_type: "cursor_hidden".to_string(),
            payload: serde_json::json!({}),
        }
    }
}
