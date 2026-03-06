//! Scenario and policy file loading and serialization.
//!
//! Provides functions to load [`Scenario`] and [`Policy`] definitions
//! from JSON or YAML files. The file format is detected by extension
//! (`.yaml`/`.yml` for YAML, everything else treated as JSON).
//!
//! # Key Functions
//!
//! - [`load_scenario_file`] — Load a scenario from a JSON or YAML file
//! - [`load_policy_file`] — Load a policy from a JSON file by path
//! - [`load_policy_ref`] — Resolve a [`PolicyRef`] (inline or file reference)
//! - [`to_json_value`] — Serialize any type to a [`serde_json::Value`]

use crate::model::policy::Policy;
use crate::model::scenario::{PolicyRef, Scenario};
use crate::runner::{RunnerError, RunnerResult};
use serde_json::Value;
use std::fs;
use std::path::Path;

/// Load and parse a scenario from a JSON or YAML file.
///
/// File format is determined by extension: `.yaml` or `.yml` for YAML,
/// anything else is treated as JSON.
///
/// # Errors
/// - `E_IO` if the file cannot be read
/// - `E_PROTOCOL` if the file cannot be parsed
pub fn load_scenario_file(path: &str) -> RunnerResult<Scenario> {
    let data = fs::read_to_string(path)
        .map_err(|err| RunnerError::io("E_IO", "failed to read scenario file", err))?;
    if path.ends_with(".yaml") || path.ends_with(".yml") {
        serde_yml::from_str(&data)
            .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to parse yaml", err))
    } else {
        serde_json::from_str(&data)
            .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to parse json", err))
    }
}

/// Resolve a policy reference to a [`Policy`].
///
/// If the reference is [`PolicyRef::Inline`], returns the policy directly.
/// If it is [`PolicyRef::File`], loads and parses the JSON file.
///
/// # Errors
/// - `E_IO` if the file cannot be read
/// - `E_PROTOCOL` if the JSON cannot be parsed
pub fn load_policy_ref(policy_ref: &PolicyRef) -> RunnerResult<Policy> {
    match policy_ref {
        PolicyRef::Inline(policy) => Ok(policy.as_ref().clone()),
        PolicyRef::File { path } => {
            let data = fs::read_to_string(path)
                .map_err(|err| RunnerError::io("E_IO", "failed to read policy file", err))?;
            serde_json::from_str(&data)
                .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to parse policy", err))
        }
    }
}

/// Load and parse a policy from a JSON file.
///
/// # Errors
/// - `E_IO` if the file cannot be read
/// - `E_PROTOCOL` if the JSON cannot be parsed
pub fn load_policy_file(path: &Path) -> RunnerResult<Policy> {
    let data = fs::read_to_string(path)
        .map_err(|err| RunnerError::io("E_IO", "failed to read policy file", err))?;
    serde_json::from_str(&data)
        .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to parse policy", err))
}

/// Serialize any `Serialize` type to a [`serde_json::Value`].
///
/// # Errors
/// Returns `E_PROTOCOL` if serialization fails.
pub fn to_json_value<T: serde::Serialize>(value: &T) -> RunnerResult<Value> {
    serde_json::to_value(value)
        .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to serialize", err))
}
