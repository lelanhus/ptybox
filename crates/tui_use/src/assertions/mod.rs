//! Assertion engine for verifying terminal screen state.
//!
//! This module provides the [`evaluate`] function for checking assertions
//! against terminal observations. Assertions verify that the screen content,
//! cursor position, and other terminal state match expected conditions.
//!
//! # Supported Assertion Types
//!
//! | Type | Description | Payload Fields |
//! |------|-------------|----------------|
//! | `screen_contains` | Screen contains substring | `text` |
//! | `regex_match` | Screen matches regex pattern | `pattern` |
//! | `cursor_at` | Cursor at specific position | `row`, `col` |
//! | `line_equals` | Specific line equals text | `line`, `text` |
//! | `line_contains` | Specific line contains text | `line`, `text` |
//! | `line_matches` | Specific line matches regex | `line`, `pattern` |
//! | `not_contains` | Screen does not contain text | `text` |
//! | `screen_empty` | All screen lines are whitespace | (none) |
//! | `cursor_visible` | Cursor is visible | (none) |
//! | `cursor_hidden` | Cursor is hidden | (none) |
//!
//! # Example
//!
//! ```
//! use tui_use::assertions::evaluate;
//! use tui_use::model::{Observation, ScreenSnapshot, Cursor, SnapshotId, RunId, SessionId};
//! use tui_use::model::scenario::Assertion;
//! use serde_json::json;
//!
//! // Create a test observation
//! let observation = Observation {
//!     protocol_version: 1,
//!     run_id: RunId::new(),
//!     session_id: SessionId::new(),
//!     timestamp_ms: 0,
//!     screen: ScreenSnapshot {
//!         snapshot_version: 1,
//!         snapshot_id: SnapshotId::new(),
//!         rows: 24,
//!         cols: 80,
//!         cursor: Cursor { row: 0, col: 5, visible: true },
//!         alternate_screen: false,
//!         lines: vec!["Hello World".to_string()],
//!         cells: None,
//!     },
//!     transcript_delta: None,
//!     events: vec![],
//! };
//!
//! // Check that screen contains expected text
//! let assertion = Assertion {
//!     assertion_type: "screen_contains".to_string(),
//!     payload: json!({"text": "Hello"}),
//! };
//! let (passed, message, _context) = evaluate(&observation, &assertion);
//! assert!(passed);
//! assert!(message.is_none());
//! ```
//!
//! # Security
//!
//! Regex patterns are limited to [`MAX_REGEX_PATTERN_LEN`]
//! characters to prevent `ReDoS` attacks.

use crate::model::scenario::Assertion;
use crate::model::{Observation, ScreenSnapshot, MAX_REGEX_PATTERN_LEN};
use serde_json::Value;

// =============================================================================
// Types
// =============================================================================

/// Result type for assertion evaluation: (passed, `error_message`, context).
type AssertionResult = (bool, Option<String>, Option<Value>);

// =============================================================================
// Main Entry Point
// =============================================================================

/// Evaluate an assertion against an observation.
///
/// Returns a tuple of (passed, error message, context).
#[must_use]
pub fn evaluate(
    observation: &Observation,
    assertion: &Assertion,
) -> (bool, Option<String>, Option<Value>) {
    let screen_text = observation.screen.lines.join("\n");

    match assertion.assertion_type.as_str() {
        "screen_contains" => eval_screen_contains(&screen_text, assertion),
        "regex_match" => eval_regex_match(&screen_text, assertion),
        "cursor_at" => eval_cursor_at(observation, assertion),
        "line_equals" => eval_line_equals(&observation.screen, assertion),
        "line_contains" => eval_line_contains(&observation.screen, assertion),
        "line_matches" => eval_line_matches(&observation.screen, assertion),
        "not_contains" => eval_not_contains(&screen_text, assertion),
        "screen_empty" => eval_screen_empty(observation),
        "cursor_visible" => eval_cursor_visible(observation),
        "cursor_hidden" => eval_cursor_hidden(observation),
        _ => (false, Some("unsupported assertion".to_string()), None),
    }
}

// =============================================================================
// Assertion Handlers
// =============================================================================

fn eval_screen_contains(screen_text: &str, assertion: &Assertion) -> AssertionResult {
    let text = get_text_field(assertion);
    let passed = screen_text.contains(text);
    let message = if passed {
        None
    } else {
        Some(format!("screen did not contain '{text}'"))
    };
    (passed, message, None)
}

fn eval_regex_match(screen_text: &str, assertion: &Assertion) -> AssertionResult {
    let pattern = get_pattern_field(assertion);

    if let Some(err) = validate_pattern_length(pattern) {
        return err;
    }

    match regex::Regex::new(pattern) {
        Ok(re) => {
            let passed = re.is_match(screen_text);
            let message = if passed {
                None
            } else {
                Some(format!("screen did not match '{pattern}'"))
            };
            (passed, message, None)
        }
        Err(err) => regex_error(err),
    }
}

fn eval_cursor_at(observation: &Observation, assertion: &Assertion) -> AssertionResult {
    let row_u64 = assertion
        .payload
        .get("row")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let col_u64 = assertion
        .payload
        .get("col")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    let Ok(row) = u16::try_from(row_u64) else {
        return u16_overflow_error("row", row_u64);
    };
    let Ok(col) = u16::try_from(col_u64) else {
        return u16_overflow_error("col", col_u64);
    };

    let cursor = &observation.screen.cursor;
    let passed = cursor.row == row && cursor.col == col;
    let message = if passed {
        None
    } else {
        Some(format!("cursor at ({}, {})", cursor.row, cursor.col))
    };
    (passed, message, None)
}

fn eval_line_equals(screen: &ScreenSnapshot, assertion: &Assertion) -> AssertionResult {
    let line_u64 = get_line_field(assertion);
    let expected = get_text_field(assertion);

    match get_screen_line(screen, line_u64) {
        Ok(actual) => {
            let passed = actual == expected;
            let message = if passed {
                None
            } else {
                Some(format!(
                    "line {line_u64} was '{actual}', expected '{expected}'"
                ))
            };
            (passed, message, None)
        }
        Err(result) => result,
    }
}

fn eval_line_contains(screen: &ScreenSnapshot, assertion: &Assertion) -> AssertionResult {
    let line_u64 = get_line_field(assertion);
    let text = get_text_field(assertion);

    match get_screen_line(screen, line_u64) {
        Ok(actual) => {
            let passed = actual.contains(text);
            let message = if passed {
                None
            } else {
                Some(format!("line {line_u64} did not contain '{text}'"))
            };
            (passed, message, None)
        }
        Err(result) => result,
    }
}

fn eval_line_matches(screen: &ScreenSnapshot, assertion: &Assertion) -> AssertionResult {
    let line_u64 = get_line_field(assertion);
    let pattern = get_pattern_field(assertion);

    if let Some(err) = validate_pattern_length(pattern) {
        return err;
    }

    match get_screen_line(screen, line_u64) {
        Ok(actual) => match regex::Regex::new(pattern) {
            Ok(re) => {
                let passed = re.is_match(actual);
                let message = if passed {
                    None
                } else {
                    Some(format!("line {line_u64} did not match '{pattern}'"))
                };
                (passed, message, None)
            }
            Err(err) => regex_error(err),
        },
        Err(result) => result,
    }
}

fn eval_not_contains(screen_text: &str, assertion: &Assertion) -> AssertionResult {
    let text = get_text_field(assertion);
    let passed = !screen_text.contains(text);
    let message = if passed {
        None
    } else {
        Some(format!("screen unexpectedly contained '{text}'"))
    };
    (passed, message, None)
}

fn eval_screen_empty(observation: &Observation) -> AssertionResult {
    let passed = observation
        .screen
        .lines
        .iter()
        .all(|line| line.trim().is_empty());
    let message = if passed {
        None
    } else {
        Some("screen is not empty".to_string())
    };
    (passed, message, None)
}

fn eval_cursor_visible(observation: &Observation) -> AssertionResult {
    let passed = observation.screen.cursor.visible;
    let message = if passed {
        None
    } else {
        Some("cursor is not visible".to_string())
    };
    (passed, message, None)
}

fn eval_cursor_hidden(observation: &Observation) -> AssertionResult {
    let passed = !observation.screen.cursor.visible;
    let message = if passed {
        None
    } else {
        Some("cursor is not hidden".to_string())
    };
    (passed, message, None)
}

// =============================================================================
// Field Extractors
// =============================================================================

fn get_text_field(assertion: &Assertion) -> &str {
    assertion
        .payload
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or("")
}

fn get_pattern_field(assertion: &Assertion) -> &str {
    assertion
        .payload
        .get("pattern")
        .and_then(Value::as_str)
        .unwrap_or("")
}

fn get_line_field(assertion: &Assertion) -> u64 {
    assertion
        .payload
        .get("line")
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

/// Get a screen line with bounds checking.
fn get_screen_line(screen: &ScreenSnapshot, line_u64: u64) -> Result<&str, AssertionResult> {
    let line_num = usize::try_from(line_u64).map_err(|_| {
        (
            false,
            Some(format!(
                "line value {line_u64} exceeds maximum usize value {}",
                usize::MAX
            )),
            None,
        )
    })?;

    screen.lines.get(line_num).map(String::as_str).ok_or((
        false,
        Some(format!(
            "line {line_num} out of bounds (screen has {} lines)",
            screen.lines.len()
        )),
        None,
    ))
}

// =============================================================================
// Error Helpers
// =============================================================================

fn validate_pattern_length(pattern: &str) -> Option<AssertionResult> {
    if pattern.len() > MAX_REGEX_PATTERN_LEN {
        Some((
            false,
            Some(format!(
                "regex pattern exceeds maximum length of {MAX_REGEX_PATTERN_LEN} characters"
            )),
            Some(serde_json::json!({
                "pattern_length": pattern.len(),
                "max_length": MAX_REGEX_PATTERN_LEN
            })),
        ))
    } else {
        None
    }
}

fn regex_error(err: regex::Error) -> AssertionResult {
    (
        false,
        Some("invalid regex".to_string()),
        Some(Value::String(err.to_string())),
    )
}

fn u16_overflow_error(field: &str, value: u64) -> AssertionResult {
    (
        false,
        Some(format!(
            "{field} value {value} exceeds maximum u16 value {}",
            u16::MAX
        )),
        None,
    )
}
