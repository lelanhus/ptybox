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

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};

use ptybox::model::{Observation, PROTOCOL_VERSION};
use serde_json::json;

/// Helper to spawn the driver process with stdio pipes.
fn spawn_driver(command: &str) -> Child {
    Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["driver", "--stdio", "--json", "--", command])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn driver")
}

/// Send an action to the driver and read the observation response.
fn send_action(child: &mut Child, action: serde_json::Value) -> Observation {
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

    serde_json::from_str(&response).expect("failed to parse observation")
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

// =============================================================================
// Action Type Tests
// =============================================================================

#[test]
fn driver_accepts_text_action() {
    let mut child = spawn_driver("/bin/cat");

    let action = json!({
        "protocol_version": PROTOCOL_VERSION,
        "action": {
            "type": "text",
            "payload": {"text": "hello"}
        }
    });

    let obs = send_action(&mut child, action);
    assert!(obs.timestamp_ms > 0);

    // Terminate cleanly
    let terminate = json!({
        "protocol_version": PROTOCOL_VERSION,
        "action": {
            "type": "terminate",
            "payload": {}
        }
    });
    let _ = send_action(&mut child, terminate);
    let _ = child.wait();
}

#[test]
fn driver_accepts_key_action() {
    let mut child = spawn_driver("/bin/cat");

    let action = json!({
        "protocol_version": PROTOCOL_VERSION,
        "action": {
            "type": "key",
            "payload": {"key": "Enter"}
        }
    });

    let obs = send_action(&mut child, action);
    assert!(obs.timestamp_ms > 0);

    // Terminate cleanly
    let terminate = json!({
        "protocol_version": PROTOCOL_VERSION,
        "action": {
            "type": "terminate",
            "payload": {}
        }
    });
    let _ = send_action(&mut child, terminate);
    let _ = child.wait();
}

#[test]
fn driver_accepts_resize_action() {
    let mut child = spawn_driver("/bin/cat");

    let action = json!({
        "protocol_version": PROTOCOL_VERSION,
        "action": {
            "type": "resize",
            "payload": {"rows": 30, "cols": 100}
        }
    });

    let obs = send_action(&mut child, action);
    assert!(obs.timestamp_ms > 0);
    assert_eq!(obs.screen.rows, 30);
    assert_eq!(obs.screen.cols, 100);

    // Terminate cleanly
    let terminate = json!({
        "protocol_version": PROTOCOL_VERSION,
        "action": {
            "type": "terminate",
            "payload": {}
        }
    });
    let _ = send_action(&mut child, terminate);
    let _ = child.wait();
}

#[test]
fn driver_terminate_exits_cleanly() {
    let mut child = spawn_driver("/bin/cat");

    let terminate = json!({
        "protocol_version": PROTOCOL_VERSION,
        "action": {
            "type": "terminate",
            "payload": {}
        }
    });

    let obs = send_action(&mut child, terminate);
    // Observation is returned (timestamp_ms may be 0 if immediate)
    assert!(obs.protocol_version > 0);

    let status = child.wait().expect("failed to wait for child");
    assert!(status.success());
}

// =============================================================================
// Protocol Version Tests
// =============================================================================

#[test]
fn driver_rejects_wrong_protocol_version() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["driver", "--stdio", "--json", "--", "/bin/cat"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn driver");

    let action = json!({
        "protocol_version": 999,
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
    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["driver", "--stdio", "--json", "--", "/bin/cat"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn driver");

    let action = json!({
        "protocol_version": 42,
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
        error["context"]["supported_version"].is_number(),
        "should include supported_version in context"
    );

    let _ = child.wait();
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[test]
fn driver_rejects_malformed_json() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["driver", "--stdio", "--json", "--", "/bin/cat"])
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
    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["driver", "--stdio", "--json", "--", "/bin/cat"])
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
        error["context"]["parse_error"].is_string(),
        "should include parse_error, got: {:?}",
        error
    );
    assert!(
        error["context"]["expected_schema"].is_object(),
        "should include expected_schema, got: {:?}",
        error
    );
    assert!(
        error["context"]["example"].is_object(),
        "should include example, got: {:?}",
        error
    );
    assert!(
        error["context"]["hint"]
            .as_str()
            .unwrap_or("")
            .contains("protocol-help"),
        "should suggest protocol-help command, got: {:?}",
        error
    );

    let _ = child.wait();
}

#[test]
fn driver_rejects_missing_protocol_version() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["driver", "--stdio", "--json", "--", "/bin/cat"])
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
    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["driver", "--stdio", "--json", "--", "/bin/cat"])
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
        let action = json!({
            "protocol_version": PROTOCOL_VERSION,
            "action": {
                "type": "text",
                "payload": {"text": "hello"}
            }
        });
        let line = serde_json::to_string(&action).expect("failed to serialize");
        writeln!(stdin, "{}", line).expect("failed to write");
        stdin.flush().expect("failed to flush");
    }

    // Should get a valid response (empty lines didn't cause errors)
    let response = read_response_line(&mut child);
    let obs: Observation = serde_json::from_str(&response).expect("failed to parse observation");
    assert!(obs.timestamp_ms > 0);

    // Terminate cleanly
    let terminate = json!({
        "protocol_version": PROTOCOL_VERSION,
        "action": {
            "type": "terminate",
            "payload": {}
        }
    });
    let _ = send_action(&mut child, terminate);
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
        let action = json!({
            "protocol_version": PROTOCOL_VERSION,
            "action": {
                "type": "text",
                "payload": {"text": format!("msg{}", i)}
            }
        });
        let obs = send_action(&mut child, action);
        assert!(obs.timestamp_ms > 0);
    }

    // Terminate cleanly
    let terminate = json!({
        "protocol_version": PROTOCOL_VERSION,
        "action": {
            "type": "terminate",
            "payload": {}
        }
    });
    let _ = send_action(&mut child, terminate);
    let _ = child.wait();
}

#[test]
fn driver_observation_includes_screen_snapshot() {
    let mut child = spawn_driver("/bin/cat");

    let action = json!({
        "protocol_version": PROTOCOL_VERSION,
        "action": {
            "type": "text",
            "payload": {"text": "visible"}
        }
    });

    let obs = send_action(&mut child, action);
    assert!(!obs.screen.lines.is_empty(), "screen should have lines");

    // Terminate cleanly
    let terminate = json!({
        "protocol_version": PROTOCOL_VERSION,
        "action": {
            "type": "terminate",
            "payload": {}
        }
    });
    let _ = send_action(&mut child, terminate);
    let _ = child.wait();
}

#[test]
fn driver_observation_includes_transcript_delta() {
    let mut child = spawn_driver("/bin/echo");

    // echo outputs text then exits, so we should see transcript delta
    let action = json!({
        "protocol_version": PROTOCOL_VERSION,
        "action": {
            "type": "text",
            "payload": {"text": "ignored"}
        }
    });

    let obs = send_action(&mut child, action);
    // Echo should have produced some output or exited (shown in events)
    let has_output = obs.transcript_delta.is_some();
    let has_exit_event = obs
        .events
        .iter()
        .any(|e| e.event_type == "exited" || e.event_type == "process_exit");
    assert!(
        has_output || has_exit_event,
        "expected transcript or exit event"
    );

    let _ = child.wait();
}
