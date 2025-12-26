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
    assert!(session.is_ok(), "Failed to spawn /bin/echo: {:?}", session.err());
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
    assert!(session.is_ok(), "Failed to spawn with args: {:?}", session.err());
}

#[test]
fn session_spawn_invalid_command() {
    let config = default_config("/nonexistent/command");
    let result = Session::spawn(config);
    match result {
        Ok(_) => panic!("Should fail to spawn nonexistent command"),
        Err(err) => assert_eq!(err.code, "E_IO"),
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
    assert!(result.is_ok(), "Failed to send Enter key: {:?}", result.err());
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
    assert!(result.is_ok(), "Failed to send single char: {:?}", result.err());
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
    assert_eq!(err.code, "E_PROTOCOL");
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
    assert_eq!(err.code, "E_PROTOCOL");
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
    assert_eq!(err.code, "E_PROTOCOL");
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
    assert_eq!(err.code, "E_PROTOCOL");
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

    // Retry a few times - echo completes quickly and timing can be tight in CI
    let mut found = false;
    for _ in 0..5 {
        std::thread::sleep(Duration::from_millis(50));
        let observation = session.observe(Duration::from_millis(100)).unwrap();
        let screen_text = observation.screen.lines.join("\n");
        if screen_text.contains("test output") {
            found = true;
            break;
        }
    }

    assert!(found, "Expected 'test output' in screen after multiple attempts");
}

#[test]
fn session_observe_returns_observation_structure() {
    let config = default_config("/bin/echo");
    let mut session = Session::spawn(config).expect("Failed to spawn");

    let observation = session.observe(Duration::from_millis(50)).unwrap();

    // Verify observation structure
    assert!(observation.timestamp_ms < 10000, "Timestamp should be reasonable");
    assert!(!observation.screen.lines.is_empty(), "Should have screen lines");
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
    assert!(observation.transcript_delta.is_none(), "No output expected from cat");
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
        session.wait_for_exit(Duration::from_millis(10)).unwrap().is_none(),
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
