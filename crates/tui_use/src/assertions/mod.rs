use crate::model::scenario::Assertion;
use crate::model::Observation;
use serde_json::Value;

/// Evaluate an assertion against an observation.
///
/// Returns a tuple of (passed, error message, context).
#[must_use]
pub fn evaluate(
    observation: &Observation,
    assertion: &Assertion,
) -> (bool, Option<String>, Option<Value>) {
    match assertion.assertion_type.as_str() {
        "screen_contains" => {
            let text = assertion
                .payload
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("");
            let joined = observation.screen.lines.join("\n");
            let passed = joined.contains(text);
            let message = if passed {
                None
            } else {
                Some(format!("screen did not contain '{text}'"))
            };
            (passed, message, None)
        }
        "regex_match" => {
            let pattern = assertion
                .payload
                .get("pattern")
                .and_then(Value::as_str)
                .unwrap_or("");
            let joined = observation.screen.lines.join("\n");
            let regex = regex::Regex::new(pattern);
            match regex {
                Ok(re) => {
                    let passed = re.is_match(&joined);
                    let message = if passed {
                        None
                    } else {
                        Some(format!("screen did not match '{pattern}'"))
                    };
                    (passed, message, None)
                }
                Err(err) => (
                    false,
                    Some("invalid regex".to_string()),
                    Some(Value::String(err.to_string())),
                ),
            }
        }
        "cursor_at" => {
            // Row/col values are terminal coordinates, always small enough for u16
            #[allow(clippy::cast_possible_truncation)]
            let row = assertion
                .payload
                .get("row")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u16;
            #[allow(clippy::cast_possible_truncation)]
            let col = assertion
                .payload
                .get("col")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u16;
            let cursor = &observation.screen.cursor;
            let passed = cursor.row == row && cursor.col == col;
            let message = if passed {
                None
            } else {
                Some(format!("cursor at ({}, {})", cursor.row, cursor.col))
            };
            (passed, message, None)
        }
        "line_equals" => {
            #[allow(clippy::cast_possible_truncation)]
            let line_num = assertion
                .payload
                .get("line")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            let expected = assertion
                .payload
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("");
            match observation.screen.lines.get(line_num) {
                Some(actual) => {
                    let passed = actual == expected;
                    let message = if passed {
                        None
                    } else {
                        Some(format!("line {line_num} was '{actual}', expected '{expected}'"))
                    };
                    (passed, message, None)
                }
                None => (
                    false,
                    Some(format!(
                        "line {line_num} out of bounds (screen has {} lines)",
                        observation.screen.lines.len()
                    )),
                    None,
                ),
            }
        }
        "line_contains" => {
            #[allow(clippy::cast_possible_truncation)]
            let line_num = assertion
                .payload
                .get("line")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            let text = assertion
                .payload
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("");
            match observation.screen.lines.get(line_num) {
                Some(actual) => {
                    let passed = actual.contains(text);
                    let message = if passed {
                        None
                    } else {
                        Some(format!("line {line_num} did not contain '{text}'"))
                    };
                    (passed, message, None)
                }
                None => (
                    false,
                    Some(format!(
                        "line {line_num} out of bounds (screen has {} lines)",
                        observation.screen.lines.len()
                    )),
                    None,
                ),
            }
        }
        "line_matches" => {
            #[allow(clippy::cast_possible_truncation)]
            let line_num = assertion
                .payload
                .get("line")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            let pattern = assertion
                .payload
                .get("pattern")
                .and_then(Value::as_str)
                .unwrap_or("");
            match observation.screen.lines.get(line_num) {
                Some(actual) => match regex::Regex::new(pattern) {
                    Ok(re) => {
                        let passed = re.is_match(actual);
                        let message = if passed {
                            None
                        } else {
                            Some(format!("line {line_num} did not match '{pattern}'"))
                        };
                        (passed, message, None)
                    }
                    Err(err) => (
                        false,
                        Some("invalid regex".to_string()),
                        Some(Value::String(err.to_string())),
                    ),
                },
                None => (
                    false,
                    Some(format!(
                        "line {line_num} out of bounds (screen has {} lines)",
                        observation.screen.lines.len()
                    )),
                    None,
                ),
            }
        }
        "not_contains" => {
            let text = assertion
                .payload
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("");
            let joined = observation.screen.lines.join("\n");
            let passed = !joined.contains(text);
            let message = if passed {
                None
            } else {
                Some(format!("screen unexpectedly contained '{text}'"))
            };
            (passed, message, None)
        }
        "screen_empty" => {
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
        "cursor_visible" => {
            let passed = observation.screen.cursor.visible;
            let message = if passed {
                None
            } else {
                Some("cursor is not visible".to_string())
            };
            (passed, message, None)
        }
        "cursor_hidden" => {
            let passed = !observation.screen.cursor.visible;
            let message = if passed {
                None
            } else {
                Some("cursor is not hidden".to_string())
            };
            (passed, message, None)
        }
        _ => (false, Some("unsupported assertion".to_string()), None),
    }
}
