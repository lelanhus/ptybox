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

use tui_use::model::{Action, ActionType, Assertion};

// =============================================================================
// Action Constructor Tests
// =============================================================================

#[test]
fn action_key_creates_correct_payload() {
    let action = Action::key("Enter");
    assert!(matches!(action.action_type, ActionType::Key));
    assert_eq!(
        action.payload.get("key").unwrap().as_str().unwrap(),
        "Enter"
    );
}

#[test]
fn action_text_creates_correct_payload() {
    let action = Action::text("hello world");
    assert!(matches!(action.action_type, ActionType::Text));
    assert_eq!(
        action.payload.get("text").unwrap().as_str().unwrap(),
        "hello world"
    );
}

#[test]
fn action_resize_creates_correct_payload() {
    let action = Action::resize(24, 80);
    assert!(matches!(action.action_type, ActionType::Resize));
    assert_eq!(action.payload.get("rows").unwrap().as_u64().unwrap(), 24);
    assert_eq!(action.payload.get("cols").unwrap().as_u64().unwrap(), 80);
}

#[test]
fn action_wait_for_text_creates_correct_payload() {
    let action = Action::wait_for_text("Ready");
    assert!(matches!(action.action_type, ActionType::Wait));
    let condition = action.payload.get("condition").unwrap();
    assert_eq!(
        condition.get("type").unwrap().as_str().unwrap(),
        "screen_contains"
    );
    assert_eq!(condition.get("text").unwrap().as_str().unwrap(), "Ready");
}

#[test]
fn action_wait_for_regex_creates_correct_payload() {
    let action = Action::wait_for_regex(r"\d+\.\d+");
    assert!(matches!(action.action_type, ActionType::Wait));
    let condition = action.payload.get("condition").unwrap();
    assert_eq!(
        condition.get("type").unwrap().as_str().unwrap(),
        "regex_match"
    );
    assert_eq!(
        condition.get("pattern").unwrap().as_str().unwrap(),
        r"\d+\.\d+"
    );
}

#[test]
fn action_wait_for_cursor_creates_correct_payload() {
    let action = Action::wait_for_cursor(5, 10);
    assert!(matches!(action.action_type, ActionType::Wait));
    let condition = action.payload.get("condition").unwrap();
    assert_eq!(
        condition.get("type").unwrap().as_str().unwrap(),
        "cursor_at"
    );
    assert_eq!(condition.get("row").unwrap().as_u64().unwrap(), 5);
    assert_eq!(condition.get("col").unwrap().as_u64().unwrap(), 10);
}

#[test]
fn action_terminate_creates_correct_type() {
    let action = Action::terminate();
    assert!(matches!(action.action_type, ActionType::Terminate));
}

// =============================================================================
// Assertion Constructor Tests
// =============================================================================

#[test]
fn assertion_screen_contains_creates_correct_payload() {
    let assertion = Assertion::screen_contains("Welcome");
    assert_eq!(assertion.assertion_type, "screen_contains");
    assert_eq!(
        assertion.payload.get("text").unwrap().as_str().unwrap(),
        "Welcome"
    );
}

#[test]
fn assertion_not_contains_creates_correct_payload() {
    let assertion = Assertion::not_contains("Error");
    assert_eq!(assertion.assertion_type, "not_contains");
    assert_eq!(
        assertion.payload.get("text").unwrap().as_str().unwrap(),
        "Error"
    );
}

#[test]
fn assertion_regex_match_creates_correct_payload() {
    let assertion = Assertion::regex_match(r"v\d+\.\d+");
    assert_eq!(assertion.assertion_type, "regex_match");
    assert_eq!(
        assertion.payload.get("pattern").unwrap().as_str().unwrap(),
        r"v\d+\.\d+"
    );
}

#[test]
fn assertion_cursor_at_creates_correct_payload() {
    let assertion = Assertion::cursor_at(5, 10);
    assert_eq!(assertion.assertion_type, "cursor_at");
    assert_eq!(assertion.payload.get("row").unwrap().as_u64().unwrap(), 5);
    assert_eq!(assertion.payload.get("col").unwrap().as_u64().unwrap(), 10);
}

#[test]
fn assertion_line_equals_creates_correct_payload() {
    let assertion = Assertion::line_equals(0, "Hello World");
    assert_eq!(assertion.assertion_type, "line_equals");
    assert_eq!(assertion.payload.get("line").unwrap().as_u64().unwrap(), 0);
    assert_eq!(
        assertion.payload.get("text").unwrap().as_str().unwrap(),
        "Hello World"
    );
}

#[test]
fn assertion_line_contains_creates_correct_payload() {
    let assertion = Assertion::line_contains(3, "foo");
    assert_eq!(assertion.assertion_type, "line_contains");
    assert_eq!(assertion.payload.get("line").unwrap().as_u64().unwrap(), 3);
    assert_eq!(
        assertion.payload.get("text").unwrap().as_str().unwrap(),
        "foo"
    );
}

#[test]
fn assertion_line_matches_creates_correct_payload() {
    let assertion = Assertion::line_matches(2, r"^\d+$");
    assert_eq!(assertion.assertion_type, "line_matches");
    assert_eq!(assertion.payload.get("line").unwrap().as_u64().unwrap(), 2);
    assert_eq!(
        assertion.payload.get("pattern").unwrap().as_str().unwrap(),
        r"^\d+$"
    );
}

#[test]
fn assertion_screen_empty_creates_correct_type() {
    let assertion = Assertion::screen_empty();
    assert_eq!(assertion.assertion_type, "screen_empty");
}

#[test]
fn assertion_cursor_visible_creates_correct_type() {
    let assertion = Assertion::cursor_visible();
    assert_eq!(assertion.assertion_type, "cursor_visible");
}

#[test]
fn assertion_cursor_hidden_creates_correct_type() {
    let assertion = Assertion::cursor_hidden();
    assert_eq!(assertion.assertion_type, "cursor_hidden");
}
