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
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tui_use::model::policy::{
    Budgets, EnvPolicy, ExecPolicy, FsPolicy, NetworkEnforcementAck, NetworkPolicy, Policy,
    ReplayPolicy, SandboxMode, POLICY_VERSION,
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
    dir.push(format!("tui-use-cli-budget-{prefix}-{stamp}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_policy(path: &Path, policy: &Policy) {
    fs::write(path, serde_json::to_vec_pretty(policy).unwrap()).unwrap();
}

fn process_is_running(pid: u32) -> bool {
    Command::new("/bin/kill")
        .arg("-0")
        .arg(pid.to_string())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn wait_for_process_exit(pid: u32, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if !process_is_running(pid) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn wait_for_file(path: &Path, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if path.exists() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn base_policy(work_dir: &Path, allowed_exec: Vec<String>) -> Policy {
    Policy {
        policy_version: POLICY_VERSION,
        sandbox: SandboxMode::Disabled { ack: true },
        network: NetworkPolicy::Disabled,
        network_enforcement: NetworkEnforcementAck {
            unenforced_ack: true,
        },
        fs: FsPolicy {
            allowed_read: vec![work_dir.display().to_string()],
            allowed_write: Vec::new(),
            working_dir: Some(work_dir.display().to_string()),
            write_ack: false,
            strict_write: false,
        },
        exec: ExecPolicy {
            allowed_executables: allowed_exec,
            allow_shell: false,
        },
        env: EnvPolicy {
            allowlist: Vec::new(),
            set: Default::default(),
            inherit: false,
        },
        budgets: Budgets::default(),
        artifacts: Default::default(),
        replay: ReplayPolicy::default(),
    }
}

fn assert_error_with_code(output: &std::process::Output, code: &str, exit_code: i32) {
    assert_eq!(output.status.code(), Some(exit_code));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    if value.get("code").is_some() {
        let err: tui_use::model::ErrorInfo = serde_json::from_value(value).unwrap();
        assert_eq!(err.code, code);
        return;
    }
    let run: tui_use::model::RunResult = serde_json::from_value(value).unwrap();
    let err = run.error.expect("run result should include error");
    assert_eq!(err.code, code);
}

#[test]
fn exec_runtime_budget_is_enforced() {
    let dir = temp_dir("runtime");
    let policy_path = dir.join("policy.json");
    let mut policy = base_policy(&dir, vec!["/bin/sleep".to_string()]);
    policy.budgets.max_runtime_ms = 50;
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            "/bin/sleep",
            "5",
        ])
        .output()
        .unwrap();

    assert_error_with_code(&output, "E_TIMEOUT", 4);
}

#[test]
fn exec_output_budget_is_enforced() {
    let dir = temp_dir("output");
    let policy_path = dir.join("policy.json");
    let mut policy = base_policy(&dir, vec!["/usr/bin/yes".to_string()]);
    policy.budgets.max_output_bytes = 200;
    policy.budgets.max_runtime_ms = 1000;
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            "/usr/bin/yes",
        ])
        .output()
        .unwrap();

    assert_error_with_code(&output, "E_TIMEOUT", 4);
}

#[test]
fn exec_snapshot_budget_is_enforced() {
    let dir = temp_dir("snapshot");
    let policy_path = dir.join("policy.json");
    let mut policy = base_policy(&dir, vec!["/bin/echo".to_string()]);
    policy.budgets.max_snapshot_bytes = 10;
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            "/bin/echo",
            "hello",
        ])
        .output()
        .unwrap();

    assert_error_with_code(&output, "E_TIMEOUT", 4);
}

#[test]
fn scenario_max_steps_is_enforced() {
    let dir = temp_dir("max-steps");
    let policy_path = dir.join("policy.json");
    let mut policy = base_policy(&dir, vec!["/bin/cat".to_string()]);
    policy.budgets.max_steps = 1;
    write_policy(&policy_path, &policy);

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "steps".to_string(),
            description: None,
        },
        run: tui_use::model::RunConfig {
            command: "/bin/cat".to_string(),
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: tui_use::model::scenario::PolicyRef::File {
                path: policy_path.display().to_string(),
            },
        },
        steps: vec![
            Step {
                id: StepId::new(),
                name: "one".to_string(),
                action: Action {
                    action_type: ActionType::Text,
                    payload: serde_json::json!({"text": "hello"}),
                },
                assert: vec![Assertion {
                    assertion_type: "screen_contains".to_string(),
                    payload: serde_json::json!({"text": "hello"}),
                }],
                timeout_ms: 100,
                retries: 0,
            },
            Step {
                id: StepId::new(),
                name: "two".to_string(),
                action: Action {
                    action_type: ActionType::Terminate,
                    payload: serde_json::json!({}),
                },
                assert: Vec::new(),
                timeout_ms: 100,
                retries: 0,
            },
        ],
    };
    let scenario_path = dir.join("scenario.json");
    fs::write(
        &scenario_path,
        serde_json::to_vec_pretty(&scenario).unwrap(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "run",
            "--json",
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    if output.status.success() {
        let run: RunResult = serde_json::from_slice(&output.stdout).unwrap();
        assert!(matches!(run.status, tui_use::model::RunStatus::Failed));
        assert!(run
            .error
            .as_ref()
            .is_some_and(|err| err.code == "E_TIMEOUT"));
    } else {
        assert_error_with_code(&output, "E_TIMEOUT", 4);
    }
}

#[test]
fn scenario_wait_budget_is_enforced() {
    let dir = temp_dir("wait");
    let policy_path = dir.join("policy.json");
    let mut policy = base_policy(&dir, vec!["/bin/cat".to_string()]);
    policy.budgets.max_wait_ms = 50;
    write_policy(&policy_path, &policy);

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "wait".to_string(),
            description: None,
        },
        run: tui_use::model::RunConfig {
            command: "/bin/cat".to_string(),
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: tui_use::model::scenario::PolicyRef::File {
                path: policy_path.display().to_string(),
            },
        },
        steps: vec![Step {
            id: StepId::new(),
            name: "wait".to_string(),
            action: Action {
                action_type: ActionType::Wait,
                payload: serde_json::json!({
                    "condition": {
                        "type": "screen_contains",
                        "payload": {"text": "never-here"}
                    }
                }),
            },
            assert: Vec::new(),
            timeout_ms: 1000,
            retries: 0,
        }],
    };
    let scenario_path = dir.join("scenario.json");
    fs::write(
        &scenario_path,
        serde_json::to_vec_pretty(&scenario).unwrap(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "run",
            "--json",
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    if output.status.success() {
        let run: RunResult = serde_json::from_slice(&output.stdout).unwrap();
        assert!(matches!(run.status, tui_use::model::RunStatus::Failed));
        assert!(run
            .error
            .as_ref()
            .is_some_and(|err| err.code == "E_TIMEOUT"));
    } else {
        assert_error_with_code(&output, "E_TIMEOUT", 4);
    }
}

#[test]
fn scenario_runtime_budget_is_enforced_after_steps_complete() {
    let dir = temp_dir("runtime-after-steps");
    let policy_path = dir.join("policy.json");
    let mut policy = base_policy(&dir, vec!["/usr/bin/yes".to_string()]);
    policy.budgets.max_runtime_ms = 300;
    policy.budgets.max_wait_ms = 100;
    write_policy(&policy_path, &policy);

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "runtime-after-steps".to_string(),
            description: None,
        },
        run: tui_use::model::RunConfig {
            command: "/usr/bin/yes".to_string(),
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: tui_use::model::scenario::PolicyRef::File {
                path: policy_path.display().to_string(),
            },
        },
        steps: vec![Step {
            id: StepId::new(),
            name: "wait-for-output".to_string(),
            action: Action {
                action_type: ActionType::Wait,
                payload: serde_json::json!({
                    "condition": {
                        "type": "screen_contains",
                        "payload": {"text": "y"}
                    }
                }),
            },
            assert: Vec::new(),
            timeout_ms: 50,
            retries: 0,
        }],
    };
    let scenario_path = dir.join("scenario.json");
    fs::write(
        &scenario_path,
        serde_json::to_vec_pretty(&scenario).unwrap(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "run",
            "--json",
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    if output.status.success() {
        let run: RunResult = serde_json::from_slice(&output.stdout).unwrap();
        assert!(matches!(run.status, tui_use::model::RunStatus::Failed));
        assert!(run
            .error
            .as_ref()
            .is_some_and(|err| err.code == "E_TIMEOUT"));
    } else {
        assert_error_with_code(&output, "E_TIMEOUT", 4);
    }
}

#[test]
fn exec_timeout_kills_process_group() {
    let dir = temp_dir("exec-timeout-kill");
    let policy_path = dir.join("policy.json");
    let pid_path = dir.join("child.pid");
    let mut policy = base_policy(&dir, vec!["/bin/bash".to_string()]);
    policy.exec.allow_shell = true;
    policy.budgets.max_runtime_ms = 300;
    policy
        .env
        .allowlist
        .push("TUI_USE_CHILD_PID_PATH".to_string());
    policy.env.set.insert(
        "TUI_USE_CHILD_PID_PATH".to_string(),
        pid_path.display().to_string(),
    );
    write_policy(&policy_path, &policy);
    let script = "sleep 10 & echo $! > \"$TUI_USE_CHILD_PID_PATH\"; wait";

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            "/bin/bash",
            "-c",
            script,
        ])
        .output()
        .unwrap();

    assert_error_with_code(&output, "E_TIMEOUT", 4);
    assert!(
        wait_for_file(&pid_path, Duration::from_millis(200)),
        "pid file should exist"
    );

    let pid: u32 = fs::read_to_string(&pid_path)
        .expect("pid file should be readable")
        .trim()
        .parse()
        .expect("pid should be valid");

    assert!(
        wait_for_process_exit(pid, Duration::from_millis(500)),
        "child process should be terminated"
    );
}

// =============================================================================
// Budget Boundary Tests
// =============================================================================

#[test]
fn scenario_max_steps_boundary_just_below_limit() {
    let dir = temp_dir("steps-below");
    let policy_path = dir.join("policy.json");
    let mut policy = base_policy(&dir, vec!["/bin/cat".to_string()]);
    policy.budgets.max_steps = 2; // Allow 2 steps
    write_policy(&policy_path, &policy);

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "steps-below".to_string(),
            description: None,
        },
        run: tui_use::model::RunConfig {
            command: "/bin/cat".to_string(),
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: tui_use::model::scenario::PolicyRef::File {
                path: policy_path.display().to_string(),
            },
        },
        steps: vec![Step {
            id: StepId::new(),
            name: "one".to_string(),
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
    fs::write(
        &scenario_path,
        serde_json::to_vec_pretty(&scenario).unwrap(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "run",
            "--json",
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // Should succeed - 1 step is below limit of 2
    let run: RunResult = serde_json::from_slice(&output.stdout).expect("Should parse RunResult");
    assert!(
        matches!(run.status, tui_use::model::RunStatus::Passed),
        "Should pass with 1 step when limit is 2"
    );
}

#[test]
fn scenario_max_steps_boundary_at_limit() {
    let dir = temp_dir("steps-at");
    let policy_path = dir.join("policy.json");
    let mut policy = base_policy(&dir, vec!["/bin/cat".to_string()]);
    policy.budgets.max_steps = 1; // Allow exactly 1 step
    write_policy(&policy_path, &policy);

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "steps-at".to_string(),
            description: None,
        },
        run: tui_use::model::RunConfig {
            command: "/bin/cat".to_string(),
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: tui_use::model::scenario::PolicyRef::File {
                path: policy_path.display().to_string(),
            },
        },
        steps: vec![Step {
            id: StepId::new(),
            name: "one".to_string(),
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
    fs::write(
        &scenario_path,
        serde_json::to_vec_pretty(&scenario).unwrap(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "run",
            "--json",
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // Should succeed - exactly at limit
    let run: RunResult = serde_json::from_slice(&output.stdout).expect("Should parse RunResult");
    assert!(
        matches!(run.status, tui_use::model::RunStatus::Passed),
        "Should pass with 1 step when limit is exactly 1"
    );
}

#[test]
fn exec_output_budget_boundary_just_below() {
    let dir = temp_dir("output-below");
    let policy_path = dir.join("policy.json");
    let mut policy = base_policy(&dir, vec!["/bin/echo".to_string()]);
    // "hello" + newline = 6 bytes, set budget to 100 to be safe
    policy.budgets.max_output_bytes = 100;
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            "/bin/echo",
            "hello",
        ])
        .output()
        .unwrap();

    // Should succeed - small output within budget
    let run: RunResult = serde_json::from_slice(&output.stdout).expect("Should parse RunResult");
    assert!(
        matches!(run.status, tui_use::model::RunStatus::Passed),
        "Should pass with small output"
    );
}

#[test]
fn terminal_resize_boundary_at_max() {
    let dir = temp_dir("resize-max");
    let policy_path = dir.join("policy.json");
    let policy = base_policy(&dir, vec!["/bin/cat".to_string()]);
    write_policy(&policy_path, &policy);

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "resize-max".to_string(),
            description: None,
        },
        run: tui_use::model::RunConfig {
            command: "/bin/cat".to_string(),
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: tui_use::model::scenario::PolicyRef::File {
                path: policy_path.display().to_string(),
            },
        },
        steps: vec![
            Step {
                id: StepId::new(),
                name: "resize-to-max".to_string(),
                action: Action {
                    action_type: ActionType::Resize,
                    payload: serde_json::json!({"rows": 500, "cols": 500}),
                },
                assert: Vec::new(),
                timeout_ms: 100,
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
                timeout_ms: 100,
                retries: 0,
            },
        ],
    };
    let scenario_path = dir.join("scenario.json");
    fs::write(
        &scenario_path,
        serde_json::to_vec_pretty(&scenario).unwrap(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "run",
            "--json",
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // Should succeed - 500x500 is at the max boundary
    let run: RunResult = serde_json::from_slice(&output.stdout).expect("Should parse RunResult");
    assert!(
        matches!(run.status, tui_use::model::RunStatus::Passed),
        "Should pass with max terminal size"
    );
}

#[test]
fn terminal_resize_boundary_exceeds_max() {
    let dir = temp_dir("resize-exceed");
    let policy_path = dir.join("policy.json");
    let policy = base_policy(&dir, vec!["/bin/cat".to_string()]);
    write_policy(&policy_path, &policy);

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "resize-exceed".to_string(),
            description: None,
        },
        run: tui_use::model::RunConfig {
            command: "/bin/cat".to_string(),
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: tui_use::model::scenario::PolicyRef::File {
                path: policy_path.display().to_string(),
            },
        },
        steps: vec![Step {
            id: StepId::new(),
            name: "resize-too-big".to_string(),
            action: Action {
                action_type: ActionType::Resize,
                payload: serde_json::json!({"rows": 501, "cols": 500}),
            },
            assert: Vec::new(),
            timeout_ms: 100,
            retries: 0,
        }],
    };
    let scenario_path = dir.join("scenario.json");
    fs::write(
        &scenario_path,
        serde_json::to_vec_pretty(&scenario).unwrap(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "run",
            "--json",
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // Should fail - 501 rows exceeds max of 500
    if output.status.success() {
        let run: RunResult = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            matches!(run.status, tui_use::model::RunStatus::Failed),
            "Should fail with oversized terminal"
        );
        assert!(run
            .error
            .as_ref()
            .is_some_and(|err| err.code == "E_PROTOCOL"));
    } else {
        assert_error_with_code(&output, "E_PROTOCOL", 9);
    }
}

#[test]
fn terminal_resize_boundary_zero_rejected() {
    let dir = temp_dir("resize-zero");
    let policy_path = dir.join("policy.json");
    let policy = base_policy(&dir, vec!["/bin/cat".to_string()]);
    write_policy(&policy_path, &policy);

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "resize-zero".to_string(),
            description: None,
        },
        run: tui_use::model::RunConfig {
            command: "/bin/cat".to_string(),
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: tui_use::model::scenario::PolicyRef::File {
                path: policy_path.display().to_string(),
            },
        },
        steps: vec![Step {
            id: StepId::new(),
            name: "resize-zero".to_string(),
            action: Action {
                action_type: ActionType::Resize,
                payload: serde_json::json!({"rows": 0, "cols": 80}),
            },
            assert: Vec::new(),
            timeout_ms: 100,
            retries: 0,
        }],
    };
    let scenario_path = dir.join("scenario.json");
    fs::write(
        &scenario_path,
        serde_json::to_vec_pretty(&scenario).unwrap(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_tui-use"))
        .args([
            "run",
            "--json",
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // Should fail - 0 rows is below minimum
    if output.status.success() {
        let run: RunResult = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            matches!(run.status, tui_use::model::RunStatus::Failed),
            "Should fail with zero rows"
        );
        assert!(run
            .error
            .as_ref()
            .is_some_and(|err| err.code == "E_PROTOCOL"));
    } else {
        assert_error_with_code(&output, "E_PROTOCOL", 9);
    }
}
