use crate::model::Observation;
use crate::model::scenario::Assertion;
use serde_json::Value;

pub fn evaluate(
    observation: &Observation,
    assertion: &Assertion,
) -> (bool, Option<String>, Option<Value>) {
    match assertion.assertion_type.as_str() {
        "screen_contains" => {
            let text = assertion
                .payload
                .get("text")
                .and_then(|v| v.as_str())
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
                .and_then(|v| v.as_str())
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
            let row = assertion
                .payload
                .get("row")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u16;
            let col = assertion
                .payload
                .get("col")
                .and_then(|v| v.as_u64())
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
        _ => (false, Some("unsupported assertion".to_string()), None),
    }
}
