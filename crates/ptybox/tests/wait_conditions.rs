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

//! Wait condition tests
//!
//! Tests for the wait condition evaluation in the runner module.

use std::time::{Duration, Instant};
use ptybox::model::{Action, ActionType, RunId, TerminalSize};
use ptybox::session::{Session, SessionConfig};

fn default_config(command: &str, args: Vec<String>) -> SessionConfig {
    SessionConfig {
        command: command.to_string(),
        args,
        cwd: None,
        size: TerminalSize::default(),
        run_id: RunId::new(),
        env: Default::default(),
    }
}

// =============================================================================
// Wait Condition Edge Cases
// =============================================================================

#[test]
fn wait_condition_screen_contains_with_immediate_output() {
    // Test that screen_contains wait condition can succeed when output is immediate
    let config = default_config("/bin/echo", vec!["hello world".to_string()]);
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // Retry loop to handle timing variations - output may take a moment to appear
    // Use more retries and longer delays for CI environments under load
    let mut found = false;
    for attempt in 0..10 {
        // Progressive backoff: start short, increase delays
        let delay = Duration::from_millis(50 * (attempt + 1));
        std::thread::sleep(delay);
        let observation = session.observe(Duration::from_millis(300)).unwrap();
        let screen_text = observation.screen.lines.join("\n");
        if screen_text.contains("hello") {
            found = true;
            break;
        }
    }

    assert!(found, "Screen should eventually contain 'hello'");
}

#[test]
fn wait_condition_process_exits_during_wait() {
    // Test that a quick-exiting process is handled correctly
    let config = default_config("/bin/echo", vec!["quick".to_string()]);
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // Use a longer timeout and retry pattern for CI environments under load
    let mut status = None;
    for _ in 0..5 {
        std::thread::sleep(Duration::from_millis(100));
        status = session.wait_for_exit(Duration::from_millis(500)).unwrap();
        if status.is_some() {
            break;
        }
    }

    assert!(status.is_some(), "Process should have exited");
}

#[test]
fn wait_condition_cursor_at_verifies_position() {
    // Test that cursor position can be verified
    let config = default_config("/bin/cat", vec![]);
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // Get initial observation
    let observation = session.observe(Duration::from_millis(100)).unwrap();

    // Cursor should be at a valid position
    let cursor = &observation.screen.cursor;
    assert!(cursor.row < 1000, "Cursor row should be reasonable");
    assert!(cursor.col < 1000, "Cursor col should be reasonable");
}

#[test]
fn wait_condition_timeout_returns_last_observation() {
    // Test that timing out still returns the last known screen state
    let config = default_config("/bin/cat", vec![]);
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // Send some text first
    let action = Action {
        action_type: ActionType::Text,
        payload: serde_json::json!({"text": "test input"}),
    };
    session.send(&action).expect("Failed to send text");

    // Observe with short timeout
    let observation = session.observe(Duration::from_millis(100)).unwrap();

    // Should have valid observation even if wait times out
    assert!(
        !observation.screen.lines.is_empty(),
        "Should have screen lines"
    );
}

#[test]
fn wait_condition_handles_large_output() {
    // Test wait condition with process that produces lots of output
    let config = default_config(
        "/bin/sh",
        vec![
            "-c".to_string(),
            "for i in $(seq 1 10); do echo line$i; done".to_string(),
        ],
    );
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // Wait for output using retry loop instead of hard-coded sleep
    let start = Instant::now();
    let timeout = Duration::from_secs(5);
    let mut found = false;
    let mut screen_text = String::new();

    while start.elapsed() < timeout {
        let observation = session.observe(Duration::from_millis(200)).unwrap();
        screen_text = observation.screen.lines.join("\n");
        if screen_text.contains("line") {
            found = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    // Should contain at least some of the output
    assert!(found, "Screen should contain output lines: {}", screen_text);
}

#[test]
fn wait_condition_resize_during_wait() {
    // Test that resize can happen while waiting
    let config = default_config("/bin/cat", vec![]);
    let mut session = Session::spawn(config).expect("Failed to spawn");

    // Send resize action
    let resize_action = Action {
        action_type: ActionType::Resize,
        payload: serde_json::json!({"rows": 40, "cols": 100}),
    };
    let result = session.send(&resize_action);
    assert!(result.is_ok(), "Resize during wait should succeed");

    // Observe should still work
    let observation = session.observe(Duration::from_millis(100)).unwrap();
    assert!(!observation.screen.lines.is_empty());
}
