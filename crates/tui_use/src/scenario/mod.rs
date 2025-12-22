use crate::model::policy::Policy;
use crate::model::scenario::{PolicyRef, Scenario};
use crate::runner::{RunnerError, RunnerResult};
use serde_json::Value;
use std::fs;
use std::path::Path;

pub fn load_scenario_file(path: &str) -> RunnerResult<Scenario> {
    let data = fs::read_to_string(path)
        .map_err(|err| RunnerError::io("E_IO", "failed to read scenario file", err))?;
    if path.ends_with(".yaml") || path.ends_with(".yml") {
        serde_yaml::from_str(&data)
            .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to parse yaml", err))
    } else {
        serde_json::from_str(&data)
            .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to parse json", err))
    }
}

pub fn load_policy_ref(policy_ref: &PolicyRef) -> RunnerResult<Policy> {
    match policy_ref {
        PolicyRef::Inline(policy) => Ok(policy.clone()),
        PolicyRef::File { path } => {
            let data = fs::read_to_string(path)
                .map_err(|err| RunnerError::io("E_IO", "failed to read policy file", err))?;
            serde_json::from_str(&data)
                .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to parse policy", err))
        }
    }
}

pub fn load_policy_file(path: &Path) -> RunnerResult<Policy> {
    let data = fs::read_to_string(path)
        .map_err(|err| RunnerError::io("E_IO", "failed to read policy file", err))?;
    serde_json::from_str(&data)
        .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to parse policy", err))
}

pub fn to_json_value<T: serde::Serialize>(value: &T) -> RunnerResult<Value> {
    serde_json::to_value(value)
        .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to serialize", err))
}
