//! Shared action dispatch and wait-condition logic.
//!
//! These helpers are used by both the interactive [`driver`](crate::driver) and
//! the stateless [`serve`](crate::serve) modules so the action execution
//! semantics stay consistent across entry points.

use crate::model::policy::Policy;
use crate::model::{Action, ActionType, Observation};
use crate::runner::{compile_safe_regex, RunnerError, RunnerResult};
use crate::session::Session;
use crate::util::pause_until;
use serde::Deserialize;
use serde_json::Value;
use std::time::{Duration, Instant};

/// Deserialized wait-action payload (extracted from `Action::payload`).
#[derive(Debug, Deserialize)]
pub(crate) struct WaitPayload {
    /// The wait condition to evaluate.
    pub condition: Condition,
}

/// A single wait condition with a type tag and type-specific payload.
#[derive(Debug, Deserialize)]
pub(crate) struct Condition {
    /// Condition type name (e.g. `"screen_contains"`, `"screen_matches"`).
    #[serde(rename = "type")]
    pub condition_type: String,
    /// Type-specific fields.
    #[serde(default)]
    pub payload: Value,
}

/// Dispatch a single action against `session` and return the resulting observation.
///
/// Wait actions are routed to [`wait_for_condition`]; terminate actions send
/// SIGTERM then observe; all others send the action and observe.
pub(crate) fn perform_action(
    session: &mut Session,
    action: &Action,
    timeout: Duration,
    policy: &Policy,
) -> RunnerResult<Observation> {
    match action.action_type {
        ActionType::Wait => wait_for_condition(session, action, timeout, policy),
        ActionType::Observe => session.observe(timeout),
        ActionType::Terminate => {
            session.terminate()?;
            session.observe(Duration::from_millis(10))
        }
        _ => {
            session.send(action)?;
            session.observe(timeout)
        }
    }
}

/// Poll until `condition` inside a wait action is satisfied or `timeout` elapses.
#[allow(clippy::too_many_lines)]
pub(crate) fn wait_for_condition(
    session: &mut Session,
    action: &Action,
    timeout: Duration,
    policy: &Policy,
) -> RunnerResult<Observation> {
    let wait_payload: WaitPayload =
        serde_json::from_value(action.payload.clone()).map_err(|err| {
            RunnerError::protocol(
                "E_PROTOCOL",
                "invalid wait action payload",
                Some(serde_json::json!({
                    "parse_error": err.to_string(),
                    "received_payload": action.payload,
                })),
            )
        })?;
    let max_wait = Duration::from_millis(policy.budgets.max_wait_ms);
    let wait_timeout = timeout.min(max_wait);
    let deadline = Instant::now() + wait_timeout;

    // Validate required fields before entering the polling loop
    if wait_payload.condition.condition_type == "screen_contains"
        && wait_payload
            .condition
            .payload
            .get("text")
            .and_then(Value::as_str)
            .is_none()
    {
        return Err(RunnerError::protocol(
            "E_PROTOCOL",
            "screen_contains condition requires 'text' field",
            Some(serde_json::json!({ "received_payload": wait_payload.condition.payload })),
        ));
    }

    let compiled_regex = if wait_payload.condition.condition_type == "screen_matches" {
        let pattern = wait_payload
            .condition
            .payload
            .get("pattern")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RunnerError::protocol(
                    "E_PROTOCOL",
                    "screen_matches condition requires 'pattern' field",
                    Some(serde_json::json!({ "received_payload": wait_payload.condition.payload })),
                )
            })?;
        Some(compile_safe_regex(pattern)?)
    } else {
        None
    };

    loop {
        if Instant::now() > deadline {
            return Err(RunnerError::timeout(
                "E_TIMEOUT",
                "wait condition timed out",
                Some(serde_json::json!({
                    "condition": wait_payload.condition.condition_type
                })),
            ));
        }

        let observation = session.observe(Duration::from_millis(50))?;
        if session.wait_for_exit(Duration::from_millis(0))?.is_some() {
            if wait_payload.condition.condition_type == "process_exited" {
                return Ok(observation);
            }
            return Err(RunnerError::process_exit(
                "E_PROCESS_EXIT",
                "process exited during wait",
            ));
        }

        if condition_satisfied(
            &observation,
            &wait_payload.condition,
            compiled_regex.as_ref(),
        )? {
            return Ok(observation);
        }
        pause_until(deadline, Duration::from_millis(10));
    }
}

/// Test whether `observation` satisfies `condition`.
pub(crate) fn condition_satisfied(
    observation: &Observation,
    condition: &Condition,
    compiled_regex: Option<&regex::Regex>,
) -> RunnerResult<bool> {
    match condition.condition_type.as_str() {
        "screen_contains" => {
            // text field is validated before the polling loop
            let text = condition
                .payload
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("");
            Ok(observation.screen.lines.join("\n").contains(text))
        }
        "screen_matches" => {
            // pattern field is validated and compiled before the polling loop
            let screen_text = observation.screen.lines.join("\n");
            if let Some(re) = compiled_regex {
                Ok(re.is_match(&screen_text))
            } else {
                let pattern = condition
                    .payload
                    .get("pattern")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let re = compile_safe_regex(pattern)?;
                Ok(re.is_match(&screen_text))
            }
        }
        "cursor_at" => {
            let row_u64 = condition
                .payload
                .get("row")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    RunnerError::protocol(
                        "E_PROTOCOL",
                        "cursor_at condition requires 'row' field",
                        Some(serde_json::json!({ "received_payload": condition.payload })),
                    )
                })?;
            let col_u64 = condition
                .payload
                .get("col")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    RunnerError::protocol(
                        "E_PROTOCOL",
                        "cursor_at condition requires 'col' field",
                        Some(serde_json::json!({ "received_payload": condition.payload })),
                    )
                })?;
            let row = u16::try_from(row_u64).map_err(|_| {
                RunnerError::protocol(
                    "E_PROTOCOL",
                    format!("row value {row_u64} exceeds maximum u16 value {}", u16::MAX),
                    Some(serde_json::json!({ "received": row_u64, "max": u16::MAX })),
                )
            })?;
            let col = u16::try_from(col_u64).map_err(|_| {
                RunnerError::protocol(
                    "E_PROTOCOL",
                    format!("col value {col_u64} exceeds maximum u16 value {}", u16::MAX),
                    Some(serde_json::json!({ "received": col_u64, "max": u16::MAX })),
                )
            })?;
            Ok(observation.screen.cursor.row == row && observation.screen.cursor.col == col)
        }
        "process_exited" => Ok(false),
        other => Err(RunnerError::protocol(
            "E_PROTOCOL",
            format!("unsupported wait condition '{other}'"),
            Some(serde_json::json!({
                "received": other,
                "supported_conditions": ["screen_contains", "screen_matches", "cursor_at", "process_exited"]
            })),
        )),
    }
}
