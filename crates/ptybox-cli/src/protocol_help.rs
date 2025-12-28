//! Protocol help generation for LLM consumption.
//!
//! Generates structured documentation of the ptybox protocol,
//! including schemas, examples, and error codes.

use ptybox::model::{
    POLICY_VERSION, PROTOCOL_VERSION, RUN_RESULT_VERSION, SCENARIO_VERSION, SNAPSHOT_VERSION,
};
use serde::Serialize;
use std::collections::BTreeMap;

/// Complete protocol documentation for LLM consumption.
#[derive(Debug, Serialize)]
pub struct ProtocolHelp {
    /// Current protocol version
    pub protocol_version: u32,
    /// All version numbers used in the protocol
    pub versions: Versions,
    /// Available CLI commands
    pub commands: BTreeMap<String, CommandHelp>,
    /// Schema definitions for all types
    pub schemas: BTreeMap<String, SchemaHelp>,
    /// Error codes and their meanings
    pub error_codes: BTreeMap<String, ErrorCodeHelp>,
    /// Working examples
    pub examples: BTreeMap<String, Example>,
    /// Quick start guide
    pub quickstart: Quickstart,
}

/// Version numbers for all protocol components.
#[derive(Debug, Serialize)]
pub struct Versions {
    pub protocol: u32,
    pub policy: u32,
    pub scenario: u32,
    pub snapshot: u32,
    pub run_result: u32,
}

/// Documentation for a CLI command.
#[derive(Debug, Serialize)]
pub struct CommandHelp {
    pub description: String,
    pub usage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_flags: Option<Vec<String>>,
    pub output: String,
}

/// Schema documentation for a type.
#[derive(Debug, Serialize)]
pub struct SchemaHelp {
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub types: Option<BTreeMap<String, TypeVariant>>,
}

/// A variant of a union type.
#[derive(Debug, Serialize)]
pub struct TypeVariant {
    pub payload: BTreeMap<String, String>,
}

/// Documentation for an error code.
#[derive(Debug, Serialize)]
pub struct ErrorCodeHelp {
    pub exit_code: u32,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub common_causes: Option<Vec<String>>,
}

/// A working example.
#[derive(Debug, Serialize)]
pub struct Example {
    pub description: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    pub expected: String,
}

/// Quick start guide for new users.
#[derive(Debug, Serialize)]
pub struct Quickstart {
    pub recommended_mode: String,
    pub reason: String,
    pub steps: Vec<String>,
    pub minimal_policy: serde_json::Value,
}

/// Generate complete protocol documentation.
#[must_use]
pub fn generate_protocol_help() -> ProtocolHelp {
    ProtocolHelp {
        protocol_version: PROTOCOL_VERSION,
        versions: Versions {
            protocol: PROTOCOL_VERSION,
            policy: POLICY_VERSION,
            scenario: SCENARIO_VERSION,
            snapshot: SNAPSHOT_VERSION,
            run_result: RUN_RESULT_VERSION,
        },
        commands: generate_commands(),
        schemas: generate_schemas(),
        error_codes: generate_error_codes(),
        examples: generate_examples(),
        quickstart: generate_quickstart(),
    }
}

fn generate_commands() -> BTreeMap<String, CommandHelp> {
    let mut commands = BTreeMap::new();

    commands.insert(
        "driver".to_string(),
        CommandHelp {
            description:
                "Interactive NDJSON driver for step-by-step TUI control. Best for LLM agents."
                    .to_string(),
            usage: "ptybox driver --stdio --json -- <command> [args...]".to_string(),
            required_flags: Some(vec!["--stdio".to_string(), "--json".to_string()]),
            output: "NDJSON stream of Observation messages on stdout".to_string(),
        },
    );

    commands.insert(
        "exec".to_string(),
        CommandHelp {
            description: "Run a command under policy control and wait for exit.".to_string(),
            usage: "ptybox exec --json [--policy <path>] -- <command> [args...]".to_string(),
            required_flags: None,
            output: "Single RunResult JSON object".to_string(),
        },
    );

    commands.insert(
        "run".to_string(),
        CommandHelp {
            description: "Execute a scenario file with steps and assertions.".to_string(),
            usage: "ptybox run --json --scenario <path>".to_string(),
            required_flags: None,
            output: "Single RunResult JSON object".to_string(),
        },
    );

    commands.insert(
        "replay".to_string(),
        CommandHelp {
            description: "Compare artifacts against a baseline for regression testing.".to_string(),
            usage: "ptybox replay --artifacts <dir> [--normalize <filter>...]".to_string(),
            required_flags: None,
            output: "Replay comparison result".to_string(),
        },
    );

    commands
}

/// Generate schema documentation.
///
/// This function builds static schema definitions - allowed to be long since it's declarative data.
#[allow(clippy::too_many_lines)]
fn generate_schemas() -> BTreeMap<String, SchemaHelp> {
    let mut schemas = BTreeMap::new();

    // DriverInput schema
    let mut driver_input_fields = BTreeMap::new();
    driver_input_fields.insert(
        "protocol_version".to_string(),
        "u32 (must be 1)".to_string(),
    );
    driver_input_fields.insert("action".to_string(), "Action object".to_string());
    schemas.insert(
        "DriverInput".to_string(),
        SchemaHelp {
            description: "Input message sent to driver via stdin (NDJSON).".to_string(),
            fields: Some(driver_input_fields),
            types: None,
        },
    );

    // Action schema with all variants
    let mut action_types = BTreeMap::new();

    let mut key_payload = BTreeMap::new();
    key_payload.insert("key".to_string(), "string: Enter, Up, Down, Left, Right, Tab, Escape, Backspace, Delete, Home, End, PageUp, PageDown, or single character".to_string());
    action_types.insert(
        "key".to_string(),
        TypeVariant {
            payload: key_payload,
        },
    );

    let mut text_payload = BTreeMap::new();
    text_payload.insert("text".to_string(), "string: text to type".to_string());
    action_types.insert(
        "text".to_string(),
        TypeVariant {
            payload: text_payload,
        },
    );

    let mut resize_payload = BTreeMap::new();
    resize_payload.insert("rows".to_string(), "u16: terminal height".to_string());
    resize_payload.insert("cols".to_string(), "u16: terminal width".to_string());
    action_types.insert(
        "resize".to_string(),
        TypeVariant {
            payload: resize_payload,
        },
    );

    let mut wait_payload = BTreeMap::new();
    wait_payload.insert("condition".to_string(), "Condition object".to_string());
    action_types.insert(
        "wait".to_string(),
        TypeVariant {
            payload: wait_payload,
        },
    );

    let terminate_payload = BTreeMap::new();
    action_types.insert(
        "terminate".to_string(),
        TypeVariant {
            payload: terminate_payload,
        },
    );

    schemas.insert(
        "Action".to_string(),
        SchemaHelp {
            description: "Action to perform on the terminal session.".to_string(),
            fields: None,
            types: Some(action_types),
        },
    );

    // Condition schema
    let mut condition_types = BTreeMap::new();

    let mut screen_contains_payload = BTreeMap::new();
    screen_contains_payload.insert(
        "text".to_string(),
        "string: substring to find on screen".to_string(),
    );
    condition_types.insert(
        "screen_contains".to_string(),
        TypeVariant {
            payload: screen_contains_payload,
        },
    );

    let mut screen_matches_payload = BTreeMap::new();
    screen_matches_payload.insert(
        "pattern".to_string(),
        "string: Rust regex pattern".to_string(),
    );
    condition_types.insert(
        "screen_matches".to_string(),
        TypeVariant {
            payload: screen_matches_payload,
        },
    );

    let mut cursor_at_payload = BTreeMap::new();
    cursor_at_payload.insert("row".to_string(), "u16: cursor row (0-based)".to_string());
    cursor_at_payload.insert(
        "col".to_string(),
        "u16: cursor column (0-based)".to_string(),
    );
    condition_types.insert(
        "cursor_at".to_string(),
        TypeVariant {
            payload: cursor_at_payload,
        },
    );

    let process_exited_payload = BTreeMap::new();
    condition_types.insert(
        "process_exited".to_string(),
        TypeVariant {
            payload: process_exited_payload,
        },
    );

    schemas.insert(
        "Condition".to_string(),
        SchemaHelp {
            description: "Wait condition for the wait action.".to_string(),
            fields: None,
            types: Some(condition_types),
        },
    );

    // Observation schema
    let mut observation_fields = BTreeMap::new();
    observation_fields.insert("protocol_version".to_string(), "u32".to_string());
    observation_fields.insert("run_id".to_string(), "string (UUID)".to_string());
    observation_fields.insert("session_id".to_string(), "string (UUID)".to_string());
    observation_fields.insert(
        "timestamp_ms".to_string(),
        "u64: milliseconds since run start".to_string(),
    );
    observation_fields.insert("screen".to_string(), "ScreenSnapshot object".to_string());
    observation_fields.insert(
        "transcript_delta".to_string(),
        "string | null: new output since last observation".to_string(),
    );
    observation_fields.insert("events".to_string(), "array of Event objects".to_string());
    schemas.insert(
        "Observation".to_string(),
        SchemaHelp {
            description: "Terminal state observation returned after each action.".to_string(),
            fields: Some(observation_fields),
            types: None,
        },
    );

    // ScreenSnapshot schema
    let mut screen_fields = BTreeMap::new();
    screen_fields.insert("rows".to_string(), "u16: terminal height".to_string());
    screen_fields.insert("cols".to_string(), "u16: terminal width".to_string());
    screen_fields.insert(
        "lines".to_string(),
        "array of strings: screen content".to_string(),
    );
    screen_fields.insert(
        "cursor".to_string(),
        "object: {row, col, visible}".to_string(),
    );
    screen_fields.insert(
        "alternate_screen".to_string(),
        "bool: true if alternate screen buffer active".to_string(),
    );
    schemas.insert(
        "ScreenSnapshot".to_string(),
        SchemaHelp {
            description: "Current terminal screen state.".to_string(),
            fields: Some(screen_fields),
            types: None,
        },
    );

    schemas
}

/// Generate error code documentation.
///
/// This function builds static error code definitions - allowed to be long since it's declarative data.
#[allow(clippy::too_many_lines)]
fn generate_error_codes() -> BTreeMap<String, ErrorCodeHelp> {
    let mut codes = BTreeMap::new();

    codes.insert(
        "E_POLICY_DENIED".to_string(),
        ErrorCodeHelp {
            exit_code: 2,
            description: "Policy validation failed.".to_string(),
            common_causes: Some(vec![
                "Path not in allowed_read or allowed_write".to_string(),
                "Executable not in allowed_executables".to_string(),
                "Network access without acknowledgement".to_string(),
                "Sandbox disabled without acknowledgement".to_string(),
                "Path under /Users, /System, /Library (blocked roots)".to_string(),
            ]),
        },
    );

    codes.insert(
        "E_SANDBOX_UNAVAILABLE".to_string(),
        ErrorCodeHelp {
            exit_code: 3,
            description: "Sandbox not available on this platform.".to_string(),
            common_causes: Some(vec![
                "Running on Linux without container support".to_string()
            ]),
        },
    );

    codes.insert(
        "E_TIMEOUT".to_string(),
        ErrorCodeHelp {
            exit_code: 4,
            description: "Budget exceeded (runtime, wait, output).".to_string(),
            common_causes: Some(vec![
                "Wait condition never satisfied".to_string(),
                "Process running longer than max_runtime_ms".to_string(),
                "Output exceeded max_output_bytes".to_string(),
            ]),
        },
    );

    codes.insert(
        "E_ASSERTION_FAILED".to_string(),
        ErrorCodeHelp {
            exit_code: 5,
            description: "Scenario assertion check failed.".to_string(),
            common_causes: Some(vec![
                "screen_contains text not found".to_string(),
                "regex_match pattern didn't match".to_string(),
                "cursor_at position incorrect".to_string(),
            ]),
        },
    );

    codes.insert(
        "E_PROCESS_EXIT".to_string(),
        ErrorCodeHelp {
            exit_code: 6,
            description: "Process exited unexpectedly.".to_string(),
            common_causes: Some(vec![
                "Command not found".to_string(),
                "Permission denied".to_string(),
                "Crash or error exit".to_string(),
            ]),
        },
    );

    codes.insert(
        "E_TERMINAL_PARSE".to_string(),
        ErrorCodeHelp {
            exit_code: 7,
            description: "Terminal output parsing failed.".to_string(),
            common_causes: Some(vec!["Invalid UTF-8 in output".to_string()]),
        },
    );

    codes.insert(
        "E_PROTOCOL_VERSION_MISMATCH".to_string(),
        ErrorCodeHelp {
            exit_code: 8,
            description: "Protocol version in message doesn't match.".to_string(),
            common_causes: Some(vec![
                "protocol_version field missing or wrong value".to_string()
            ]),
        },
    );

    codes.insert(
        "E_PROTOCOL".to_string(),
        ErrorCodeHelp {
            exit_code: 9,
            description: "Protocol error (malformed message).".to_string(),
            common_causes: Some(vec![
                "Invalid JSON".to_string(),
                "Missing required fields".to_string(),
                "Invalid action type".to_string(),
                "Invalid payload format".to_string(),
            ]),
        },
    );

    codes.insert(
        "E_IO".to_string(),
        ErrorCodeHelp {
            exit_code: 10,
            description: "I/O error (file, PTY, network).".to_string(),
            common_causes: Some(vec![
                "File not found".to_string(),
                "Permission denied".to_string(),
                "PTY creation failed".to_string(),
            ]),
        },
    );

    codes.insert(
        "E_REPLAY_MISMATCH".to_string(),
        ErrorCodeHelp {
            exit_code: 11,
            description: "Replay comparison found differences.".to_string(),
            common_causes: Some(vec![
                "Screen content changed".to_string(),
                "Timestamps differ (use --normalize)".to_string(),
            ]),
        },
    );

    codes
}

fn generate_examples() -> BTreeMap<String, Example> {
    let mut examples = BTreeMap::new();

    examples.insert(
        "driver_text".to_string(),
        Example {
            description: "Send text to a process using driver mode.".to_string(),
            command: "ptybox driver --stdio --json -- /bin/cat".to_string(),
            input: Some(serde_json::json!({
                "protocol_version": 1,
                "action": {
                    "type": "text",
                    "payload": {"text": "hello\n"}
                }
            })),
            expected: "Observation with screen.lines containing 'hello'".to_string(),
        },
    );

    examples.insert(
        "driver_key".to_string(),
        Example {
            description: "Send a key press to a process.".to_string(),
            command: "ptybox driver --stdio --json -- /bin/bash".to_string(),
            input: Some(serde_json::json!({
                "protocol_version": 1,
                "action": {
                    "type": "key",
                    "payload": {"key": "Enter"}
                }
            })),
            expected: "Observation showing shell prompt on new line".to_string(),
        },
    );

    examples.insert(
        "driver_wait".to_string(),
        Example {
            description: "Wait for text to appear on screen.".to_string(),
            command: "ptybox driver --stdio --json -- /bin/bash".to_string(),
            input: Some(serde_json::json!({
                "protocol_version": 1,
                "action": {
                    "type": "wait",
                    "payload": {
                        "condition": {
                            "type": "screen_contains",
                            "payload": {"text": "$"}
                        }
                    }
                }
            })),
            expected: "Observation once screen contains '$'".to_string(),
        },
    );

    examples.insert(
        "exec_simple".to_string(),
        Example {
            description: "Run a command and capture output.".to_string(),
            command: "ptybox exec --json -- /bin/echo hello".to_string(),
            input: None,
            expected: "RunResult with status 'passed' and screen containing 'hello'".to_string(),
        },
    );

    examples
}

fn generate_quickstart() -> Quickstart {
    Quickstart {
        recommended_mode: "driver".to_string(),
        reason: "Interactive control with immediate feedback after each action.".to_string(),
        steps: vec![
            "1. Copy binary to /tmp (paths under /Users are blocked by policy)".to_string(),
            "2. Start driver: ptybox driver --stdio --json -- /tmp/your-app".to_string(),
            "3. Send NDJSON actions to stdin, receive Observations on stdout".to_string(),
            "4. Use 'wait' action with screen_contains to wait for UI updates".to_string(),
            "5. Send 'terminate' action when done".to_string(),
        ],
        minimal_policy: serde_json::json!({
            "policy_version": 3,
            "sandbox": "none",
            "sandbox_unsafe_ack": true,
            "network": "disabled",
            "network_unsafe_ack": true,
            "fs": {
                "allowed_read": ["/tmp"],
                "allowed_write": ["/tmp"],
                "working_dir": "/tmp"
            },
            "fs_write_unsafe_ack": true,
            "exec": {
                "allowed_executables": ["/tmp/your-app"],
                "allow_shell": false
            },
            "env": {
                "allowlist": [],
                "set": {},
                "inherit": false
            },
            "budgets": {
                "max_runtime_ms": 60_000,
                "max_steps": 10_000,
                "max_output_bytes": 8_388_608,
                "max_snapshot_bytes": 2_097_152,
                "max_wait_ms": 10_000
            },
            "artifacts": {
                "enabled": false,
                "dir": null,
                "overwrite": false
            },
            "replay": {
                "strict": false
            }
        }),
    }
}
