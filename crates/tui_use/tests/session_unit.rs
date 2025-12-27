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

//! Session API unit tests
//!
//! Tests the core PTY session management functionality.

use std::time::Duration;
use tui_use::model::{Action, ActionType, RunId, TerminalSize};
use tui_use::runner::ErrorCode;
use tui_use::session::{Session, SessionConfig};

fn default_config(command: &str) -> SessionConfig {
    SessionConfig {
        command: command.to_string(),
        args: vec![],
        cwd: None,
        size: TerminalSize::default(),
        run_id: RunId::new(),
        env: Default::default(),
    }
}

// =============================================================================
// Spawn Tests
// =============================================================================

#[test]
fn session_spawn_success() {
    let config = default_config("/bin/echo");
    let session = Session::spawn(config);
    assert!(
        session.is_ok(),
        "Failed to spawn /bin/echo: {:?}",
        session.err()
    );
}

#[test]
fn session_spawn_with_args() {
    let config = SessionConfig {
        command: "/bin/echo".to_string(),
        args: vec!["hello".to_string(), "world".to_string()],
        cwd: None,
        size: TerminalSize::default(),
        run_id: RunId::new(),
        env: Default::default(),
    };
    let session = Session::spawn(config);
    assert!(
        session.is_ok(),
        "Failed to spawn with args: {:?}",
        session.err()
    );
}

#[test]
fn session_spawn_invalid_command() {
    let config = default_config("/nonexistent/command");
    let result = Session::spawn(config);
    match result {
        Ok(_) => panic!("Should fail to spawn nonexistent command"),
        Err(err) => assert_eq!(err.code, ErrorCode::Io),
    }
}

#[test]
fn session_spawn_with_cwd() {
    let config = SessionConfig {
        command: "/bin/pwd".to_string(),
        args: vec![],
        cwd: Some("/tmp".to_string()),
        size: TerminalSize::default(),
        run_id: RunId::new(),
        env: Default::default(),
    };
    let mut session = Session::spawn(config).expect("Failed to spawn");
    let observation = session.observe(Duration::from_millis(100)).unwrap();
    // The output should contain /tmp (or /private/tmp on macOS)
    let screen_text = observation.screen.lines.join("\n");
    assert!(
        screen_text.contains("tmp"),
        "Expected output to contain 'tmp', got: {}",
        screen_text
    );
}

// =============================================================================
// Send Action Tests
// =============================================================================

#[test]
fn session_send_key_enter() {
    let config = default_config("/bin/cat");
    let mut session = Session::spawn(config).expect("Failed to spawn");

    let action = Action {
        action_type: ActionType::Key,
        payload: serde_json::json!({"key": "Enter"}),
    };
    let result = session.send(&action);
    assert!(
        result.is_ok(),
        "Failed to send Enter key: {:?}",
        result.err()
    );
}

#[test]
fn session_send_key_single_char() {
    let config = default_config("/bin/cat");
    let mut session = Session::spawn(config).expect("Failed to spawn");

    let action = Action {
        action_type: ActionType::Key,
        payload: serde_json::json!({"key": "a"}),
    };
    let result = session.send(&action);
    assert!(
        result.is_ok(),
        "Failed to send single char: {:?}",
        result.err()
    );
}

#[test]
fn session_send_key_missing_payload() {
    let config = default_config("/bin/cat");
    let mut session = Session::spawn(config).expect("Failed to spawn");

    let action = Action {
        action_type: ActionType::Key,
        payload: serde_json::json!({}),
    };
    let result = session.send(&action);
    assert!(result.is_err(), "Should fail without key field");
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::Protocol);
}

#[test]
fn session_send_key_unsupported() {
    let config = default_config("/bin/cat");
    let mut session = Session::spawn(config).expect("Failed to spawn");

    let action = Action {
        action_type: ActionType::Key,
        payload: serde_json::json!({"key": "UnsupportedKey"}),
    };
    let result = session.send(&action);
    assert!(result.is_err(), "Should fail for unsupported key");
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::Protocol);
}

#[test]
fn session_send_text() {
    let config = default_config("/bin/cat");
    let mut session = Session::spawn(config).expect("Failed to spawn");

    let action = Action {
        action_type: ActionType::Text,
        payload: serde_json::json!({"text": "hello world"}),
    };
    let result = session.send(&action);
    assert!(result.is_ok(), "Failed to send text: {:?}", result.err());
}

#[test]
fn session_send_text_missing_payload() {
    let config = default_config("/bin/cat");
    let mut session = Session::spawn(config).expect("Failed to spawn");

    let action = Action {
        action_type: ActionType::Text,
        payload: serde_json::json!({}),
    };
    let result = session.send(&action);
    assert!(result.is_err(), "Should fail without text field");
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::Protocol);
}

#[test]
fn session_send_resize() {
    let config = default_config("/bin/cat");
    let mut session = Session::spawn(config).expect("Failed to spawn");

    let action = Action {
        action_type: ActionType::Resize,
        payload: serde_json::json!({"rows": 40, "cols": 120}),
    };
    let result = session.send(&action);
    assert!(result.is_ok(), "Failed to resize: {:?}", result.err());

    // Verify screen structure reflects the new size using retry loop
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(5);
    let mut verified = false;

    while start.elapsed() < timeout {
        let observation = session.observe(Duration::from_millis(100)).unwrap();
        // After resize, screen should have 40 rows
        if observation.screen.lines.len() == 40 {
            verified = true;
            // Verify cursor is within new bounds
            assert!(
                observation.screen.cursor.row < 40,
                "Cursor row {} should be within new screen bounds (40 rows)",
                observation.screen.cursor.row
            );
            assert!(
                observation.screen.cursor.col < 120,
                "Cursor col {} should be within new screen bounds (120 cols)",
                observation.screen.cursor.col
            );
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    assert!(verified, "Screen should have 40 rows after resize");
}

#[test]
fn session_send_resize_missing_rows() {
    let config = default_config("/bin/cat");
    let mut session = Session::spawn(config).expect("Failed to spawn");

    let action = Action {
        action_type: ActionType::Resize,
        payload: serde_json::json!({"cols": 120}),
    };
    let result = session.send(&action);
    assert!(result.is_err(), "Should fail without rows");
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::Protocol);
}

#[test]
fn session_send_wait_is_noop() {
    let config = default_config("/bin/cat");
    let mut session = Session::spawn(config).expect("Failed to spawn");

    let action = Action {
        action_type: ActionType::Wait,
        payload: serde_json::json!({}),
    };
    let result = session.send(&action);
    assert!(result.is_ok(), "Wait action should succeed as no-op");
}

// =============================================================================
// Observe Tests
// =============================================================================

#[test]
fn session_observe_captures_output() {
    let config = SessionConfig {
        command: "/bin/echo".to_string(),
        args: vec!["test output".to_string()],
        cwd: None,
        size: TerminalSize::default(),
        run_id: RunId::new(),
        env: Default::default(),
    };
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // Retry with progressive backoff - timing can be tight in CI environments under load
    let mut found = false;
    for attempt in 0..10 {
        // Progressive backoff: 50ms, 100ms, 150ms, etc.
        let delay = Duration::from_millis(50 * (attempt + 1));
        std::thread::sleep(delay);
        let observation = session.observe(Duration::from_millis(200)).unwrap();
        let screen_text = observation.screen.lines.join("\n");
        if screen_text.contains("test output") {
            found = true;
            break;
        }
    }

    assert!(
        found,
        "Expected 'test output' in screen after multiple attempts"
    );
}

#[test]
fn session_observe_returns_observation_structure() {
    let config = default_config("/bin/echo");
    let mut session = Session::spawn(config).expect("Failed to spawn");

    let observation = session.observe(Duration::from_millis(50)).unwrap();

    // Verify timestamp is within reasonable bounds (0 to 60 seconds from session start)
    assert!(
        observation.timestamp_ms < 60_000,
        "Timestamp should be less than 60 seconds, got: {} ms",
        observation.timestamp_ms
    );

    // Verify screen structure matches default terminal size (24 rows, 80 cols)
    assert_eq!(
        observation.screen.lines.len(),
        24,
        "Screen should have 24 rows (default terminal height)"
    );

    // Verify each line has reasonable length (should not exceed default cols)
    for (i, line) in observation.screen.lines.iter().enumerate() {
        assert!(
            line.len() <= 80,
            "Line {} should not exceed 80 chars (default terminal width), got: {}",
            i,
            line.len()
        );
    }

    // Verify cursor position is within bounds
    assert!(
        observation.screen.cursor.row < 24,
        "Cursor row {} should be within screen bounds",
        observation.screen.cursor.row
    );
    assert!(
        observation.screen.cursor.col < 80,
        "Cursor col {} should be within screen bounds",
        observation.screen.cursor.col
    );
}

#[test]
fn session_observe_timeout_returns_partial() {
    let config = default_config("/bin/cat");
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // cat without input produces no output, so observe should timeout
    let start = std::time::Instant::now();
    let observation = session.observe(Duration::from_millis(50)).unwrap();
    let elapsed = start.elapsed();

    // Should return within a reasonable time
    assert!(elapsed < Duration::from_millis(200), "Should not hang");
    assert!(
        observation.transcript_delta.is_none(),
        "No output expected from cat"
    );
}

// =============================================================================
// Terminate Tests
// =============================================================================

#[test]
fn session_terminate_graceful() {
    let config = SessionConfig {
        command: "/bin/sleep".to_string(),
        args: vec!["10".to_string()],
        cwd: None,
        size: TerminalSize::default(),
        run_id: RunId::new(),
        env: Default::default(),
    };
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // Should be running
    assert!(
        session
            .wait_for_exit(Duration::from_millis(10))
            .unwrap()
            .is_none(),
        "Process should still be running"
    );

    // Terminate
    let result = session.terminate();
    assert!(result.is_ok(), "Failed to terminate: {:?}", result.err());

    // Should exit after termination
    let status = session.wait_for_exit(Duration::from_millis(500)).unwrap();
    assert!(status.is_some(), "Process should have exited after SIGTERM");
}

#[test]
fn session_terminate_process_group_returns_status() {
    let config = SessionConfig {
        command: "/bin/sleep".to_string(),
        args: vec!["10".to_string()],
        cwd: None,
        size: TerminalSize::default(),
        run_id: RunId::new(),
        env: Default::default(),
    };
    let mut session = Session::spawn(config).expect("Failed to spawn");

    let result = session.terminate_process_group(Duration::from_millis(200));
    assert!(result.is_ok(), "Failed to terminate: {:?}", result.err());

    let status = result.unwrap();
    assert!(status.is_some(), "Should return exit status");
}

// =============================================================================
// Wait for Exit Tests
// =============================================================================

#[test]
fn session_wait_for_exit_immediate() {
    let config = SessionConfig {
        command: "/bin/echo".to_string(),
        args: vec!["quick".to_string()],
        cwd: None,
        size: TerminalSize::default(),
        run_id: RunId::new(),
        env: Default::default(),
    };
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // Wait for process to complete
    let status = session.wait_for_exit(Duration::from_millis(1000)).unwrap();
    assert!(status.is_some(), "Echo should have exited");
    assert!(status.unwrap().success(), "Echo should exit successfully");
}

#[test]
fn session_wait_for_exit_timeout() {
    let config = SessionConfig {
        command: "/bin/sleep".to_string(),
        args: vec!["10".to_string()],
        cwd: None,
        size: TerminalSize::default(),
        run_id: RunId::new(),
        env: Default::default(),
    };
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // Should timeout since sleep is running
    let status = session.wait_for_exit(Duration::from_millis(50)).unwrap();
    assert!(status.is_none(), "Should timeout while sleep is running");
}

// =============================================================================
// Session ID Tests
// =============================================================================

#[test]
fn session_id_is_unique() {
    let config1 = default_config("/bin/echo");
    let config2 = default_config("/bin/echo");

    let session1 = Session::spawn(config1).expect("Failed to spawn");
    let session2 = Session::spawn(config2).expect("Failed to spawn");

    assert_ne!(
        session1.session_id(),
        session2.session_id(),
        "Each session should have a unique ID"
    );
}

// =============================================================================
// Process Termination Edge Cases
// =============================================================================

#[test]
fn session_terminate_process_group_already_dead() {
    let config = SessionConfig {
        command: "/bin/echo".to_string(),
        args: vec!["quick".to_string()],
        cwd: None,
        size: TerminalSize::default(),
        run_id: RunId::new(),
        env: Default::default(),
    };
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // Wait for echo to finish using retry loop instead of hard-coded sleep
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(5);
    while start.elapsed() < timeout {
        if session
            .wait_for_exit(Duration::from_millis(50))
            .unwrap()
            .is_some()
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    // Terminating an already-dead process should not error
    let result = session.terminate_process_group(Duration::from_millis(100));
    assert!(
        result.is_ok(),
        "Terminating dead process should succeed: {:?}",
        result.err()
    );
}

#[test]
fn session_terminate_long_running_with_grace_period() {
    let config = SessionConfig {
        command: "/bin/sleep".to_string(),
        args: vec!["60".to_string()],
        cwd: None,
        size: TerminalSize::default(),
        run_id: RunId::new(),
        env: Default::default(),
    };
    let mut session = Session::spawn(config).expect("Failed to spawn");

    let start = std::time::Instant::now();
    let result = session.terminate_process_group(Duration::from_millis(500));
    let elapsed = start.elapsed();

    assert!(
        result.is_ok(),
        "Termination should succeed: {:?}",
        result.err()
    );
    // Should complete within grace period (not waiting full 60 seconds)
    assert!(
        elapsed < Duration::from_secs(5),
        "Should not wait for sleep to finish naturally"
    );
}

#[test]
fn session_wait_for_exit_zero_timeout_returns_immediately() {
    let config = SessionConfig {
        command: "/bin/sleep".to_string(),
        args: vec!["10".to_string()],
        cwd: None,
        size: TerminalSize::default(),
        run_id: RunId::new(),
        env: Default::default(),
    };
    let mut session = Session::spawn(config).expect("Failed to spawn");

    let start = std::time::Instant::now();
    let status = session.wait_for_exit(Duration::from_millis(0)).unwrap();
    let elapsed = start.elapsed();

    assert!(status.is_none(), "Should return None for zero timeout");
    assert!(
        elapsed < Duration::from_millis(100),
        "Should return immediately"
    );
}

#[test]
fn session_multiple_terminate_calls_safe() {
    let config = SessionConfig {
        command: "/bin/sleep".to_string(),
        args: vec!["10".to_string()],
        cwd: None,
        size: TerminalSize::default(),
        run_id: RunId::new(),
        env: Default::default(),
    };
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // First terminate
    let result1 = session.terminate();
    assert!(result1.is_ok(), "First terminate should succeed");

    // Second terminate (process may already be dead)
    let result2 = session.terminate();
    assert!(result2.is_ok(), "Second terminate should succeed");

    // Wait for process to be fully dead before third call
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(5);
    while start.elapsed() < timeout {
        if session
            .wait_for_exit(Duration::from_millis(50))
            .unwrap()
            .is_some()
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    // Third with process group - may return error if process already exited
    // which is fine, we just want to ensure it doesn't panic
    let _result3 = session.terminate_process_group(Duration::from_millis(100));
    // Don't assert success - the process may already be fully cleaned up
}

#[test]
fn session_observe_after_terminate() {
    let config = SessionConfig {
        command: "/bin/sleep".to_string(),
        args: vec!["10".to_string()],
        cwd: None,
        size: TerminalSize::default(),
        run_id: RunId::new(),
        env: Default::default(),
    };
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // Terminate process
    session.terminate().unwrap();

    // Wait for process to actually terminate using retry loop
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(5);
    while start.elapsed() < timeout {
        if session
            .wait_for_exit(Duration::from_millis(50))
            .unwrap()
            .is_some()
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    // Observe should still work (returns last screen state)
    let result = session.observe(Duration::from_millis(50));
    assert!(
        result.is_ok(),
        "Observe after terminate should work: {:?}",
        result.err()
    );
}

#[test]
fn session_terminate_returns_exit_status_for_quick_exit() {
    let config = SessionConfig {
        command: "/bin/sh".to_string(),
        args: vec!["-c".to_string(), "exit 42".to_string()],
        cwd: None,
        size: TerminalSize::default(),
        run_id: RunId::new(),
        env: Default::default(),
    };
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // Wait for process to exit naturally
    let status = session.wait_for_exit(Duration::from_millis(500)).unwrap();
    assert!(status.is_some(), "Process should have exited");

    // Verify process exited with non-success (exit 42)
    let exit_status = status.unwrap();
    assert!(!exit_status.success(), "Exit code 42 should not be success");
}

// =============================================================================
// Edge Case Tests - Bounds and Error Handling
// =============================================================================

#[test]
fn session_resize_to_zero_rows_fails() {
    let config = default_config("/bin/cat");
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // Resize to 0 rows should fail with protocol error
    let resize_action = Action {
        action_type: ActionType::Resize,
        payload: serde_json::json!({"rows": 0, "cols": 80}),
    };

    let result = session.send(&resize_action);
    assert!(result.is_err(), "Resize to 0 rows should fail");

    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::Protocol, "Should be protocol error");
    assert!(
        err.message.contains("rows must be between"),
        "Error message should mention row bounds: {}",
        err.message
    );
}

#[test]
fn session_resize_to_zero_cols_fails() {
    let config = default_config("/bin/cat");
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // Resize to 0 cols should fail with protocol error
    let resize_action = Action {
        action_type: ActionType::Resize,
        payload: serde_json::json!({"rows": 24, "cols": 0}),
    };

    let result = session.send(&resize_action);
    assert!(result.is_err(), "Resize to 0 cols should fail");

    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::Protocol, "Should be protocol error");
    assert!(
        err.message.contains("cols must be between"),
        "Error message should mention col bounds: {}",
        err.message
    );
}

#[test]
fn session_resize_to_max_bounds_succeeds() {
    let config = default_config("/bin/cat");
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // Resize to maximum allowed values (500x500 per constants)
    let resize_action = Action {
        action_type: ActionType::Resize,
        payload: serde_json::json!({"rows": 500, "cols": 500}),
    };

    let result = session.send(&resize_action);
    assert!(
        result.is_ok(),
        "Resize to max bounds should succeed: {:?}",
        result.err()
    );
}

#[test]
fn session_resize_exceeds_max_bounds_fails() {
    let config = default_config("/bin/cat");
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // Resize to values exceeding maximum (>500)
    let resize_action = Action {
        action_type: ActionType::Resize,
        payload: serde_json::json!({"rows": 501, "cols": 80}),
    };

    let result = session.send(&resize_action);
    assert!(result.is_err(), "Resize exceeding max rows should fail");

    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::Protocol, "Should be protocol error");
}

#[test]
fn session_resize_overflow_u16_fails() {
    let config = default_config("/bin/cat");
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // Resize with value exceeding u16::MAX
    let resize_action = Action {
        action_type: ActionType::Resize,
        payload: serde_json::json!({"rows": 70000, "cols": 80}),
    };

    let result = session.send(&resize_action);
    assert!(result.is_err(), "Resize exceeding u16::MAX should fail");

    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::Protocol, "Should be protocol error");
    assert!(
        err.message.contains("exceeds maximum"),
        "Error message should mention overflow: {}",
        err.message
    );
}

#[test]
fn session_key_action_missing_key_field_fails() {
    let config = default_config("/bin/cat");
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // Key action without 'key' field
    let action = Action {
        action_type: ActionType::Key,
        payload: serde_json::json!({"wrong_field": "Enter"}),
    };

    let result = session.send(&action);
    assert!(result.is_err(), "Key action without key field should fail");

    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::Protocol, "Should be protocol error");
}

#[test]
fn session_text_action_missing_text_field_fails() {
    let config = default_config("/bin/cat");
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // Text action without 'text' field
    let action = Action {
        action_type: ActionType::Text,
        payload: serde_json::json!({"content": "hello"}),
    };

    let result = session.send(&action);
    assert!(
        result.is_err(),
        "Text action without text field should fail"
    );

    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::Protocol, "Should be protocol error");
}

#[test]
fn session_unknown_key_name_fails() {
    let config = default_config("/bin/cat");
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // Unknown key name
    let action = Action {
        action_type: ActionType::Key,
        payload: serde_json::json!({"key": "UnknownKey123"}),
    };

    let result = session.send(&action);
    assert!(result.is_err(), "Unknown key name should fail");

    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::Protocol, "Should be protocol error");
    assert!(
        err.message.contains("unsupported key"),
        "Error message should mention unsupported key: {}",
        err.message
    );
}
