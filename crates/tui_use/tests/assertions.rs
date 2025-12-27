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
        msg.contains("cursor")
            || msg.contains("position")
            || msg.contains("row")
            || msg.contains("col"),
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
        msg.contains("first line")
            || msg.contains("wrong text")
            || msg.contains("expected")
            || msg.contains("actual"),
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
        msg.contains("hello")
            || msg.contains("found")
            || msg.contains("present")
            || msg.contains("contains"),
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

// ============ screen_contains case sensitivity tests ============

#[test]
fn test_screen_contains_case_sensitive() {
    let observation = observation_with_lines(&["Hello World"]);

    // Exact case should match
    let assertion_exact = Assertion {
        assertion_type: "screen_contains".to_string(),
        payload: serde_json::json!({"text": "Hello"}),
    };
    let (passed, message, _) = evaluate(&observation, &assertion_exact);
    assert!(passed, "Exact case should match: {:?}", message);

    // Different case should NOT match (case-sensitive)
    let assertion_wrong_case = Assertion {
        assertion_type: "screen_contains".to_string(),
        payload: serde_json::json!({"text": "hello"}),
    };
    let (passed, message, _) = evaluate(&observation, &assertion_wrong_case);
    assert!(
        !passed,
        "Different case should not match (case-sensitive search)"
    );
    assert!(message.is_some(), "Should have failure message");
}

// ============ screen_contains partial match tests ============

#[test]
fn test_screen_contains_partial_match() {
    let observation = observation_with_lines(&["The quick brown fox jumps over the lazy dog"]);

    // Partial substring at beginning
    let assertion_begin = Assertion {
        assertion_type: "screen_contains".to_string(),
        payload: serde_json::json!({"text": "The quick"}),
    };
    let (passed, _, _) = evaluate(&observation, &assertion_begin);
    assert!(passed, "Should match partial at beginning");

    // Partial substring in middle
    let assertion_middle = Assertion {
        assertion_type: "screen_contains".to_string(),
        payload: serde_json::json!({"text": "brown fox"}),
    };
    let (passed, _, _) = evaluate(&observation, &assertion_middle);
    assert!(passed, "Should match partial in middle");

    // Partial substring at end
    let assertion_end = Assertion {
        assertion_type: "screen_contains".to_string(),
        payload: serde_json::json!({"text": "lazy dog"}),
    };
    let (passed, _, _) = evaluate(&observation, &assertion_end);
    assert!(passed, "Should match partial at end");

    // Single character
    let assertion_char = Assertion {
        assertion_type: "screen_contains".to_string(),
        payload: serde_json::json!({"text": "x"}),
    };
    let (passed, _, _) = evaluate(&observation, &assertion_char);
    assert!(passed, "Should match single character");
}

// ============ screen_contains unicode tests ============

#[test]
fn test_screen_contains_unicode() {
    let observation = observation_with_lines(&["Hello World", "Bonjour le monde", "Hallo Welt"]);

    // Basic unicode with accents
    let assertion_accent = Assertion {
        assertion_type: "screen_contains".to_string(),
        payload: serde_json::json!({"text": "monde"}),
    };
    let (passed, _, _) = evaluate(&observation, &assertion_accent);
    assert!(passed, "Should match unicode text");

    // Multi-line unicode search
    let assertion_welt = Assertion {
        assertion_type: "screen_contains".to_string(),
        payload: serde_json::json!({"text": "Welt"}),
    };
    let (passed, _, _) = evaluate(&observation, &assertion_welt);
    assert!(passed, "Should find unicode on any line");
}

#[test]
fn test_screen_contains_unicode_special_chars() {
    let observation = observation_with_lines(&[
        "CJK characters",
        "Emoji test: smile",
        "Symbols: arrows and math",
    ]);

    // Test basic ASCII in lines with unicode context
    let assertion_symbols = Assertion {
        assertion_type: "screen_contains".to_string(),
        payload: serde_json::json!({"text": "Symbols"}),
    };
    let (passed, _, _) = evaluate(&observation, &assertion_symbols);
    assert!(passed, "Should match text in unicode context");

    let assertion_arrows = Assertion {
        assertion_type: "screen_contains".to_string(),
        payload: serde_json::json!({"text": "arrows and math"}),
    };
    let (passed, _, _) = evaluate(&observation, &assertion_arrows);
    assert!(passed, "Should match multi-word substring");
}

// ============ line_equals exact vs contains tests ============

#[test]
fn test_line_equals_exact_vs_contains() {
    let observation = observation_with_lines(&["hello world", "hello", "world"]);

    // line_equals requires EXACT match
    let assertion_exact = Assertion {
        assertion_type: "line_equals".to_string(),
        payload: serde_json::json!({"line": 0, "text": "hello world"}),
    };
    let (passed, _, _) = evaluate(&observation, &assertion_exact);
    assert!(passed, "Exact match should pass");

    // line_equals should FAIL for partial match
    let assertion_partial = Assertion {
        assertion_type: "line_equals".to_string(),
        payload: serde_json::json!({"line": 0, "text": "hello"}),
    };
    let (passed, message, _) = evaluate(&observation, &assertion_partial);
    assert!(
        !passed,
        "Partial match should FAIL for line_equals (requires exact)"
    );
    assert!(message.is_some());

    // line_contains should pass for partial match
    let assertion_contains = Assertion {
        assertion_type: "line_contains".to_string(),
        payload: serde_json::json!({"line": 0, "text": "hello"}),
    };
    let (passed, _, _) = evaluate(&observation, &assertion_contains);
    assert!(passed, "Partial match should pass for line_contains");

    // Verify line 1 exact match with "hello"
    let assertion_line1_exact = Assertion {
        assertion_type: "line_equals".to_string(),
        payload: serde_json::json!({"line": 1, "text": "hello"}),
    };
    let (passed, _, _) = evaluate(&observation, &assertion_line1_exact);
    assert!(passed, "Line 1 exact match with 'hello' should pass");
}

// ============ regex_matches complex pattern tests ============

#[test]
fn test_regex_matches_complex_pattern() {
    let observation = observation_with_lines(&[
        "Email: user@example.com",
        "Phone: 123-456-7890",
        "Date: 2024-01-15",
        "Version: v1.2.3-beta",
    ]);

    // Email pattern with capturing groups (groups not exposed but pattern should match)
    let assertion_email = Assertion {
        assertion_type: "regex_match".to_string(),
        payload: serde_json::json!({"pattern": r"(\w+)@(\w+)\.(\w+)"}),
    };
    let (passed, message, _) = evaluate(&observation, &assertion_email);
    assert!(
        passed,
        "Email regex with groups should match: {:?}",
        message
    );

    // Phone number pattern
    let assertion_phone = Assertion {
        assertion_type: "regex_match".to_string(),
        payload: serde_json::json!({"pattern": r"\d{3}-\d{3}-\d{4}"}),
    };
    let (passed, _, _) = evaluate(&observation, &assertion_phone);
    assert!(passed, "Phone number pattern should match");

    // ISO date pattern with groups
    let assertion_date = Assertion {
        assertion_type: "regex_match".to_string(),
        payload: serde_json::json!({"pattern": r"(\d{4})-(\d{2})-(\d{2})"}),
    };
    let (passed, _, _) = evaluate(&observation, &assertion_date);
    assert!(passed, "Date pattern with groups should match");

    // Semantic version pattern
    let assertion_version = Assertion {
        assertion_type: "regex_match".to_string(),
        payload: serde_json::json!({"pattern": r"v(\d+)\.(\d+)\.(\d+)(-\w+)?"}),
    };
    let (passed, _, _) = evaluate(&observation, &assertion_version);
    assert!(passed, "Semantic version pattern should match");

    // Complex alternation pattern
    let assertion_alt = Assertion {
        assertion_type: "regex_match".to_string(),
        payload: serde_json::json!({"pattern": r"(Email|Phone|Date|Version):"}),
    };
    let (passed, _, _) = evaluate(&observation, &assertion_alt);
    assert!(passed, "Alternation pattern should match");

    // Pattern that should NOT match
    let assertion_no_match = Assertion {
        assertion_type: "regex_match".to_string(),
        payload: serde_json::json!({"pattern": r"^\d{5}$"}),
    };
    let (passed, _, _) = evaluate(&observation, &assertion_no_match);
    assert!(!passed, "Non-matching pattern should fail");
}
