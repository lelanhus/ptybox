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
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use ptybox::model::policy::{
    EnvPolicy, ExecPolicy, FsPolicy, NetworkEnforcementAck, NetworkPolicy, Policy, ReplayPolicy,
    SandboxMode, POLICY_VERSION,
};
use ptybox::model::{Observation, RunResult, TerminalSize};
use ptybox::policy::PolicyExplanation;

fn temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    dir.push(format!("ptybox-cli-test-{prefix}-{stamp}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_policy(path: &Path, policy: &Policy) {
    let data = serde_json::to_vec_pretty(policy).unwrap();
    fs::write(path, data).unwrap();
}

fn read_events_transcript(dir: &Path) -> String {
    let data = fs::read_to_string(dir.join("events.jsonl")).unwrap();
    let mut transcript = String::new();
    for line in data.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let observation: Observation = serde_json::from_str(line).unwrap();
        if let Some(delta) = observation.transcript_delta {
            transcript.push_str(&delta);
        }
    }
    transcript
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
        budgets: Default::default(),
        artifacts: Default::default(),
        replay: ReplayPolicy::default(),
    }
}

#[test]
fn exec_json_outputs_run_result() {
    let dir = temp_dir("exec-json");
    let policy_path = dir.join("policy.json");
    let policy = base_policy(&dir, vec!["/bin/echo".to_string()]);
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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

    assert!(output.status.success());
    let run: RunResult = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(run.command, "/bin/echo");
    assert!(run.exit_status.is_some());
}

#[test]
fn exec_json_stdout_contains_only_json() {
    let dir = temp_dir("exec-json-stdout");
    let policy_path = dir.join("policy.json");
    let policy = base_policy(&dir, vec!["/bin/echo".to_string()]);
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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

    assert!(output.status.success());
    let run: RunResult = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(run.command, "/bin/echo");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("run completed"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("\"run_result_version\""));
}

#[test]
fn exec_nonzero_exit_returns_exit_code() {
    let dir = temp_dir("exec-nonzero");
    let policy_path = dir.join("policy.json");
    let policy = base_policy(&dir, vec!["/usr/bin/false".to_string()]);
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            "/usr/bin/false",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(6));
    let run: RunResult = serde_json::from_slice(&output.stdout).unwrap();
    assert!(matches!(run.status, ptybox::model::RunStatus::Failed));
    let err = run.error.expect("run should include error");
    assert_eq!(err.code, "E_PROCESS_EXIT");
}

#[test]
fn exec_json_error_returns_exit_code() {
    let dir = temp_dir("exec-deny");
    let policy_path = dir.join("policy.json");
    let policy = base_policy(&dir, Vec::new());
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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

    assert_eq!(output.status.code(), Some(2));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(err.code, "E_POLICY_DENIED");
}

#[test]
fn exec_denies_non_allowlisted_executable() {
    let dir = temp_dir("exec-deny-unlisted");
    let policy_path = dir.join("policy.json");
    let policy = base_policy(&dir, vec!["/bin/echo".to_string()]);
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            "/bin/ls",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(err.code, "E_POLICY_DENIED");
}

#[test]
fn exec_denies_shell_when_disabled() {
    let dir = temp_dir("exec-deny-shell");
    let policy_path = dir.join("policy.json");
    let policy = base_policy(&dir, vec!["/bin/sh".to_string()]);
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            "/bin/sh",
            "-c",
            "echo hello",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(err.code, "E_POLICY_DENIED");
}

#[test]
fn exec_filters_environment_by_allowlist() {
    let dir = temp_dir("exec-env-allowlist");
    let policy_path = dir.join("policy.json");
    let artifacts_dir = dir.join("artifacts");

    let mut policy = base_policy(&dir, vec!["/usr/bin/env".to_string()]);
    policy.env.allowlist = vec!["FOO".to_string()];
    policy.env.set.insert("FOO".to_string(), "BAR".to_string());
    policy.fs.allowed_write = vec![artifacts_dir.display().to_string()];
    policy.fs.write_ack = true;
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
            "--",
            "/usr/bin/env",
        ])
        .output()
        .unwrap();

    if !output.status.success() {
        panic!(
            "exec failed: status={:?}\nstdout={}\nstderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let transcript = read_events_transcript(&artifacts_dir);
    assert!(transcript.contains("FOO=BAR"));
    assert!(!transcript.contains("HOME="));
}

#[test]
fn exec_invalid_utf8_returns_terminal_parse_error_and_writes_artifacts() {
    let dir = temp_dir("exec-invalid-utf8");
    let policy_path = dir.join("policy.json");
    let artifacts_dir = dir.join("artifacts");
    let input_path = dir.join("invalid.bin");
    fs::write(&input_path, [0xff, 0xfe]).unwrap();

    let mut policy = base_policy(&dir, vec!["/bin/cat".to_string()]);
    policy.fs.allowed_write = vec![artifacts_dir.display().to_string()];
    policy.fs.write_ack = true;
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
            "--",
            "/bin/cat",
            input_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(7));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(err.code, "E_TERMINAL_PARSE");
    assert!(artifacts_dir.join("run.json").exists());
    assert!(artifacts_dir.join("policy.json").exists());
}

#[test]
fn exec_rejects_existing_artifacts_without_overwrite() {
    let dir = temp_dir("exec-artifacts-exists");
    let policy_path = dir.join("policy.json");
    let artifacts_dir = dir.join("artifacts");
    fs::create_dir_all(&artifacts_dir).unwrap();

    let mut policy = base_policy(&dir, vec!["/bin/echo".to_string()]);
    policy.fs.allowed_write = vec![artifacts_dir.display().to_string()];
    policy.fs.write_ack = true;
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
            "--",
            "/bin/echo",
            "hello",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(err.code, "E_POLICY_DENIED");
}

#[test]
fn explain_policy_outputs_json() {
    let dir = temp_dir("explain");
    let policy_path = dir.join("policy.json");
    let policy = base_policy(&dir, Vec::new());
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "exec",
            "--json",
            "--explain-policy",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            "/bin/echo",
            "hello",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let explanation: PolicyExplanation = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!explanation.allowed);
    assert!(!explanation.errors.is_empty());
}

#[test]
fn driver_protocol_version_mismatch_is_reported() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["driver", "--stdio", "--json", "--", "/bin/cat"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let stdin = child.stdin.as_mut().unwrap();
        let payload = r#"{"protocol_version":999,"action":{"type":"terminate","payload":{}}}"#;
        writeln!(stdin, "{payload}").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(err.code, "E_PROTOCOL_VERSION_MISMATCH");
    let context = err
        .context
        .expect("protocol mismatch should include context");
    assert_eq!(
        context
            .get("supported_version")
            .and_then(|value| value.as_u64())
            .unwrap_or_default(),
        ptybox::model::PROTOCOL_VERSION as u64
    );
    assert!(context.get("provided_version").is_some());
}

#[test]
fn driver_protocol_version_ok_returns_observation() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["driver", "--stdio", "--json", "--", "/bin/cat"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let stdin = child.stdin.as_mut().unwrap();
        let payload = serde_json::json!({
            "protocol_version": ptybox::model::PROTOCOL_VERSION,
            "action": {
                "type": "terminate",
                "payload": {}
            }
        });
        writeln!(stdin, "{}", payload).unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let observation: Observation = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        observation.protocol_version,
        ptybox::model::PROTOCOL_VERSION
    );
}

#[test]
fn artifacts_dir_requires_write_allowlist_in_cli() {
    let dir = temp_dir("artifacts-deny");
    let policy_path = dir.join("policy.json");
    let artifacts_dir = dir.join("artifacts");
    fs::create_dir_all(&artifacts_dir).unwrap();
    let policy = base_policy(&dir, vec!["/bin/echo".to_string()]);
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
            "--",
            "/bin/echo",
            "hello",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(err.code, "E_POLICY_DENIED");
}

#[test]
fn exec_uses_policy_artifacts_dir_when_enabled() {
    let dir = temp_dir("artifacts-policy");
    let policy_path = dir.join("policy.json");
    let artifacts_dir = dir.join("artifacts");
    fs::create_dir_all(&artifacts_dir).unwrap();
    let mut policy = base_policy(&dir, vec!["/bin/echo".to_string()]);
    policy.fs.allowed_write = vec![artifacts_dir.display().to_string()];
    policy.fs.write_ack = true;
    policy.artifacts.enabled = true;
    policy.artifacts.dir = Some(artifacts_dir.display().to_string());
    policy.artifacts.overwrite = true;
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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

    assert!(output.status.success());
    assert!(artifacts_dir.join("run.json").exists());
    assert!(artifacts_dir.join("policy.json").exists());
}

#[test]
fn artifacts_policy_requires_dir() {
    let dir = temp_dir("artifacts-policy-missing-dir");
    let policy_path = dir.join("policy.json");
    let mut policy = base_policy(&dir, vec!["/bin/echo".to_string()]);
    policy.artifacts.enabled = true;
    policy.artifacts.dir = None;
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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

    assert_eq!(output.status.code(), Some(2));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(err.code, "E_POLICY_DENIED");
}

#[test]
fn exec_network_requires_ack() {
    let dir = temp_dir("network-ack");
    let policy_path = dir.join("policy.json");
    let mut policy = base_policy(&dir, vec!["/bin/echo".to_string()]);
    policy.network_enforcement.unenforced_ack = false;
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--no-sandbox",
            "--ack-unsafe-sandbox",
            "--enable-network",
            "--",
            "/bin/echo",
            "hello",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(err.code, "E_POLICY_DENIED");
}

#[test]
fn exec_network_with_ack_is_allowed() {
    let dir = temp_dir("network-ack-ok");
    let policy_path = dir.join("policy.json");
    let policy = base_policy(&dir, vec!["/bin/echo".to_string()]);
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--no-sandbox",
            "--ack-unsafe-sandbox",
            "--enable-network",
            "--ack-unsafe-network",
            "--",
            "/bin/echo",
            "hello",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn exec_timeout_writes_artifacts() {
    let dir = temp_dir("exec-timeout-artifacts");
    let policy_path = dir.join("policy.json");
    let artifacts_dir = dir.join("artifacts");
    let mut policy = base_policy(&dir, vec!["/bin/sleep".to_string()]);
    policy.fs.allowed_write = vec![artifacts_dir.display().to_string()];
    policy.fs.write_ack = true;
    policy.budgets.max_runtime_ms = 50;
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
            "--",
            "/bin/sleep",
            "5",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(4));
    assert!(artifacts_dir.join("run.json").exists());
    assert!(artifacts_dir.join("policy.json").exists());
}

#[test]
fn exec_rejects_relative_cwd_flag() {
    let dir = temp_dir("exec-relative-cwd");
    let policy_path = dir.join("policy.json");
    let policy = base_policy(&dir, vec!["/bin/echo".to_string()]);
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--cwd",
            "relative",
            "--",
            "/bin/echo",
            "hello",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(12));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(err.code, "E_CLI_INVALID_ARG");
}

#[test]
fn strict_write_cli_requires_ack_for_artifacts() {
    let dir = temp_dir("strict-write-cli");
    let policy_path = dir.join("policy.json");
    let artifacts_dir = dir.join("artifacts");
    let mut policy = base_policy(&dir, vec!["/bin/echo".to_string()]);
    policy.fs.allowed_read = vec![dir.display().to_string()];
    policy.fs.allowed_write = Vec::new();
    policy.fs.write_ack = false;
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--strict-write",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
            "--",
            "/bin/echo",
            "hello",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(err.code, "E_POLICY_DENIED");
    assert!(err.message.contains("write access"));
}

#[test]
fn strict_write_cli_with_ack_allows_artifacts() {
    let dir = temp_dir("strict-write-cli-ack");
    let policy_path = dir.join("policy.json");
    let artifacts_dir = dir.join("artifacts");
    let mut policy = base_policy(&dir, vec!["/bin/echo".to_string()]);
    policy.fs.allowed_read = vec![dir.display().to_string()];
    policy.fs.allowed_write = vec![artifacts_dir.display().to_string()];
    policy.fs.write_ack = false;
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--strict-write",
            "--ack-unsafe-write",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
            "--",
            "/bin/echo",
            "hello",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(artifacts_dir.join("run.json").exists());
}

#[test]
fn driver_rejects_malformed_json() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args(["driver", "--stdio", "--json", "--", "/bin/cat"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, "{{not-json").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(err.code, "E_PROTOCOL");
}

#[test]
fn driver_strict_write_with_ack_allows_start() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "driver",
            "--stdio",
            "--json",
            "--strict-write",
            "--ack-unsafe-write",
            "--",
            "/bin/cat",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let stdin = child.stdin.as_mut().unwrap();
        let payload = serde_json::json!({
            "protocol_version": ptybox::model::PROTOCOL_VERSION,
            "action": {
                "type": "terminate",
                "payload": {}
            }
        });
        writeln!(stdin, "{}", payload).unwrap();
    }

    let output = child.wait_with_output().unwrap();
    let observation: Observation = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        observation.protocol_version,
        ptybox::model::PROTOCOL_VERSION
    );
}

#[test]
fn run_explain_policy_uses_scenario_config() {
    let dir = temp_dir("run-explain");
    let policy = base_policy(&dir, Vec::new());
    let scenario = ptybox::model::Scenario {
        scenario_version: 1,
        metadata: ptybox::model::ScenarioMetadata {
            name: "cli-explain".to_string(),
            description: None,
        },
        run: ptybox::model::RunConfig {
            command: "/bin/echo".to_string(),
            args: vec!["hello".to_string()],
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy)),
        },
        steps: Vec::new(),
    };
    let scenario_path = dir.join("scenario.json");
    fs::write(
        &scenario_path,
        serde_json::to_vec_pretty(&scenario).unwrap(),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "run",
            "--json",
            "--explain-policy",
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let explanation: PolicyExplanation = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!explanation.allowed);
}
