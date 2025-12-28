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

//! Replay module unit tests
//!
//! Tests the replay comparison and normalization functionality.

use std::fs;
use std::path::{Path, PathBuf};
use ptybox::model::{NormalizationFilter, NormalizationRuleTarget, Policy};
use ptybox::replay::{explain_replay, ReplayOptions};
use ptybox::runner::ErrorCode;

/// Write a test policy file with optional replay configuration.
fn write_test_policy(dir: &Path, replay_config: Option<serde_json::Value>) {
    let mut policy = Policy::default();
    // Set up minimal valid policy
    policy.exec.allowed_executables = vec!["/bin/echo".to_string()];
    policy.fs.allowed_read = vec!["/tmp".to_string()];

    // Serialize to JSON and inject replay config if provided
    let mut policy_value = serde_json::to_value(&policy).expect("Failed to serialize policy");
    if let Some(replay) = replay_config {
        policy_value["replay"] = replay;
    }

    fs::write(
        dir.join("policy.json"),
        serde_json::to_string_pretty(&policy_value).unwrap(),
    )
    .expect("Failed to write policy");
}

fn temp_test_dir(name: &str) -> PathBuf {
    // Include thread ID for test isolation when running tests in parallel
    let thread_id = format!("{:?}", std::thread::current().id());
    // Extract numeric part from ThreadId format "ThreadId(N)"
    let thread_num = thread_id
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>();

    let dir = std::env::temp_dir().join(format!(
        "ptybox-replay-test-{}-{}-{}-{}",
        name,
        std::process::id(),
        thread_num,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("Failed to create test dir");
    dir
}

fn cleanup_dir(dir: &PathBuf) {
    let _ = fs::remove_dir_all(dir);
}

// =============================================================================
// Explain Replay Tests
// =============================================================================

#[test]
fn explain_replay_missing_policy_fails() {
    let dir = temp_test_dir("explain-missing");

    let result = explain_replay(&dir, ReplayOptions::default());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::Io);
    assert!(err.message.contains("policy.json"));

    cleanup_dir(&dir);
}

#[test]
fn explain_replay_with_valid_policy() {
    let dir = temp_test_dir("explain-valid");
    write_test_policy(&dir, Some(serde_json::json!({})));

    let result = explain_replay(&dir, ReplayOptions::default());
    assert!(result.is_ok(), "Should succeed: {:?}", result.err());

    let explanation = result.unwrap();
    assert!(!explanation.strict, "Default should not be strict");
    assert!(
        !explanation.filters.is_empty(),
        "Should have default filters"
    );

    cleanup_dir(&dir);
}

#[test]
fn explain_replay_strict_mode_overrides_policy() {
    let dir = temp_test_dir("explain-strict");
    write_test_policy(
        &dir,
        Some(serde_json::json!({
            "normalization_filters": ["snapshot_id", "run_id"]
        })),
    );

    let options = ReplayOptions {
        strict: true,
        ..Default::default()
    };
    let result = explain_replay(&dir, options);
    assert!(result.is_ok(), "Should succeed: {:?}", result.err());

    let explanation = result.unwrap();
    assert!(explanation.strict, "Strict should be true");
    assert!(
        explanation.filters.is_empty(),
        "Strict mode should have no filters"
    );

    cleanup_dir(&dir);
}

#[test]
fn explain_replay_cli_filters_override_policy() {
    let dir = temp_test_dir("explain-cli-filters");
    write_test_policy(
        &dir,
        Some(serde_json::json!({
            "normalization_filters": ["snapshot_id", "run_id", "session_id"]
        })),
    );

    let options = ReplayOptions {
        filters: Some(vec![NormalizationFilter::RunId]),
        ..Default::default()
    };
    let result = explain_replay(&dir, options);
    assert!(result.is_ok(), "Should succeed: {:?}", result.err());

    let explanation = result.unwrap();
    assert!(!explanation.strict);
    assert_eq!(explanation.filters.len(), 1);
    assert_eq!(explanation.filters[0], NormalizationFilter::RunId);

    cleanup_dir(&dir);
}

// =============================================================================
// Filter Precedence Tests
// =============================================================================

#[test]
fn filter_precedence_strict_first() {
    let dir = temp_test_dir("precedence-strict");
    write_test_policy(
        &dir,
        Some(serde_json::json!({
            "normalization_filters": ["snapshot_id"]
        })),
    );

    // Both strict=true and filters provided - strict wins
    let options = ReplayOptions {
        strict: true,
        filters: Some(vec![
            NormalizationFilter::RunId,
            NormalizationFilter::SessionId,
        ]),
        ..Default::default()
    };
    let result = explain_replay(&dir, options).expect("Should succeed");

    assert!(result.strict, "Strict should be true");
    assert!(result.filters.is_empty(), "Strict mode ignores filters");

    cleanup_dir(&dir);
}

#[test]
fn filter_precedence_cli_over_policy() {
    let dir = temp_test_dir("precedence-cli");
    write_test_policy(
        &dir,
        Some(serde_json::json!({
            "normalization_filters": ["snapshot_id", "run_id", "session_id"]
        })),
    );

    // CLI filters should override policy
    let options = ReplayOptions {
        filters: Some(vec![NormalizationFilter::RunTimestamps]),
        ..Default::default()
    };
    let result = explain_replay(&dir, options).expect("Should succeed");

    // Should have only the CLI-specified filter
    assert_eq!(result.filters.len(), 1);
    assert_eq!(result.filters[0], NormalizationFilter::RunTimestamps);

    cleanup_dir(&dir);
}

#[test]
fn filter_precedence_policy_strict() {
    let dir = temp_test_dir("precedence-policy-strict");
    write_test_policy(
        &dir,
        Some(serde_json::json!({
            "strict": true,
            "normalization_filters": ["snapshot_id"]
        })),
    );

    // No CLI options - should use policy strict
    let result = explain_replay(&dir, ReplayOptions::default()).expect("Should succeed");

    assert!(result.strict, "Policy strict should be honored");
    assert!(result.filters.is_empty(), "Strict mode has no filters");

    cleanup_dir(&dir);
}

#[test]
fn filter_precedence_policy_filters() {
    let dir = temp_test_dir("precedence-policy-filters");
    write_test_policy(
        &dir,
        Some(serde_json::json!({
            "normalization_filters": ["snapshot_id", "run_id"]
        })),
    );

    // No CLI options - should use policy filters
    let result = explain_replay(&dir, ReplayOptions::default()).expect("Should succeed");

    assert!(!result.strict);
    assert_eq!(result.filters.len(), 2);
    assert!(result.filters.contains(&NormalizationFilter::SnapshotId));
    assert!(result.filters.contains(&NormalizationFilter::RunId));

    cleanup_dir(&dir);
}

#[test]
fn filter_precedence_default_when_empty() {
    let dir = temp_test_dir("precedence-default");
    write_test_policy(&dir, Some(serde_json::json!({})));

    let result = explain_replay(&dir, ReplayOptions::default()).expect("Should succeed");

    // Should have default filters
    assert!(!result.strict);
    assert!(!result.filters.is_empty(), "Should have default filters");

    // Verify the EXACT set of default filters (as defined in default_replay_filters())
    // Default filters should be exactly these 6 filters:
    assert_eq!(
        result.filters.len(),
        6,
        "Should have exactly 6 default filters"
    );

    // Verify each specific default filter is present
    assert!(
        result.filters.contains(&NormalizationFilter::SnapshotId),
        "Default filters should include SnapshotId"
    );
    assert!(
        result.filters.contains(&NormalizationFilter::RunId),
        "Default filters should include RunId"
    );
    assert!(
        result.filters.contains(&NormalizationFilter::RunTimestamps),
        "Default filters should include RunTimestamps"
    );
    assert!(
        result
            .filters
            .contains(&NormalizationFilter::StepTimestamps),
        "Default filters should include StepTimestamps"
    );
    assert!(
        result
            .filters
            .contains(&NormalizationFilter::ObservationTimestamp),
        "Default filters should include ObservationTimestamp"
    );
    assert!(
        result.filters.contains(&NormalizationFilter::SessionId),
        "Default filters should include SessionId"
    );

    cleanup_dir(&dir);
}

// =============================================================================
// Normalization Rule Tests
// =============================================================================

#[test]
fn normalization_rules_from_policy() {
    let dir = temp_test_dir("norm-rules");
    write_test_policy(
        &dir,
        Some(serde_json::json!({
            "normalization_rules": [
                {
                    "target": "snapshot_lines",
                    "pattern": "\\d{4}-\\d{2}-\\d{2}",
                    "replace": "YYYY-MM-DD"
                }
            ]
        })),
    );

    let result = explain_replay(&dir, ReplayOptions::default()).expect("Should succeed");

    assert_eq!(result.rules.len(), 1);
    assert_eq!(
        result.rules[0].target,
        NormalizationRuleTarget::SnapshotLines
    );
    assert_eq!(result.rules[0].pattern, "\\d{4}-\\d{2}-\\d{2}");
    assert_eq!(result.rules[0].replace, "YYYY-MM-DD");

    cleanup_dir(&dir);
}

// =============================================================================
// Source Tracking Tests
// =============================================================================

#[test]
fn source_tracking_cli() {
    let dir = temp_test_dir("source-cli");
    write_test_policy(&dir, Some(serde_json::json!({})));

    // CLI strict
    let options = ReplayOptions {
        strict: true,
        ..Default::default()
    };
    let result = explain_replay(&dir, options).expect("Should succeed");
    assert_eq!(
        result.source,
        ptybox::model::NormalizationSource::Cli,
        "Strict from CLI should be source Cli"
    );

    // CLI filters
    let options = ReplayOptions {
        filters: Some(vec![NormalizationFilter::RunId]),
        ..Default::default()
    };
    let result = explain_replay(&dir, options).expect("Should succeed");
    assert_eq!(
        result.source,
        ptybox::model::NormalizationSource::Cli,
        "Filters from CLI should be source Cli"
    );

    cleanup_dir(&dir);
}

#[test]
fn source_tracking_policy() {
    let dir = temp_test_dir("source-policy");
    write_test_policy(
        &dir,
        Some(serde_json::json!({
            "normalization_filters": ["snapshot_id"]
        })),
    );

    let result = explain_replay(&dir, ReplayOptions::default()).expect("Should succeed");
    assert_eq!(
        result.source,
        ptybox::model::NormalizationSource::Policy,
        "Filters from policy should be source Policy"
    );

    cleanup_dir(&dir);
}

#[test]
fn source_tracking_default() {
    let dir = temp_test_dir("source-default");
    write_test_policy(&dir, Some(serde_json::json!({})));

    let result = explain_replay(&dir, ReplayOptions::default()).expect("Should succeed");
    assert_eq!(
        result.source,
        ptybox::model::NormalizationSource::Default,
        "No overrides should use source Default"
    );

    cleanup_dir(&dir);
}
