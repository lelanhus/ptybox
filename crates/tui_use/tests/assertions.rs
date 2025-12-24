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
    assert!(message.is_some());
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
