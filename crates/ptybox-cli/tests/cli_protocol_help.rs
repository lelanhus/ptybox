// Test module - relaxed lint rules
#![allow(clippy::default_trait_access)]
#![allow(clippy::indexing_slicing)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::inefficient_to_string)]
#![allow(clippy::panic)]
#![allow(clippy::manual_assert)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::cast_possible_truncation)]
#![allow(missing_docs)]

//! Tests for the `protocol-help` command.

use std::process::Command;

use ptybox::model::PROTOCOL_VERSION;

#[test]
fn protocol_help_json_output_valid() {
    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["protocol-help", "--json"])
        .output()
        .expect("failed to run command");

    assert!(output.status.success());

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("output should be valid JSON");

    // Should have top-level keys
    assert!(
        json["protocol_version"].is_number(),
        "missing protocol_version"
    );
    assert!(json["versions"].is_object(), "missing versions");
    assert!(json["commands"].is_object(), "missing commands");
    assert!(json["schemas"].is_object(), "missing schemas");
    assert!(json["error_codes"].is_object(), "missing error_codes");
}

#[test]
fn protocol_help_versions_match_constants() {
    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["protocol-help", "--json"])
        .output()
        .expect("failed to run command");

    assert!(output.status.success());

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    let reported_version = json["protocol_version"]
        .as_u64()
        .expect("protocol_version should be a number");

    assert_eq!(
        reported_version as u32, PROTOCOL_VERSION,
        "protocol_version in help should match PROTOCOL_VERSION constant"
    );

    let versions = &json["versions"];
    assert_eq!(
        versions["protocol"].as_u64().unwrap() as u32,
        PROTOCOL_VERSION,
        "versions.protocol should match"
    );
}

#[test]
fn protocol_help_commands_documented() {
    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["protocol-help", "--json"])
        .output()
        .expect("failed to run command");

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let commands = &json["commands"];

    // All main commands should be documented
    assert!(
        commands["driver"].is_object(),
        "driver command not documented"
    );
    assert!(commands["exec"].is_object(), "exec command not documented");
    assert!(commands["run"].is_object(), "run command not documented");
    assert!(
        commands["replay"].is_object(),
        "replay command not documented"
    );

    // Each command should have description and usage
    for cmd_name in ["driver", "exec", "run", "replay"] {
        let cmd = &commands[cmd_name];
        assert!(
            cmd["description"].is_string(),
            "{} should have description",
            cmd_name
        );
        assert!(cmd["usage"].is_string(), "{} should have usage", cmd_name);
    }
}

#[test]
fn protocol_help_action_types_documented() {
    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["protocol-help", "--json"])
        .output()
        .expect("failed to run command");

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let action_schema = &json["schemas"]["Action"]["types"];

    // All action types should be documented
    assert!(
        action_schema["key"].is_object(),
        "key action not documented"
    );
    assert!(
        action_schema["text"].is_object(),
        "text action not documented"
    );
    assert!(
        action_schema["resize"].is_object(),
        "resize action not documented"
    );
    assert!(
        action_schema["wait"].is_object(),
        "wait action not documented"
    );
    assert!(
        action_schema["terminate"].is_object(),
        "terminate action not documented"
    );
}

#[test]
fn protocol_help_error_codes_documented() {
    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["protocol-help", "--json"])
        .output()
        .expect("failed to run command");

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let error_codes = json["error_codes"]
        .as_object()
        .expect("error_codes should be object");

    // Should have all major error codes
    assert!(
        error_codes.contains_key("E_POLICY_DENIED"),
        "E_POLICY_DENIED not documented"
    );
    assert!(
        error_codes.contains_key("E_TIMEOUT"),
        "E_TIMEOUT not documented"
    );
    assert!(
        error_codes.contains_key("E_PROTOCOL"),
        "E_PROTOCOL not documented"
    );
    assert!(
        error_codes.contains_key("E_SANDBOX_UNAVAILABLE"),
        "E_SANDBOX_UNAVAILABLE not documented"
    );

    // Each error code should have exit_code and description
    for (code_name, error) in error_codes {
        assert!(
            error["exit_code"].is_number(),
            "{} should have exit_code",
            code_name
        );
        assert!(
            error["description"].is_string(),
            "{} should have description",
            code_name
        );
    }
}

#[test]
fn protocol_help_text_output_contains_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["protocol-help"])
        .output()
        .expect("failed to run command");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should mention key commands and concepts
    assert!(stdout.contains("driver"), "should mention driver command");
    assert!(stdout.contains("exec"), "should mention exec command");
    assert!(
        stdout.contains("Protocol version"),
        "should show protocol version"
    );
    assert!(
        stdout.contains("ACTION TYPES"),
        "should have ACTION TYPES section"
    );
}

#[test]
fn protocol_help_observation_schema_documented() {
    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["protocol-help", "--json"])
        .output()
        .expect("failed to run command");

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let schemas = &json["schemas"];

    // Observation schema should be documented
    assert!(
        schemas["Observation"].is_object(),
        "Observation schema not documented"
    );
}

#[test]
fn protocol_help_wait_conditions_documented() {
    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["protocol-help", "--json"])
        .output()
        .expect("failed to run command");

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let condition_types = &json["schemas"]["Condition"]["types"];

    // Wait conditions should be documented
    assert!(
        condition_types["screen_contains"].is_object(),
        "screen_contains not documented"
    );
    assert!(
        condition_types["screen_matches"].is_object(),
        "screen_matches not documented"
    );
    assert!(
        condition_types["cursor_at"].is_object(),
        "cursor_at not documented"
    );
    assert!(
        condition_types["process_exited"].is_object(),
        "process_exited not documented"
    );
}
