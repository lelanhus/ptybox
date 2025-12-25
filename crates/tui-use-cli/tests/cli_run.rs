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
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use tui_use::model::policy::{
    EnvPolicy, ExecPolicy, FsPolicy, NetworkPolicy, Policy, ReplayPolicy, SandboxMode,
    POLICY_VERSION,
};
use tui_use::model::{
    Action, ActionType, Assertion, RunResult, Scenario, ScenarioMetadata, Step, StepId,
    TerminalSize,
};

fn temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    dir.push(format!("tui-use-cli-test-{prefix}-{stamp}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_scenario(path: &Path, scenario: &Scenario) {
    let data = serde_json::to_vec_pretty(scenario).unwrap();
    fs::write(path, data).unwrap();
}

fn write_scenario_yaml(path: &Path, scenario: &Scenario) {
    let data = serde_yml::to_string(scenario).unwrap();
    fs::write(path, data).unwrap();
}

fn base_policy(work_dir: &Path, allowed_exec: Vec<String>) -> Policy {
    Policy {
        policy_version: POLICY_VERSION,
        sandbox: SandboxMode::None,
        sandbox_unsafe_ack: true,
        network: NetworkPolicy::Disabled,
        network_unsafe_ack: true,
        fs: FsPolicy {
            allowed_read: vec![work_dir.display().to_string()],
            allowed_write: Vec::new(),
            working_dir: Some(work_dir.display().to_string()),
        },
        fs_write_unsafe_ack: false,
        fs_strict_write: false,
        exec: ExecPolicy {
            allowed_executables: allowed_exec,
            allow_shell: false,
        },
        env: EnvPolicy {
            allowlist: Vec::new(),
            set: Default::default(),
            inherit: false,
        },
        budgets: Default::default(),
        artifacts: Default::default(),
        replay: ReplayPolicy::default(),
    }
}

#[test]
fn run_scenario_outputs_passed_run_result() {
    let dir = temp_dir("scenario-run");
    let fixture = "/bin/cat".to_string();
    let policy = base_policy(&dir, vec![fixture.clone()]);

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "echo".to_string(),
            description: None,
        },
        run: tui_use::model::RunConfig {
            command: fixture,
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: tui_use::model::scenario::PolicyRef::Inline(policy),
        },
        steps: vec![
            Step {
                id: StepId::new(),
                name: "type".to_string(),
                action: Action {
                    action_type: ActionType::Text,
                    payload: serde_json::json!({"text": "hello"}),
                },
                assert: vec![Assertion {
                    assertion_type: "screen_contains".to_string(),
                    payload: serde_json::json!({"text": "hello"}),
                }],
                timeout_ms: 1000,
                retries: 0,
            },
            Step {
                id: StepId::new(),
                name: "terminate".to_string(),
                action: Action {
                    action_type: ActionType::Terminate,
                    payload: serde_json::json!({}),
                },
                assert: Vec::new(),
                timeout_ms: 1000,
                retries: 0,
            },
        ],
    };

    let scenario_path = dir.join("scenario.json");
    write_scenario(&scenario_path, &scenario);

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "run",
            "--json",
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    if !output.status.success() {
        panic!(
            "run failed: status={:?}\nstdout={}\nstderr={}\nscenario={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
            fs::read_to_string(&scenario_path).unwrap_or_default()
        );
    }
    let run: RunResult = serde_json::from_slice(&output.stdout).unwrap();
    if !matches!(run.status, tui_use::model::RunStatus::Passed) {
        panic!("run failed: {:#?}", run);
    }
    assert!(run.steps.as_ref().unwrap()[0]
        .status
        .eq(&tui_use::model::StepStatus::Passed));
}

#[test]
fn run_scenario_accepts_yaml() {
    let dir = temp_dir("scenario-yaml");
    let fixture = "/bin/cat".to_string();
    let policy = base_policy(&dir, vec![fixture.clone()]);

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "echo".to_string(),
            description: None,
        },
        run: tui_use::model::RunConfig {
            command: fixture,
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: tui_use::model::scenario::PolicyRef::Inline(policy),
        },
        steps: vec![
            Step {
                id: StepId::new(),
                name: "type".to_string(),
                action: Action {
                    action_type: ActionType::Text,
                    payload: serde_json::json!({"text": "hello"}),
                },
                assert: vec![Assertion {
                    assertion_type: "screen_contains".to_string(),
                    payload: serde_json::json!({"text": "hello"}),
                }],
                timeout_ms: 1000,
                retries: 0,
            },
            Step {
                id: StepId::new(),
                name: "terminate".to_string(),
                action: Action {
                    action_type: ActionType::Terminate,
                    payload: serde_json::json!({}),
                },
                assert: Vec::new(),
                timeout_ms: 1000,
                retries: 0,
            },
        ],
    };

    let scenario_path = dir.join("scenario.yaml");
    write_scenario_yaml(&scenario_path, &scenario);

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "run",
            "--json",
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let run: RunResult = serde_json::from_slice(&output.stdout).unwrap();
    assert!(matches!(run.status, tui_use::model::RunStatus::Passed));
}

#[test]
fn run_scenario_supports_key_action() {
    let dir = temp_dir("scenario-key");
    let fixture = "/bin/cat".to_string();
    let policy = base_policy(&dir, vec![fixture.clone()]);

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "key".to_string(),
            description: None,
        },
        run: tui_use::model::RunConfig {
            command: fixture,
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: tui_use::model::scenario::PolicyRef::Inline(policy),
        },
        steps: vec![
            Step {
                id: StepId::new(),
                name: "key".to_string(),
                action: Action {
                    action_type: ActionType::Key,
                    payload: serde_json::json!({"key": "a"}),
                },
                assert: vec![Assertion {
                    assertion_type: "screen_contains".to_string(),
                    payload: serde_json::json!({"text": "a"}),
                }],
                timeout_ms: 1000,
                retries: 0,
            },
            Step {
                id: StepId::new(),
                name: "terminate".to_string(),
                action: Action {
                    action_type: ActionType::Terminate,
                    payload: serde_json::json!({}),
                },
                assert: Vec::new(),
                timeout_ms: 1000,
                retries: 0,
            },
        ],
    };

    let scenario_path = dir.join("scenario.json");
    write_scenario(&scenario_path, &scenario);

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "run",
            "--json",
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let run: RunResult = serde_json::from_slice(&output.stdout).unwrap();
    assert!(matches!(run.status, tui_use::model::RunStatus::Passed));
}

#[test]
fn run_scenario_supports_resize_action() {
    let dir = temp_dir("scenario-resize");
    let fixture = "/bin/cat".to_string();
    let artifacts_dir = dir.join("artifacts");
    let mut policy = base_policy(&dir, vec![fixture.clone()]);
    policy.fs.allowed_write = vec![artifacts_dir.display().to_string()];
    policy.fs_write_unsafe_ack = true;

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "resize".to_string(),
            description: None,
        },
        run: tui_use::model::RunConfig {
            command: fixture,
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: tui_use::model::scenario::PolicyRef::Inline(policy),
        },
        steps: vec![
            Step {
                id: StepId::new(),
                name: "resize".to_string(),
                action: Action {
                    action_type: ActionType::Resize,
                    payload: serde_json::json!({"rows": 40, "cols": 100}),
                },
                assert: Vec::new(),
                timeout_ms: 1000,
                retries: 0,
            },
            Step {
                id: StepId::new(),
                name: "type".to_string(),
                action: Action {
                    action_type: ActionType::Text,
                    payload: serde_json::json!({"text": "x"}),
                },
                assert: Vec::new(),
                timeout_ms: 1000,
                retries: 0,
            },
            Step {
                id: StepId::new(),
                name: "terminate".to_string(),
                action: Action {
                    action_type: ActionType::Terminate,
                    payload: serde_json::json!({}),
                },
                assert: Vec::new(),
                timeout_ms: 1000,
                retries: 0,
            },
        ],
    };

    let scenario_path = dir.join("scenario.json");
    write_scenario(&scenario_path, &scenario);

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "run",
            "--json",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let run: RunResult = serde_json::from_slice(&output.stdout).unwrap();
    assert!(matches!(run.status, tui_use::model::RunStatus::Passed));
    let snapshot: tui_use::model::ScreenSnapshot = serde_json::from_str(
        &fs::read_to_string(artifacts_dir.join("snapshots/000001.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(snapshot.rows, 40);
    assert_eq!(snapshot.cols, 100);
}

#[test]
fn run_scenario_supports_wait_action() {
    let dir = temp_dir("scenario-wait");
    let fixture = "/bin/cat".to_string();
    let policy = base_policy(&dir, vec![fixture.clone()]);

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "wait".to_string(),
            description: None,
        },
        run: tui_use::model::RunConfig {
            command: fixture,
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: tui_use::model::scenario::PolicyRef::Inline(policy),
        },
        steps: vec![
            Step {
                id: StepId::new(),
                name: "type".to_string(),
                action: Action {
                    action_type: ActionType::Text,
                    payload: serde_json::json!({"text": "hello"}),
                },
                assert: Vec::new(),
                timeout_ms: 1000,
                retries: 0,
            },
            Step {
                id: StepId::new(),
                name: "wait".to_string(),
                action: Action {
                    action_type: ActionType::Wait,
                    payload: serde_json::json!({"condition": {"type": "screen_contains", "payload": {"text": "hello"}}}),
                },
                assert: Vec::new(),
                timeout_ms: 500,
                retries: 0,
            },
            Step {
                id: StepId::new(),
                name: "terminate".to_string(),
                action: Action {
                    action_type: ActionType::Terminate,
                    payload: serde_json::json!({}),
                },
                assert: Vec::new(),
                timeout_ms: 1000,
                retries: 0,
            },
        ],
    };

    let scenario_path = dir.join("scenario.json");
    write_scenario(&scenario_path, &scenario);

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "run",
            "--json",
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let run: RunResult = serde_json::from_slice(&output.stdout).unwrap();
    assert!(matches!(run.status, tui_use::model::RunStatus::Passed));
}

#[test]
fn run_scenario_retries_failed_steps() {
    let dir = temp_dir("scenario-retries");
    let fixture = "/bin/cat".to_string();
    let policy = base_policy(&dir, vec![fixture.clone()]);

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "retries".to_string(),
            description: None,
        },
        run: tui_use::model::RunConfig {
            command: fixture,
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: tui_use::model::scenario::PolicyRef::Inline(policy),
        },
        steps: vec![
            Step {
                id: StepId::new(),
                name: "type".to_string(),
                action: Action {
                    action_type: ActionType::Text,
                    payload: serde_json::json!({"text": "hi"}),
                },
                assert: vec![Assertion {
                    assertion_type: "screen_contains".to_string(),
                    payload: serde_json::json!({"text": "hihi"}),
                }],
                timeout_ms: 1000,
                retries: 1,
            },
            Step {
                id: StepId::new(),
                name: "terminate".to_string(),
                action: Action {
                    action_type: ActionType::Terminate,
                    payload: serde_json::json!({}),
                },
                assert: Vec::new(),
                timeout_ms: 1000,
                retries: 0,
            },
        ],
    };

    let scenario_path = dir.join("scenario.json");
    write_scenario(&scenario_path, &scenario);

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "run",
            "--json",
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let run: RunResult = serde_json::from_slice(&output.stdout).unwrap();
    assert!(matches!(run.status, tui_use::model::RunStatus::Passed));
    let steps = run.steps.expect("steps should exist");
    assert_eq!(steps[0].attempts, 2);
}

#[test]
fn run_scenario_is_deterministic_for_same_inputs() {
    let dir = temp_dir("scenario-deterministic");
    let fixture = "/bin/cat".to_string();
    let artifacts_a = dir.join("artifacts-a");
    let artifacts_b = dir.join("artifacts-b");
    let mut policy = base_policy(&dir, vec![fixture.clone()]);
    policy.fs.allowed_write = vec![
        artifacts_a.display().to_string(),
        artifacts_b.display().to_string(),
    ];
    policy.fs_write_unsafe_ack = true;

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "deterministic".to_string(),
            description: None,
        },
        run: tui_use::model::RunConfig {
            command: fixture,
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: tui_use::model::scenario::PolicyRef::Inline(policy),
        },
        steps: vec![
            Step {
                id: StepId::new(),
                name: "type".to_string(),
                action: Action {
                    action_type: ActionType::Text,
                    payload: serde_json::json!({"text": "hello"}),
                },
                assert: vec![Assertion {
                    assertion_type: "screen_contains".to_string(),
                    payload: serde_json::json!({"text": "hello"}),
                }],
                timeout_ms: 1000,
                retries: 0,
            },
            Step {
                id: StepId::new(),
                name: "terminate".to_string(),
                action: Action {
                    action_type: ActionType::Terminate,
                    payload: serde_json::json!({}),
                },
                assert: Vec::new(),
                timeout_ms: 1000,
                retries: 0,
            },
        ],
    };

    let scenario_path = dir.join("scenario.json");
    write_scenario(&scenario_path, &scenario);

    let output_a = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "run",
            "--json",
            "--artifacts",
            artifacts_a.to_str().unwrap(),
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output_a.status.success());

    let output_b = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "run",
            "--json",
            "--artifacts",
            artifacts_b.to_str().unwrap(),
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output_b.status.success());

    let snapshot_a: tui_use::model::ScreenSnapshot = serde_json::from_str(
        &fs::read_to_string(artifacts_a.join("snapshots/000001.json")).unwrap(),
    )
    .unwrap();
    let snapshot_b: tui_use::model::ScreenSnapshot = serde_json::from_str(
        &fs::read_to_string(artifacts_b.join("snapshots/000001.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(snapshot_a.lines, snapshot_b.lines);
    assert_eq!(snapshot_a.cursor.row, snapshot_b.cursor.row);
    assert_eq!(snapshot_a.cursor.col, snapshot_b.cursor.col);
}

#[test]
fn run_timeout_includes_step_context() {
    let dir = temp_dir("scenario-timeout-context");
    let fixture = "/bin/cat".to_string();
    let policy = base_policy(&dir, vec![fixture.clone()]);

    let step_id = StepId::new();
    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "timeout-context".to_string(),
            description: None,
        },
        run: tui_use::model::RunConfig {
            command: fixture,
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: tui_use::model::scenario::PolicyRef::Inline(policy),
        },
        steps: vec![
            Step {
                id: step_id,
                name: "wait".to_string(),
                action: Action {
                    action_type: ActionType::Wait,
                    payload: serde_json::json!({"condition": {"type": "screen_contains", "payload": {"text": "never"}}}),
                },
                assert: Vec::new(),
                timeout_ms: 50,
                retries: 0,
            },
            Step {
                id: StepId::new(),
                name: "terminate".to_string(),
                action: Action {
                    action_type: ActionType::Terminate,
                    payload: serde_json::json!({}),
                },
                assert: Vec::new(),
                timeout_ms: 1000,
                retries: 0,
            },
        ],
    };

    let scenario_path = dir.join("scenario.json");
    write_scenario(&scenario_path, &scenario);

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "run",
            "--json",
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(4));
    let run: RunResult = serde_json::from_slice(&output.stdout).unwrap();
    let err = run.error.expect("run should include error");
    assert_eq!(err.code, "E_TIMEOUT");
    let context = err.context.expect("timeout should include context");
    assert_eq!(
        context
            .get("step_id")
            .and_then(|value| value.as_str())
            .unwrap_or_default(),
        step_id.to_string()
    );
    let details = context.get("details").expect("details should exist");
    assert!(details.get("condition").is_some());
}

#[test]
fn run_writes_artifacts_on_assertion_failure() {
    let dir = temp_dir("scenario-assert-fail");
    let artifacts_dir = dir.join("artifacts");
    let fixture = "/bin/cat".to_string();
    let mut policy = base_policy(&dir, vec![fixture.clone()]);
    policy.fs.allowed_write = vec![artifacts_dir.display().to_string()];
    policy.fs_write_unsafe_ack = true;

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "assert-fail".to_string(),
            description: None,
        },
        run: tui_use::model::RunConfig {
            command: fixture,
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: tui_use::model::scenario::PolicyRef::Inline(policy),
        },
        steps: vec![Step {
            id: StepId::new(),
            name: "type".to_string(),
            action: Action {
                action_type: ActionType::Text,
                payload: serde_json::json!({"text": "hello\n"}),
            },
            assert: vec![Assertion {
                assertion_type: "screen_contains".to_string(),
                payload: serde_json::json!({"text": "nope"}),
            }],
            timeout_ms: 100,
            retries: 0,
        }],
    };
    let scenario_path = dir.join("scenario.json");
    write_scenario(&scenario_path, &scenario);

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "run",
            "--json",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(5));
    let run: RunResult = serde_json::from_slice(&output.stdout).unwrap();
    assert!(matches!(run.status, tui_use::model::RunStatus::Failed));
    assert!(artifacts_dir.join("run.json").exists());
    assert!(artifacts_dir.join("policy.json").exists());
}

#[test]
fn run_rejects_relative_scenario_cwd() {
    let dir = temp_dir("scenario-relative-cwd");
    let fixture = "/bin/cat".to_string();
    let policy = base_policy(&dir, vec![fixture.clone()]);

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "relative-cwd".to_string(),
            description: None,
        },
        run: tui_use::model::RunConfig {
            command: fixture,
            args: Vec::new(),
            cwd: Some("relative".to_string()),
            initial_size: TerminalSize::default(),
            policy: tui_use::model::scenario::PolicyRef::Inline(policy),
        },
        steps: vec![Step {
            id: StepId::new(),
            name: "terminate".to_string(),
            action: Action {
                action_type: ActionType::Terminate,
                payload: serde_json::json!({}),
            },
            assert: Vec::new(),
            timeout_ms: 100,
            retries: 0,
        }],
    };
    let scenario_path = dir.join("scenario.json");
    write_scenario(&scenario_path, &scenario);

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "run",
            "--json",
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(12));
    let err: tui_use::model::ErrorInfo = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(err.code, "E_CLI_INVALID_ARG");
}

#[test]
fn run_scenario_policy_file_writes_effective_policy() {
    let dir = temp_dir("scenario-policy-file");
    let fixture = "/bin/cat".to_string();
    let artifacts_dir = dir.join("artifacts");
    let policy_path = dir.join("policy.json");
    let scenario_path = dir.join("scenario.json");

    let mut policy = base_policy(&dir, vec![fixture.clone()]);
    policy.fs.allowed_write = vec![artifacts_dir.display().to_string()];
    policy.fs_write_unsafe_ack = true;

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "echo".to_string(),
            description: None,
        },
        run: tui_use::model::RunConfig {
            command: fixture,
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: tui_use::model::scenario::PolicyRef::File {
                path: policy_path.display().to_string(),
            },
        },
        steps: vec![Step {
            id: StepId::new(),
            name: "terminate".to_string(),
            action: Action {
                action_type: ActionType::Terminate,
                payload: serde_json::json!({}),
            },
            assert: Vec::new(),
            timeout_ms: 1000,
            retries: 0,
        }],
    };

    let policy_data = serde_json::to_vec_pretty(&policy).unwrap();
    fs::write(&policy_path, policy_data).unwrap();
    write_scenario(&scenario_path, &scenario);

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "run",
            "--json",
            "--scenario",
            scenario_path.to_str().unwrap(),
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    if !output.status.success() {
        panic!(
            "run failed: status={:?}\nstdout={}\nstderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let effective: Policy =
        serde_json::from_str(&fs::read_to_string(artifacts_dir.join("policy.json")).unwrap())
            .unwrap();
    assert_eq!(
        effective.exec.allowed_executables,
        policy.exec.allowed_executables
    );
    assert_eq!(effective.fs.allowed_read, policy.fs.allowed_read);
    assert_eq!(effective.fs.allowed_write, policy.fs.allowed_write);
}

#[test]
fn artifacts_layout_is_written() {
    let dir = temp_dir("artifacts-layout");
    let artifacts_dir = dir.join("artifacts");
    let fixture = "/bin/cat".to_string();
    let mut policy = base_policy(&dir, vec![fixture.clone()]);
    policy.fs.allowed_write = vec![artifacts_dir.display().to_string()];
    policy.fs_write_unsafe_ack = true;

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "echo".to_string(),
            description: None,
        },
        run: tui_use::model::RunConfig {
            command: fixture,
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: tui_use::model::scenario::PolicyRef::Inline(policy),
        },
        steps: vec![
            Step {
                id: StepId::new(),
                name: "type".to_string(),
                action: Action {
                    action_type: ActionType::Text,
                    payload: serde_json::json!({"text": "hello"}),
                },
                assert: vec![Assertion {
                    assertion_type: "screen_contains".to_string(),
                    payload: serde_json::json!({"text": "hello"}),
                }],
                timeout_ms: 1000,
                retries: 0,
            },
            Step {
                id: StepId::new(),
                name: "terminate".to_string(),
                action: Action {
                    action_type: ActionType::Terminate,
                    payload: serde_json::json!({}),
                },
                assert: Vec::new(),
                timeout_ms: 1000,
                retries: 0,
            },
        ],
    };

    let scenario_path = dir.join("scenario.json");
    write_scenario(&scenario_path, &scenario);

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "run",
            "--json",
            "--scenario",
            scenario_path.to_str().unwrap(),
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
            "--overwrite",
        ])
        .output()
        .unwrap();

    if !output.status.success() {
        panic!(
            "run failed: status={:?}\nstdout={}\nstderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(artifacts_dir.join("run.json").exists());
    assert!(artifacts_dir.join("policy.json").exists());
    assert!(artifacts_dir.join("scenario.json").exists());
    assert!(artifacts_dir.join("transcript.log").exists());
    assert!(artifacts_dir.join("snapshots/000001.json").exists());
    assert!(artifacts_dir.join("events.jsonl").exists());
    assert!(artifacts_dir.join("normalization.json").exists());
    assert!(artifacts_dir.join("checksums.json").exists());
    let normalization: tui_use::model::NormalizationRecord = serde_json::from_str(
        &fs::read_to_string(artifacts_dir.join("normalization.json")).unwrap(),
    )
    .unwrap();
    assert!(normalization.filters.is_empty());
    assert!(!normalization.strict);
    assert_eq!(
        normalization.source,
        tui_use::model::NormalizationSource::None
    );
}
