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

use std::fs;
use std::path::PathBuf;

fn schema_path(name: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("../../spec/schemas").join(name)
}

fn example_path(name: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("../../spec/examples").join(name)
}

#[test]
fn policy_schema_is_valid_json() {
    let data = fs::read_to_string(schema_path("policy.schema.json")).unwrap();
    let _: serde_json::Value = serde_json::from_str(&data).unwrap();
}

#[test]
fn normalization_schema_is_valid_json() {
    let data = fs::read_to_string(schema_path("normalization.schema.json")).unwrap();
    let _: serde_json::Value = serde_json::from_str(&data).unwrap();
}

#[test]
fn replay_schema_is_valid_json() {
    let data = fs::read_to_string(schema_path("replay.schema.json")).unwrap();
    let _: serde_json::Value = serde_json::from_str(&data).unwrap();
}

#[test]
fn policy_example_conforms_to_schema_subset() {
    let schema: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(schema_path("policy.schema.json")).unwrap())
            .unwrap();
    let example: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(example_path("policy.json")).unwrap()).unwrap();
    validate_required(&schema, &example);
    validate_enums(&schema, &example);
}

#[test]
fn normalization_example_conforms_to_schema_subset() {
    let schema: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(schema_path("normalization.schema.json")).unwrap(),
    )
    .unwrap();
    let example: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(example_path("normalization.json")).unwrap())
            .unwrap();
    validate_required(&schema, &example);
    validate_enums(&schema, &example);
}

#[test]
fn replay_example_conforms_to_schema_subset() {
    let schema: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(schema_path("replay.schema.json")).unwrap())
            .unwrap();
    let example: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(example_path("replay.json")).unwrap()).unwrap();
    validate_required(&schema, &example);
    validate_enums(&schema, &example);
}

fn validate_required(schema: &serde_json::Value, instance: &serde_json::Value) {
    let Some(required) = schema.get("required").and_then(|val| val.as_array()) else {
        return;
    };
    for key in required.iter().filter_map(|val| val.as_str()) {
        assert!(instance.get(key).is_some(), "missing required field {key}");
    }
    if let (Some(props), Some(obj)) = (schema.get("properties"), instance.as_object()) {
        if let Some(props) = props.as_object() {
            for (name, prop_schema) in props {
                if let Some(value) = obj.get(name) {
                    validate_required(prop_schema, value);
                }
            }
        }
    }
}

fn validate_enums(schema: &serde_json::Value, instance: &serde_json::Value) {
    if let Some(enums) = schema.get("enum").and_then(|val| val.as_array()) {
        assert!(
            enums.iter().any(|val| val == instance),
            "enum mismatch: {instance:?}"
        );
    }
    if let (Some(props), Some(obj)) = (schema.get("properties"), instance.as_object()) {
        if let Some(props) = props.as_object() {
            for (name, prop_schema) in props {
                if let Some(value) = obj.get(name) {
                    validate_enums(prop_schema, value);
                }
            }
        }
    }
    if let (Some(items), Some(array)) = (schema.get("items"), instance.as_array()) {
        for item in array {
            validate_enums(items, item);
        }
    }
}

// Tests for new Tier 1 schemas

#[test]
fn scenario_schema_is_valid_json() {
    let data = fs::read_to_string(schema_path("scenario.schema.json")).unwrap();
    let _: serde_json::Value = serde_json::from_str(&data).unwrap();
}

#[test]
fn run_result_schema_is_valid_json() {
    let data = fs::read_to_string(schema_path("run-result.schema.json")).unwrap();
    let _: serde_json::Value = serde_json::from_str(&data).unwrap();
}

#[test]
fn observation_schema_is_valid_json() {
    let data = fs::read_to_string(schema_path("observation.schema.json")).unwrap();
    let _: serde_json::Value = serde_json::from_str(&data).unwrap();
}

#[test]
fn driver_request_schema_is_valid_json() {
    let data = fs::read_to_string(schema_path("driver-request-v2.schema.json")).unwrap();
    let _: serde_json::Value = serde_json::from_str(&data).unwrap();
}

#[test]
fn driver_response_schema_is_valid_json() {
    let data = fs::read_to_string(schema_path("driver-response-v2.schema.json")).unwrap();
    let _: serde_json::Value = serde_json::from_str(&data).unwrap();
}

#[test]
fn scenario_example_conforms_to_schema_subset() {
    let schema: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(schema_path("scenario.schema.json")).unwrap())
            .unwrap();
    let example: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(example_path("audio-extractor-inline.json")).unwrap(),
    )
    .unwrap();
    validate_required(&schema, &example);
    validate_enums(&schema, &example);
}

#[test]
fn scenario_schema_has_required_fields() {
    let schema: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(schema_path("scenario.schema.json")).unwrap())
            .unwrap();

    // Verify the schema declares its required top-level fields
    let required = schema.get("required").and_then(|v| v.as_array()).unwrap();
    let required_strings: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
    assert!(required_strings.contains(&"scenario_version"));
    assert!(required_strings.contains(&"metadata"));
    assert!(required_strings.contains(&"run"));
    assert!(required_strings.contains(&"steps"));
}

#[test]
fn run_result_schema_has_required_fields() {
    let schema: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(schema_path("run-result.schema.json")).unwrap())
            .unwrap();

    // Verify the schema declares its required top-level fields
    let required = schema.get("required").and_then(|v| v.as_array()).unwrap();
    let required_strings: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
    assert!(required_strings.contains(&"run_result_version"));
    assert!(required_strings.contains(&"protocol_version"));
    assert!(required_strings.contains(&"run_id"));
    assert!(required_strings.contains(&"status"));
}

#[test]
fn observation_schema_has_required_fields() {
    let schema: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(schema_path("observation.schema.json")).unwrap())
            .unwrap();

    // Verify the schema declares its required top-level fields
    let required = schema.get("required").and_then(|v| v.as_array()).unwrap();
    let required_strings: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
    assert!(required_strings.contains(&"protocol_version"));
    assert!(required_strings.contains(&"run_id"));
    assert!(required_strings.contains(&"session_id"));
    assert!(required_strings.contains(&"timestamp_ms"));
    assert!(required_strings.contains(&"screen"));
}

#[test]
fn driver_request_schema_has_required_fields() {
    let schema: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(schema_path("driver-request-v2.schema.json")).unwrap(),
    )
    .unwrap();

    let required = schema.get("required").and_then(|v| v.as_array()).unwrap();
    let required_strings: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
    assert!(required_strings.contains(&"protocol_version"));
    assert!(required_strings.contains(&"request_id"));
    assert!(required_strings.contains(&"action"));
}

#[test]
fn driver_response_schema_has_required_fields() {
    let schema: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(schema_path("driver-response-v2.schema.json")).unwrap(),
    )
    .unwrap();

    let required = schema.get("required").and_then(|v| v.as_array()).unwrap();
    let required_strings: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
    assert!(required_strings.contains(&"protocol_version"));
    assert!(required_strings.contains(&"request_id"));
    assert!(required_strings.contains(&"status"));
}
