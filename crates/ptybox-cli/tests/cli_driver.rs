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

//! Tests for the `driver` command (NDJSON protocol mode).
//!
//! The driver command is the primary interface for LLM agents to interact with
//! TUI applications via a stable JSON protocol.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use ptybox::model::policy::PolicyBuilder;
use ptybox::model::{DriverResponseStatus, DriverResponseV2, PROTOCOL_VERSION};
use serde_json::json;

static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Helper to spawn the driver process with stdio pipes.
fn spawn_driver(command: &str) -> Child {
    let policy_path = write_driver_policy(command);

    Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "driver",
            "--stdio",
            "--json",
            "--policy",
            policy_path.to_str().expect("policy path should be utf-8"),
            "--",
            command,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn driver")
}

fn temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let sequence = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    dir.push(format!("ptybox-driver-test-{prefix}-{stamp}-{sequence}"));
    fs::create_dir_all(&dir).expect("failed to create temp dir");
    dir
}

fn write_driver_policy(command: &str) -> PathBuf {
    let dir = temp_dir("policy");
    let policy_path = dir.join("policy.json");
    let policy = PolicyBuilder::new()
        .sandbox_disabled()
        .allowed_executables(vec![command.to_string()])
        .build();
    let payload = serde_json::to_vec_pretty(&policy).expect("failed to serialize policy");
    fs::write(&policy_path, payload).expect("failed to write policy");
    policy_path
}

fn write_driver_policy_with_artifacts(command: &str, artifacts_dir: &Path) -> PathBuf {
    let dir = temp_dir("policy-artifacts");
    let policy_path = dir.join("policy.json");
    let policy = PolicyBuilder::new()
        .sandbox_disabled()
        .allowed_executables(vec![command.to_string()])
        .allowed_write(vec![artifacts_dir.display().to_string()])
        .build();
    let payload = serde_json::to_vec_pretty(&policy).expect("failed to serialize policy");
    fs::write(&policy_path, payload).expect("failed to write policy");
    policy_path
}

fn request(request_id: &str, action_type: &str, payload: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "protocol_version": PROTOCOL_VERSION,
        "request_id": request_id,
        "action": {
            "type": action_type,
            "payload": payload
        }
    })
}

/// Send an action to the driver and read the response envelope.
fn send_action(child: &mut Child, action: serde_json::Value) -> DriverResponseV2 {
    let stdin = child.stdin.as_mut().expect("stdin not available");
    let line = serde_json::to_string(&action).expect("failed to serialize action");
    writeln!(stdin, "{}", line).expect("failed to write to stdin");
    stdin.flush().expect("failed to flush stdin");

    let stdout = child.stdout.as_mut().expect("stdout not available");
    let mut reader = BufReader::new(stdout);
    let mut response = String::new();
    reader
        .read_line(&mut response)
        .expect("failed to read response");

    serde_json::from_str(&response).expect("failed to parse driver response")
}

/// Read raw response line from driver stdout.
fn read_response_line(child: &mut Child) -> String {
    let stdout = child.stdout.as_mut().expect("stdout not available");
    let mut reader = BufReader::new(stdout);
    let mut response = String::new();
    reader
        .read_line(&mut response)
        .expect("failed to read response");
    response
}

// =============================================================================
// Basic Requirements Tests
// =============================================================================

#[test]
fn driver_requires_stdio_flag() {
    // Without --stdio, driver should fail
    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["driver", "--json", "--", "/bin/echo", "hello"])
        .output()
        .expect("failed to run command");

    assert!(!output.status.success());
    // With --json, error goes to stdout as JSON
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("driver requires --stdio --json"),
        "expected error about --stdio, got stdout: {}, stderr: {}",
        stdout,
        stderr
    );
}

#[test]
fn driver_requires_json_flag() {
    // Without --json, driver should fail
    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["driver", "--stdio", "--", "/bin/echo", "hello"])
        .output()
        .expect("failed to run command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("driver requires --stdio --json"),
        "expected error about --json, got: {}",
        stderr
    );
}

#[test]
fn driver_requires_command() {
    // Without a command, driver should fail
    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["driver", "--stdio", "--json"])
        .output()
        .expect("failed to run command");

    assert!(!output.status.success());
}

#[test]
fn driver_rejects_non_allowlisted_executable() {
    let policy_path = write_driver_policy("/bin/echo");
    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "driver",
            "--stdio",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            "/bin/cat",
        ])
        .output()
        .expect("failed to run command");

    assert_eq!(output.status.code(), Some(2));
}

// =============================================================================
// Action Type Tests
// =============================================================================

#[test]
fn driver_accepts_text_action() {
    let mut child = spawn_driver("/bin/cat");

    let response = send_action(
        &mut child,
        request("req-text", "text", json!({"text": "hello"})),
    );
    assert_eq!(response.status, DriverResponseStatus::Ok);
    let observation = response.observation.expect("observation should be present");
    assert!(observation.timestamp_ms > 0);
    assert_eq!(
        response
            .action_metrics
            .as_ref()
            .expect("metrics should be present")
            .sequence,
        1
    );

    // Terminate cleanly
    let _ = send_action(&mut child, request("req-term", "terminate", json!({})));
    let _ = child.wait();
}

#[test]
fn driver_accepts_key_action() {
    let mut child = spawn_driver("/bin/cat");

    let response = send_action(
        &mut child,
        request("req-key", "key", json!({"key": "Enter"})),
    );
    assert_eq!(response.status, DriverResponseStatus::Ok);
    let observation = response.observation.expect("observation should be present");
    assert!(observation.timestamp_ms > 0);

    // Terminate cleanly
    let _ = send_action(&mut child, request("req-term", "terminate", json!({})));
    let _ = child.wait();
}

#[test]
fn driver_accepts_resize_action() {
    let mut child = spawn_driver("/bin/cat");

    let response = send_action(
        &mut child,
        request("req-resize", "resize", json!({"rows": 30, "cols": 100})),
    );
    assert_eq!(response.status, DriverResponseStatus::Ok);
    let observation = response.observation.expect("observation should be present");
    assert!(observation.timestamp_ms > 0);
    assert_eq!(observation.screen.rows, 30);
    assert_eq!(observation.screen.cols, 100);

    // Terminate cleanly
    let _ = send_action(&mut child, request("req-term", "terminate", json!({})));
    let _ = child.wait();
}

#[test]
fn driver_terminate_exits_cleanly() {
    let mut child = spawn_driver("/bin/cat");

    let response = send_action(&mut child, request("req-term", "terminate", json!({})));
    assert_eq!(response.status, DriverResponseStatus::Ok);
    assert!(
        response
            .observation
            .expect("observation should be present")
            .protocol_version
            > 0
    );

    let status = child.wait().expect("failed to wait for child");
    assert!(status.success());
}

// =============================================================================
// Protocol Version Tests
// =============================================================================

#[test]
fn driver_rejects_wrong_protocol_version() {
    let policy_path = write_driver_policy("/bin/cat");
    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "driver",
            "--stdio",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            "/bin/cat",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn driver");

    let action = json!({
        "protocol_version": 999,
        "request_id": "req-wrong-version",
        "action": {
            "type": "text",
            "payload": {"text": "hello"}
        }
    });

    let stdin = child.stdin.as_mut().expect("stdin not available");
    let line = serde_json::to_string(&action).expect("failed to serialize");
    writeln!(stdin, "{}", line).expect("failed to write");
    stdin.flush().expect("failed to flush");

    let response = read_response_line(&mut child);
    assert!(
        response.contains("E_PROTOCOL_VERSION_MISMATCH"),
        "expected version mismatch error, got: {}",
        response
    );

    let status = child.wait().expect("failed to wait");
    assert!(!status.success());
    // Exit code 8 is E_PROTOCOL_VERSION
    assert_eq!(status.code(), Some(8));
}

#[test]
fn driver_version_mismatch_includes_context() {
    let policy_path = write_driver_policy("/bin/cat");
    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "driver",
            "--stdio",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            "/bin/cat",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn driver");

    let action = json!({
        "protocol_version": 42,
        "request_id": "req-version-context",
        "action": {
            "type": "text",
            "payload": {"text": "hello"}
        }
    });

    let stdin = child.stdin.as_mut().expect("stdin not available");
    let line = serde_json::to_string(&action).expect("failed to serialize");
    writeln!(stdin, "{}", line).expect("failed to write");
    stdin.flush().expect("failed to flush");

    let response = read_response_line(&mut child);
    let error: serde_json::Value = serde_json::from_str(&response).expect("failed to parse");

    // Should include provided and supported versions
    assert!(
        response.contains("42"),
        "should show provided version 42, got: {}",
        response
    );
    assert!(
        error["error"]["context"]["supported_version"].is_number(),
        "should include supported_version in context"
    );

    let _ = child.wait();
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[test]
fn driver_rejects_malformed_json() {
    let policy_path = write_driver_policy("/bin/cat");
    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "driver",
            "--stdio",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            "/bin/cat",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn driver");

    let stdin = child.stdin.as_mut().expect("stdin not available");
    writeln!(stdin, "{{not valid json}}").expect("failed to write");
    stdin.flush().expect("failed to flush");

    let response = read_response_line(&mut child);
    assert!(
        response.contains("E_PROTOCOL"),
        "expected protocol error, got: {}",
        response
    );

    let status = child.wait().expect("failed to wait");
    assert!(!status.success());
}

#[test]
fn driver_malformed_json_includes_helpful_context() {
    let policy_path = write_driver_policy("/bin/cat");
    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "driver",
            "--stdio",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            "/bin/cat",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn driver");

    let stdin = child.stdin.as_mut().expect("stdin not available");
    writeln!(stdin, "garbage input").expect("failed to write");
    stdin.flush().expect("failed to flush");

    let response = read_response_line(&mut child);
    let error: serde_json::Value = serde_json::from_str(&response).expect("failed to parse");

    // Should include helpful context
    assert!(
        error["error"]["context"]["parse_error"].is_string(),
        "should include parse_error, got: {:?}",
        error
    );
    assert!(
        error["error"]["context"]["hint"]
            .as_str()
            .unwrap_or("")
            .contains("DriverRequestV2"),
        "should include schema hint, got: {:?}",
        error
    );

    let _ = child.wait();
}

#[test]
fn driver_rejects_missing_protocol_version() {
    let policy_path = write_driver_policy("/bin/cat");
    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "driver",
            "--stdio",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            "/bin/cat",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn driver");

    // Missing protocol_version field
    let action = json!({
        "action": {
            "type": "text",
            "payload": {"text": "hello"}
        }
    });

    let stdin = child.stdin.as_mut().expect("stdin not available");
    let line = serde_json::to_string(&action).expect("failed to serialize");
    writeln!(stdin, "{}", line).expect("failed to write");
    stdin.flush().expect("failed to flush");

    let response = read_response_line(&mut child);
    assert!(
        response.contains("E_PROTOCOL"),
        "expected protocol error, got: {}",
        response
    );

    let _ = child.wait();
}

#[test]
fn driver_rejects_missing_action() {
    let policy_path = write_driver_policy("/bin/cat");
    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "driver",
            "--stdio",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            "/bin/cat",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn driver");

    // Missing action field
    let action = json!({
        "protocol_version": PROTOCOL_VERSION
    });

    let stdin = child.stdin.as_mut().expect("stdin not available");
    let line = serde_json::to_string(&action).expect("failed to serialize");
    writeln!(stdin, "{}", line).expect("failed to write");
    stdin.flush().expect("failed to flush");

    let response = read_response_line(&mut child);
    assert!(
        response.contains("E_PROTOCOL"),
        "expected protocol error, got: {}",
        response
    );

    let _ = child.wait();
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn driver_ignores_empty_lines() {
    let mut child = spawn_driver("/bin/cat");

    // Send empty lines (should be ignored), then a real action
    {
        let stdin = child.stdin.as_mut().expect("stdin not available");
        writeln!(stdin).expect("failed to write");
        writeln!(stdin, "   ").expect("failed to write");

        // Now send a real action
        let action = request("req-text", "text", json!({"text": "hello"}));
        let line = serde_json::to_string(&action).expect("failed to serialize");
        writeln!(stdin, "{}", line).expect("failed to write");
        stdin.flush().expect("failed to flush");
    }

    // Should get a valid response (empty lines didn't cause errors)
    let response = read_response_line(&mut child);
    let envelope: DriverResponseV2 =
        serde_json::from_str(&response).expect("failed to parse response");
    assert_eq!(envelope.status, DriverResponseStatus::Ok);
    assert!(
        envelope
            .observation
            .expect("observation should be present")
            .timestamp_ms
            > 0
    );

    // Terminate cleanly
    let _ = send_action(&mut child, request("req-term", "terminate", json!({})));
    let _ = child.wait();
}

#[test]
fn driver_handles_eof_gracefully() {
    let mut child = spawn_driver("/bin/cat");

    // Close stdin without sending terminate
    drop(child.stdin.take());

    // Should exit cleanly
    let status = child.wait().expect("failed to wait");
    assert!(status.success(), "driver should exit cleanly on EOF");
}

#[test]
fn driver_multiple_actions_sequential() {
    let mut child = spawn_driver("/bin/cat");

    // Send multiple text actions
    for i in 0..5 {
        let response = send_action(
            &mut child,
            request(
                &format!("req-{i}"),
                "text",
                json!({"text": format!("msg{}", i)}),
            ),
        );
        assert_eq!(response.status, DriverResponseStatus::Ok);
        assert!(
            response
                .observation
                .expect("observation should be present")
                .timestamp_ms
                > 0
        );
        assert_eq!(
            response
                .action_metrics
                .expect("metrics should be present")
                .sequence,
            i + 1
        );
    }

    // Terminate cleanly
    let _ = send_action(&mut child, request("req-term", "terminate", json!({})));
    let _ = child.wait();
}

#[test]
fn driver_observation_includes_screen_snapshot() {
    let mut child = spawn_driver("/bin/cat");

    let response = send_action(
        &mut child,
        request("req-visible", "text", json!({"text": "visible"})),
    );
    assert_eq!(response.status, DriverResponseStatus::Ok);
    let obs = response.observation.expect("observation should be present");
    assert!(!obs.screen.lines.is_empty(), "screen should have lines");

    // Terminate cleanly
    let _ = send_action(&mut child, request("req-term", "terminate", json!({})));
    let _ = child.wait();
}

#[test]
fn driver_observation_includes_transcript_delta() {
    let mut child = spawn_driver("/bin/cat");

    let response = send_action(
        &mut child,
        request(
            "req-transcript",
            "text",
            json!({"text": "transcript-check"}),
        ),
    );
    assert_eq!(response.status, DriverResponseStatus::Ok);
    let obs = response.observation.expect("observation should be present");

    let has_output = obs.transcript_delta.is_some();
    let has_exit_event = obs
        .events
        .iter()
        .any(|e| e.event_type == "exited" || e.event_type == "process_exit");
    assert!(
        has_output || has_exit_event,
        "expected transcript or exit event"
    );

    let _ = send_action(&mut child, request("req-term", "terminate", json!({})));
    let _ = child.wait();
}

#[test]
fn driver_artifacts_are_replay_compatible() {
    let dir = temp_dir("driver-artifacts");
    let artifacts_dir = dir.join("artifacts");
    let policy_path = write_driver_policy_with_artifacts("/bin/cat", &artifacts_dir);

    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "driver",
            "--stdio",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
            "--overwrite",
            "--",
            "/bin/cat",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn driver");

    let response = send_action(
        &mut child,
        request("req-text", "text", json!({"text": "replay-check"})),
    );
    assert_eq!(response.status, DriverResponseStatus::Ok);
    let _ = send_action(&mut child, request("req-term", "terminate", json!({})));

    let status = child.wait().expect("failed to wait for child");
    assert!(status.success());

    for required in [
        "driver-actions.jsonl",
        "scenario.json",
        "run.json",
        "events.jsonl",
        "transcript.log",
        "checksums.json",
    ] {
        assert!(
            artifacts_dir.join(required).exists(),
            "missing required artifact {required}"
        );
    }

    let replay_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "replay",
            "--json",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run replay");
    assert!(
        replay_output.status.success(),
        "replay failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&replay_output.stdout),
        String::from_utf8_lossy(&replay_output.stderr)
    );
}
