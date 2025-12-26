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

use tui_use::assertions::evaluate;
use tui_use::model::scenario::Assertion;
use tui_use::model::PROTOCOL_VERSION;
use tui_use::model::{Cursor, Observation, RunId, ScreenSnapshot, SnapshotId};

fn observation_with_lines(lines: &[&str]) -> Observation {
    Observation {
        protocol_version: PROTOCOL_VERSION,
        run_id: RunId::new(),
        session_id: tui_use::model::SessionId::new(),
        timestamp_ms: 0,
        screen: ScreenSnapshot {
            snapshot_version: 1,
            snapshot_id: SnapshotId::new(),
            rows: lines.len() as u16,
            cols: lines.iter().map(|line| line.len()).max().unwrap_or(0) as u16,
            cursor: Cursor {
                row: 0,
                col: 0,
                visible: true,
            },
            alternate_screen: false,
            lines: lines.iter().map(|s| s.to_string()).collect(),
            cells: None,
        },
        transcript_delta: None,
        events: Vec::new(),
    }
}

#[test]
fn screen_contains_passes_when_text_present() {
    let observation = observation_with_lines(&["hello world"]);
    let assertion = Assertion {
        assertion_type: "screen_contains".to_string(),
        payload: serde_json::json!({"text": "world"}),
    };

    let (passed, message, _) = evaluate(&observation, &assertion);
    assert!(passed);
    assert!(message.is_none());
}

#[test]
fn cursor_at_fails_when_position_mismatch() {
    let mut observation = observation_with_lines(&["hi"]);
    observation.screen.cursor.row = 1;
    observation.screen.cursor.col = 2;

    let assertion = Assertion {
        assertion_type: "cursor_at".to_string(),
        payload: serde_json::json!({"row": 0, "col": 0}),
    };

    let (passed, message, _) = evaluate(&observation, &assertion);
    assert!(!passed);
    let msg = message.expect("Should have failure message");
    // Verify message contains cursor position info
    assert!(
        msg.contains("cursor") || msg.contains("position") || msg.contains("row") || msg.contains("col"),
        "Message should mention cursor/position: {}",
        msg
    );
}

#[test]
fn regex_match_passes_when_pattern_matches() {
    let observation = observation_with_lines(&["hello world"]);
    let assertion = Assertion {
        assertion_type: "regex_match".to_string(),
        payload: serde_json::json!({"pattern": "hello\\s+world"}),
    };

    let (passed, message, _) = evaluate(&observation, &assertion);
    assert!(passed);
    assert!(message.is_none());
}

// ============ line_equals tests ============

#[test]
fn line_equals_passes_when_line_matches_exactly() {
    let observation = observation_with_lines(&["first line", "second line", "third line"]);
    let assertion = Assertion {
        assertion_type: "line_equals".to_string(),
        payload: serde_json::json!({"line": 1, "text": "second line"}),
    };

    let (passed, message, _) = evaluate(&observation, &assertion);
    assert!(passed, "line_equals should pass: {:?}", message);
    assert!(message.is_none());
}

#[test]
fn line_equals_fails_when_line_differs() {
    let observation = observation_with_lines(&["first line", "second line"]);
    let assertion = Assertion {
        assertion_type: "line_equals".to_string(),
        payload: serde_json::json!({"line": 0, "text": "wrong text"}),
    };

    let (passed, message, _) = evaluate(&observation, &assertion);
    assert!(!passed);
    let msg = message.expect("Should have failure message");
    // Message should explain the mismatch (expected vs actual)
    assert!(
        msg.contains("first line") || msg.contains("wrong text") || msg.contains("expected") || msg.contains("actual"),
        "Message should show expected/actual content: {}",
        msg
    );
}

#[test]
fn line_equals_fails_when_line_out_of_bounds() {
    let observation = observation_with_lines(&["only line"]);
    let assertion = Assertion {
        assertion_type: "line_equals".to_string(),
        payload: serde_json::json!({"line": 99, "text": "anything"}),
    };

    let (passed, message, _) = evaluate(&observation, &assertion);
    assert!(!passed);
    assert!(message.is_some());
    assert!(message.unwrap().contains("out of bounds"));
}

// ============ line_contains tests ============

#[test]
fn line_contains_passes_when_substring_present() {
    let observation = observation_with_lines(&["hello world", "goodbye world"]);
    let assertion = Assertion {
        assertion_type: "line_contains".to_string(),
        payload: serde_json::json!({"line": 0, "text": "hello"}),
    };

    let (passed, message, _) = evaluate(&observation, &assertion);
    assert!(passed, "line_contains should pass: {:?}", message);
    assert!(message.is_none());
}

#[test]
fn line_contains_fails_when_substring_absent() {
    let observation = observation_with_lines(&["hello world"]);
    let assertion = Assertion {
        assertion_type: "line_contains".to_string(),
        payload: serde_json::json!({"line": 0, "text": "missing"}),
    };

    let (passed, message, _) = evaluate(&observation, &assertion);
    assert!(!passed);
    let msg = message.expect("Should have failure message");
    // Message should indicate what was being searched for
    assert!(
        msg.contains("missing") || msg.contains("not found") || msg.contains("does not contain"),
        "Message should explain failure: {}",
        msg
    );
}

// ============ line_matches tests ============

#[test]
fn line_matches_passes_when_regex_matches() {
    let observation = observation_with_lines(&["user: admin", "role: superuser"]);
    let assertion = Assertion {
        assertion_type: "line_matches".to_string(),
        payload: serde_json::json!({"line": 0, "pattern": "user:\\s+\\w+"}),
    };

    let (passed, message, _) = evaluate(&observation, &assertion);
    assert!(passed, "line_matches should pass: {:?}", message);
    assert!(message.is_none());
}

#[test]
fn line_matches_fails_with_invalid_regex() {
    let observation = observation_with_lines(&["test"]);
    let assertion = Assertion {
        assertion_type: "line_matches".to_string(),
        payload: serde_json::json!({"line": 0, "pattern": "[invalid"}),
    };

    let (passed, message, _) = evaluate(&observation, &assertion);
    assert!(!passed);
    assert!(message.is_some());
    assert!(message.unwrap().contains("invalid regex"));
}

// ============ not_contains tests ============

#[test]
fn not_contains_passes_when_text_absent() {
    let observation = observation_with_lines(&["hello world"]);
    let assertion = Assertion {
        assertion_type: "not_contains".to_string(),
        payload: serde_json::json!({"text": "goodbye"}),
    };

    let (passed, message, _) = evaluate(&observation, &assertion);
    assert!(passed, "not_contains should pass: {:?}", message);
    assert!(message.is_none());
}

#[test]
fn not_contains_fails_when_text_present() {
    let observation = observation_with_lines(&["hello world"]);
    let assertion = Assertion {
        assertion_type: "not_contains".to_string(),
        payload: serde_json::json!({"text": "hello"}),
    };

    let (passed, message, _) = evaluate(&observation, &assertion);
    assert!(!passed);
    let msg = message.expect("Should have failure message");
    // Message should indicate the unexpected text was found
    assert!(
        msg.contains("hello") || msg.contains("found") || msg.contains("present") || msg.contains("contains"),
        "Message should explain failure: {}",
        msg
    );
}

// ============ screen_empty tests ============

#[test]
fn screen_empty_passes_when_all_whitespace() {
    let observation = observation_with_lines(&["", "   ", "\t"]);
    let assertion = Assertion {
        assertion_type: "screen_empty".to_string(),
        payload: serde_json::json!({}),
    };

    let (passed, message, _) = evaluate(&observation, &assertion);
    assert!(passed, "screen_empty should pass: {:?}", message);
    assert!(message.is_none());
}

#[test]
fn screen_empty_fails_when_content_present() {
    let observation = observation_with_lines(&["", "x", ""]);
    let assertion = Assertion {
        assertion_type: "screen_empty".to_string(),
        payload: serde_json::json!({}),
    };

    let (passed, message, _) = evaluate(&observation, &assertion);
    assert!(!passed);
    let msg = message.expect("Should have failure message");
    // Message should indicate screen is not empty
    assert!(
        msg.contains("empty") || msg.contains("content") || msg.contains("not"),
        "Message should explain failure: {}",
        msg
    );
}

// ============ cursor_visible tests ============

#[test]
fn cursor_visible_passes_when_visible() {
    let mut observation = observation_with_lines(&["test"]);
    observation.screen.cursor.visible = true;
    let assertion = Assertion {
        assertion_type: "cursor_visible".to_string(),
        payload: serde_json::json!({}),
    };

    let (passed, message, _) = evaluate(&observation, &assertion);
    assert!(passed, "cursor_visible should pass: {:?}", message);
    assert!(message.is_none());
}

#[test]
fn cursor_visible_fails_when_hidden() {
    let mut observation = observation_with_lines(&["test"]);
    observation.screen.cursor.visible = false;
    let assertion = Assertion {
        assertion_type: "cursor_visible".to_string(),
        payload: serde_json::json!({}),
    };

    let (passed, message, _) = evaluate(&observation, &assertion);
    assert!(!passed);
    let msg = message.expect("Should have failure message");
    // Message should explain cursor visibility issue
    assert!(
        msg.contains("cursor") || msg.contains("visible") || msg.contains("hidden"),
        "Message should explain failure: {}",
        msg
    );
}

// ============ cursor_hidden tests ============

#[test]
fn cursor_hidden_passes_when_hidden() {
    let mut observation = observation_with_lines(&["test"]);
    observation.screen.cursor.visible = false;
    let assertion = Assertion {
        assertion_type: "cursor_hidden".to_string(),
        payload: serde_json::json!({}),
    };

    let (passed, message, _) = evaluate(&observation, &assertion);
    assert!(passed, "cursor_hidden should pass: {:?}", message);
    assert!(message.is_none());
}

#[test]
fn cursor_hidden_fails_when_visible() {
    let mut observation = observation_with_lines(&["test"]);
    observation.screen.cursor.visible = true;
    let assertion = Assertion {
        assertion_type: "cursor_hidden".to_string(),
        payload: serde_json::json!({}),
    };

    let (passed, message, _) = evaluate(&observation, &assertion);
    assert!(!passed);
    let msg = message.expect("Should have failure message");
    // Message should explain cursor visibility issue
    assert!(
        msg.contains("cursor") || msg.contains("visible") || msg.contains("hidden"),
        "Message should explain failure: {}",
        msg
    );
}
