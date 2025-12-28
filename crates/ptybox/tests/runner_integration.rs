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

//! Runner API integration tests
//!
//! Tests the high-level `run_scenario` and `run_exec` functions.

use ptybox::model::policy::PolicyBuilder;
use ptybox::model::scenario::PolicyRef;
use ptybox::model::{
    Action, ActionType, RunConfig, RunStatus, Scenario, ScenarioMetadata, Step, StepId, StepStatus,
    TerminalSize,
};
use ptybox::run::{run_exec, run_scenario};

// =============================================================================
// Helper Functions
// =============================================================================

fn minimal_policy() -> ptybox::model::Policy {
    PolicyBuilder::new()
        .sandbox_disabled()
        .allowed_executables(vec![
            "/bin/echo".to_string(),
            "/bin/sleep".to_string(),
            "/bin/sh".to_string(),
        ])
        .max_runtime_ms(10_000)
        .build()
}

fn create_scenario(steps: Vec<Step>, command: &str, args: Vec<String>) -> Scenario {
    Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "test_scenario".to_string(),
            description: Some("Integration test scenario".to_string()),
        },
        run: RunConfig {
            command: command.to_string(),
            args,
            cwd: None,
            initial_size: TerminalSize::default(),
            policy: PolicyRef::Inline(minimal_policy()),
        },
        steps,
    }
}

// =============================================================================
// run_scenario Tests
// =============================================================================

#[test]
fn run_scenario_with_empty_steps() {
    // Create a scenario with 0 steps
    let scenario = create_scenario(vec![], "/bin/echo", vec!["done".to_string()]);

    let result = run_scenario(scenario);

    assert!(
        result.is_ok(),
        "Empty scenario should succeed: {:?}",
        result.err()
    );

    let run_result = result.unwrap();
    assert_eq!(
        run_result.status,
        RunStatus::Passed,
        "Empty scenario should pass"
    );
    assert!(run_result.steps.is_some(), "Steps should be present");
    assert!(
        run_result.steps.as_ref().unwrap().is_empty(),
        "Steps should be empty"
    );
    assert!(
        run_result.exit_status.is_some(),
        "Exit status should be present"
    );
    assert!(
        run_result.exit_status.as_ref().unwrap().success,
        "Echo should exit successfully"
    );
}

#[test]
fn run_scenario_assertion_retry_success() {
    // Create a scenario that tests assertion with a wait condition for process exit.
    // Use cat which stays running until we send input, then use process_exited condition.

    let policy = PolicyBuilder::new()
        .sandbox_disabled()
        .allowed_executables(vec!["/bin/cat".to_string()])
        .max_runtime_ms(10_000)
        .max_wait_ms(5000)
        .build();

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "retry_test".to_string(),
            description: Some("Test assertion with cat and terminate".to_string()),
        },
        run: RunConfig {
            command: "/bin/cat".to_string(),
            args: vec![],
            cwd: None,
            initial_size: TerminalSize::default(),
            policy: PolicyRef::Inline(policy),
        },
        steps: vec![
            // Step 1: Send some text to cat
            Step {
                id: StepId::new(),
                name: "send_text".to_string(),
                action: Action {
                    action_type: ActionType::Text,
                    payload: serde_json::json!({"text": "Hello Test"}),
                },
                assert: vec![], // No assertions on this step
                timeout_ms: 1000,
                retries: 0,
            },
            // Step 2: Terminate cat
            Step {
                id: StepId::new(),
                name: "terminate".to_string(),
                action: Action {
                    action_type: ActionType::Terminate,
                    payload: serde_json::json!({}),
                },
                assert: vec![], // No assertions
                timeout_ms: 1000,
                retries: 0,
            },
        ],
    };

    let result = run_scenario(scenario);

    assert!(
        result.is_ok(),
        "Scenario should succeed: {:?}",
        result.err()
    );

    let run_result = result.unwrap();
    assert_eq!(run_result.status, RunStatus::Passed, "Scenario should pass");

    let steps = run_result.steps.as_ref().unwrap();
    assert_eq!(steps.len(), 2, "Should have two steps");
    assert_eq!(
        steps[0].status,
        StepStatus::Passed,
        "First step should pass"
    );
    assert_eq!(
        steps[1].status,
        StepStatus::Passed,
        "Second step should pass"
    );
}

#[test]
fn run_scenario_timeout_boundary() {
    // Test behavior at exact timeout boundary by setting a very short timeout
    // and running a command that takes longer than that.

    let policy = PolicyBuilder::new()
        .sandbox_disabled()
        .allowed_executables(vec!["/bin/sleep".to_string()])
        .max_runtime_ms(100) // Very short runtime budget
        .build();

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "timeout_test".to_string(),
            description: Some("Test timeout boundary".to_string()),
        },
        run: RunConfig {
            command: "/bin/sleep".to_string(),
            args: vec!["10".to_string()], // Sleep for 10 seconds (way over budget)
            cwd: None,
            initial_size: TerminalSize::default(),
            policy: PolicyRef::Inline(policy),
        },
        steps: vec![], // No steps - just let it run until timeout
    };

    let start = std::time::Instant::now();
    let result = run_scenario(scenario);
    let elapsed = start.elapsed();

    // Should fail with timeout error
    assert!(result.is_err(), "Should timeout");

    let err = result.unwrap_err();
    assert_eq!(
        err.code,
        ptybox::runner::ErrorCode::Timeout,
        "Should be timeout error"
    );

    // Should complete reasonably quickly (not wait 10 seconds)
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "Should timeout quickly, took {:?}",
        elapsed
    );
}

// =============================================================================
// run_exec Tests
// =============================================================================

#[test]
fn run_exec_simple_command() {
    let policy = minimal_policy();

    let result = run_exec(
        "/bin/echo".to_string(),
        vec!["hello".to_string()],
        None,
        policy,
    );

    assert!(
        result.is_ok(),
        "run_exec should succeed: {:?}",
        result.err()
    );

    let run_result = result.unwrap();
    assert_eq!(run_result.status, RunStatus::Passed, "Echo should pass");
    assert_eq!(run_result.command, "/bin/echo", "Command should match");
    assert_eq!(run_result.args, vec!["hello"], "Args should match");

    // Verify exit status
    assert!(run_result.exit_status.is_some(), "Should have exit status");
    let exit_status = run_result.exit_status.unwrap();
    assert!(exit_status.success, "Echo should exit successfully");
    assert_eq!(exit_status.exit_code, Some(0), "Exit code should be 0");
    assert!(
        !exit_status.terminated_by_harness,
        "Should not be terminated by harness"
    );

    // Verify final observation contains output
    assert!(
        run_result.final_observation.is_some(),
        "Should have final observation"
    );
    let observation = run_result.final_observation.unwrap();
    let screen_text = observation.screen.lines.join("\n");
    assert!(
        screen_text.contains("hello"),
        "Screen should contain 'hello', got: {}",
        screen_text
    );
}

#[test]
fn run_exec_with_args() {
    let policy = minimal_policy();

    let result = run_exec(
        "/bin/echo".to_string(),
        vec!["one".to_string(), "two".to_string(), "three".to_string()],
        None,
        policy,
    );

    assert!(
        result.is_ok(),
        "run_exec should succeed: {:?}",
        result.err()
    );

    let run_result = result.unwrap();
    assert_eq!(run_result.status, RunStatus::Passed, "Should pass");

    // Verify args are recorded correctly
    assert_eq!(
        run_result.args,
        vec!["one", "two", "three"],
        "Args should match"
    );

    // Verify exit status
    assert!(run_result.exit_status.is_some(), "Should have exit status");
    assert!(
        run_result.exit_status.as_ref().unwrap().success,
        "Should succeed"
    );
    assert_eq!(
        run_result.exit_status.as_ref().unwrap().exit_code,
        Some(0),
        "Exit code should be 0"
    );

    // Verify command was recorded
    assert_eq!(run_result.command, "/bin/echo", "Command should match");
}

#[test]
fn run_exec_nonzero_exit_code() {
    let policy = PolicyBuilder::new()
        .sandbox_disabled()
        .allowed_executables(vec!["/bin/sh".to_string()])
        .allow_shell()
        .max_runtime_ms(5000)
        .build();

    let result = run_exec(
        "/bin/sh".to_string(),
        vec!["-c".to_string(), "exit 42".to_string()],
        None,
        policy,
    );

    assert!(
        result.is_ok(),
        "run_exec should complete: {:?}",
        result.err()
    );

    let run_result = result.unwrap();
    assert_eq!(
        run_result.status,
        RunStatus::Failed,
        "Non-zero exit should result in Failed status"
    );

    // Verify exit status
    let exit_status = run_result.exit_status.unwrap();
    assert!(!exit_status.success, "Should not be successful");
    assert_eq!(exit_status.exit_code, Some(42), "Exit code should be 42");
}

#[test]
fn run_exec_captures_output() {
    let policy = minimal_policy();

    let result = run_exec(
        "/bin/echo".to_string(),
        vec!["test_output_12345".to_string()],
        None,
        policy,
    );

    assert!(
        result.is_ok(),
        "run_exec should succeed: {:?}",
        result.err()
    );

    let run_result = result.unwrap();
    let observation = run_result
        .final_observation
        .expect("Should have observation");

    // Check screen lines for the output
    let found = observation
        .screen
        .lines
        .iter()
        .any(|line| line.contains("test_output_12345"));

    assert!(found, "Output should contain test string");
}
