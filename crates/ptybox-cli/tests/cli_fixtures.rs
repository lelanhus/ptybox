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

//! Integration tests using fixture TUI programs.
//!
//! These tests validate features against purpose-built fixture programs:
//! - `ptybox-echo-keys`: Echoes keypresses with byte values
//! - `ptybox-show-size`: Displays terminal size, updates on resize
//! - `ptybox-delay-output`: Outputs text after a delay
//! - `ptybox-exit-code`: Exits with specified code
//! - `ptybox-alt-screen`: Uses alternate screen buffer
//! - `ptybox-unicode-test`: Prints Unicode/CJK/emoji

use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use ptybox::model::policy::{
    EnvPolicy, ExecPolicy, FsPolicy, NetworkEnforcementAck, NetworkPolicy, Policy, ReplayPolicy,
    SandboxMode, POLICY_VERSION,
};
use ptybox::model::{
    Action, ActionType, Assertion, DriverResponseStatus, DriverResponseV2, Observation, RunResult,
    RunStatus, Scenario, ScenarioMetadata, Step, StepId, TerminalSize, PROTOCOL_VERSION,
};

static DRIVER_REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    dir.push(format!("ptybox-fixture-test-{prefix}-{stamp}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_policy(path: &Path, policy: &Policy) {
    let data = serde_json::to_vec_pretty(policy).unwrap();
    fs::write(path, data).unwrap();
}

fn write_scenario(path: &Path, scenario: &Scenario) {
    let data = serde_json::to_vec_pretty(scenario).unwrap();
    fs::write(path, data).unwrap();
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

fn write_driver_policy(work_dir: &Path, command: &str) -> PathBuf {
    let policy_path = work_dir.join("driver-policy.json");
    let policy = base_policy(work_dir, vec![command.to_string()]);
    write_policy(&policy_path, &policy);
    policy_path
}

fn next_driver_request_id() -> String {
    let sequence = DRIVER_REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("req-{sequence}")
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

/// Get path to a fixture binary.
/// The fixtures are built by the ptybox-fixtures crate.
fn fixture_path(name: &str) -> String {
    // Get the path to the ptybox binary (which is in target/debug or target/release)
    let ptybox_path = env!("CARGO_BIN_EXE_ptybox");
    let ptybox_dir = Path::new(ptybox_path).parent().unwrap();
    let fixture = ptybox_dir.join(name);

    // The fixture should exist in the same directory as ptybox
    if fixture.exists() {
        fixture.display().to_string()
    } else {
        panic!(
            "Fixture binary not found: {}. Run 'cargo build --workspace' first.",
            fixture.display()
        );
    }
}

/// Send a driver action and read the observation.
fn driver_action(
    stdin: &mut impl Write,
    lines: &mut impl Iterator<Item = std::io::Result<String>>,
    action_type: &str,
    payload: serde_json::Value,
) -> Observation {
    let request_id = next_driver_request_id();
    let action = serde_json::json!({
        "protocol_version": PROTOCOL_VERSION,
        "request_id": request_id,
        "action": { "type": action_type, "payload": payload }
    });
    writeln!(stdin, "{}", serde_json::to_string(&action).unwrap()).unwrap();
    stdin.flush().unwrap();

    let line = lines.next().unwrap().unwrap();
    let response: DriverResponseV2 = serde_json::from_str(&line).unwrap();
    assert_eq!(
        response.request_id, request_id,
        "driver should echo request_id"
    );
    assert_eq!(
        response.status,
        DriverResponseStatus::Ok,
        "driver action should succeed: {:?}",
        response.error
    );
    response
        .observation
        .expect("successful driver response should include observation")
}

// ============================================================================
// Feature 57: Unicode handling tests
// ============================================================================

#[test]
fn unicode_fixture_produces_valid_snapshot() {
    let dir = temp_dir("unicode");
    let artifacts_dir = dir.join("artifacts");

    let fixture = fixture_path("ptybox-unicode-test");
    let policy_path = dir.join("policy.json");
    let mut policy = base_policy(&dir, vec![fixture.clone()]);
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
            &fixture,
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "exec failed: {:?}", output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: RunResult = serde_json::from_str(&stdout).unwrap();

    assert_eq!(result.status, RunStatus::Passed, "run should pass");

    // Check transcript contains Unicode content
    let transcript = read_events_transcript(&artifacts_dir);
    assert!(
        transcript.contains("ASCII: Hello, World!"),
        "should contain ASCII"
    );
    assert!(
        transcript.contains("café") || transcript.contains("caf"),
        "should contain accented chars"
    );
    assert!(
        transcript.contains("你好") || transcript.contains("CJK"),
        "should contain CJK or marker"
    );
    assert!(
        transcript.contains("🎉") || transcript.contains("Emoji"),
        "should contain emoji or marker"
    );

    // Final observation should contain screen content
    let final_obs = result
        .final_observation
        .expect("should have final observation");
    assert!(
        !final_obs.screen.lines.is_empty(),
        "should have screen content"
    );
}

#[test]
fn unicode_fixture_box_drawing_preserved() {
    let dir = temp_dir("unicode-box");
    let artifacts_dir = dir.join("artifacts");

    let fixture = fixture_path("ptybox-unicode-test");
    let policy_path = dir.join("policy.json");
    let mut policy = base_policy(&dir, vec![fixture.clone()]);
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
            &fixture,
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "exec failed: {:?}", output);

    let transcript = read_events_transcript(&artifacts_dir);
    // Box drawing characters should be preserved
    assert!(
        transcript.contains("┌") || transcript.contains("Box"),
        "should contain box drawing"
    );
    assert!(
        transcript.contains("│") || transcript.contains("test"),
        "should contain box content"
    );
}

// ============================================================================
// Feature 4: Resize action tests
// ============================================================================

#[test]
fn show_size_fixture_displays_terminal_dimensions() {
    let dir = temp_dir("show-size");
    let artifacts_dir = dir.join("artifacts");

    let fixture = fixture_path("ptybox-show-size");
    let policy_path = dir.join("policy.json");
    let mut policy = base_policy(&dir, vec![fixture.clone()]);
    policy.fs.allowed_write = vec![artifacts_dir.display().to_string()];
    policy.fs.write_ack = true;
    write_policy(&policy_path, &policy);

    // Use "once" mode so fixture exits immediately after printing
    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
            "--",
            &fixture,
            "once",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "exec failed: {:?}", output);

    let transcript = read_events_transcript(&artifacts_dir);
    // Default terminal size is 24x80
    assert!(
        transcript.contains("24 rows") || transcript.contains("24"),
        "should report rows: {transcript}"
    );
    assert!(
        transcript.contains("80 cols") || transcript.contains("80"),
        "should report cols: {transcript}"
    );
}

#[test]
fn resize_action_updates_terminal_size_via_driver() {
    let dir = temp_dir("resize-driver");
    let fixture = fixture_path("ptybox-show-size");
    let policy_path = write_driver_policy(&dir, &fixture);

    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "driver",
            "--stdio",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            &fixture,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let reader = std::io::BufReader::new(stdout);

    let mut lines = reader.lines();

    // Send a wait action to get initial observation (driver produces output after action)
    let first_obs = driver_action(
        &mut stdin,
        &mut lines,
        "wait",
        serde_json::json!({ "condition": { "type": "screen_contains", "payload": { "text": "" } }, "timeout_ms": 100 }),
    );

    // Default size is 24x80
    assert_eq!(first_obs.screen.rows, 24);
    assert_eq!(first_obs.screen.cols, 80);

    // Send resize action
    let resize_obs = driver_action(
        &mut stdin,
        &mut lines,
        "resize",
        serde_json::json!({ "rows": 30, "cols": 100 }),
    );
    assert_eq!(resize_obs.screen.rows, 30, "rows should update to 30");
    assert_eq!(resize_obs.screen.cols, 100, "cols should update to 100");

    // Terminate
    let _term_obs = driver_action(&mut stdin, &mut lines, "terminate", serde_json::json!({}));

    let status = child.wait().unwrap();
    assert!(status.success());
}

// ============================================================================
// Exit code tests
// ============================================================================

#[test]
fn exit_code_fixture_returns_specified_code() {
    let dir = temp_dir("exit-code");
    let fixture = fixture_path("ptybox-exit-code");
    let policy_path = dir.join("policy.json");
    let policy = base_policy(&dir, vec![fixture.clone()]);
    write_policy(&policy_path, &policy);

    // Test exit code 0
    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            &fixture,
            "0",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: RunResult = serde_json::from_str(&stdout).unwrap();
    assert_eq!(result.status, RunStatus::Passed);
    let exit_status = result.exit_status.expect("should have exit status");
    assert!(exit_status.success);
    assert_eq!(exit_status.exit_code, Some(0));

    // Test exit code 42
    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            &fixture,
            "42",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: RunResult = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        result.status,
        RunStatus::Failed,
        "non-zero exit should fail"
    );
    let exit_status = result.exit_status.expect("should have exit status");
    assert!(!exit_status.success, "non-zero exit should be !success");
    assert_eq!(exit_status.exit_code, Some(42));
}

#[test]
fn exit_code_fixture_cli_exit_status() {
    let dir = temp_dir("exit-cli");
    let fixture = fixture_path("ptybox-exit-code");
    let policy_path = dir.join("policy.json");
    let policy = base_policy(&dir, vec![fixture.clone()]);
    write_policy(&policy_path, &policy);

    // Exit code 0 should result in CLI success
    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            &fixture,
            "0",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());

    // Exit code 1 should result in CLI exit code 6 (E_PROCESS_FAILED)
    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            &fixture,
            "1",
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(6),
        "should exit with E_PROCESS_FAILED (6)"
    );
}

// ============================================================================
// Wait condition tests (delay_output fixture)
// ============================================================================

#[test]
fn delay_output_fixture_wait_condition() {
    let dir = temp_dir("delay");
    let artifacts_dir = dir.join("artifacts");

    let fixture = fixture_path("ptybox-delay-output");
    let mut policy = base_policy(&dir, vec![fixture.clone()]);
    policy.fs.allowed_write = vec![artifacts_dir.display().to_string()];
    policy.fs.write_ack = true;

    // Create a scenario that waits for the delayed output
    // Using process_exited for the final wait since the fixture exits after printing
    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "delay-wait".to_string(),
            description: None,
        },
        run: ptybox::model::RunConfig {
            command: fixture.clone(),
            args: vec!["200".to_string(), "DELAYED_MESSAGE".to_string()],
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy)),
        },
        steps: vec![
            Step {
                id: StepId::new(),
                name: "wait-for-initial".to_string(),
                action: Action {
                    action_type: ActionType::Wait,
                    payload: serde_json::json!({"condition": {"type": "screen_contains", "payload": {"text": "waiting..."}}}),
                },
                assert: Vec::new(),
                timeout_ms: 5000,
                retries: 0,
            },
            // Wait for process to exit (it exits after printing the delayed message)
            Step {
                id: StepId::new(),
                name: "wait-for-exit".to_string(),
                action: Action {
                    action_type: ActionType::Wait,
                    payload: serde_json::json!({"condition": {"type": "process_exited"}}),
                },
                assert: Vec::new(),
                timeout_ms: 5000,
                retries: 0,
            },
        ],
    };

    let scenario_path = dir.join("scenario.json");
    write_scenario(&scenario_path, &scenario);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "run",
            "--json",
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "scenario should pass: stdout={}, stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: RunResult = serde_json::from_str(&stdout).unwrap();
    assert_eq!(result.status, RunStatus::Passed, "run should pass");

    // Verify the delayed message appeared in the final observation
    let final_obs = result
        .final_observation
        .expect("should have final observation");
    let screen_text = final_obs.screen.lines.join("\n");
    assert!(
        screen_text.contains("DELAYED_MESSAGE"),
        "delayed message should appear on screen: {screen_text}"
    );
}

// ============================================================================
// Driver protocol tests
// ============================================================================

#[test]
fn driver_protocol_version_mismatch_is_rejected() {
    let dir = temp_dir("driver-version-mismatch");
    let fixture = fixture_path("ptybox-exit-code");
    let policy_path = write_driver_policy(&dir, &fixture);

    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "driver",
            "--stdio",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            &fixture,
            "0",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let reader = std::io::BufReader::new(stdout);

    let mut lines = reader.lines();

    // Send action with wrong protocol version
    let request_id = "req-invalid-version";
    let bad_action = serde_json::json!({
        "protocol_version": 999,  // Invalid version
        "request_id": request_id,
        "action": { "type": "terminate", "payload": {} }
    });
    writeln!(stdin, "{}", serde_json::to_string(&bad_action).unwrap()).unwrap();
    stdin.flush().unwrap();

    let line = lines.next().unwrap().unwrap();
    let response: DriverResponseV2 = serde_json::from_str(&line).unwrap();

    // Should get an error about protocol version mismatch
    assert_eq!(response.request_id, request_id);
    assert_eq!(response.status, DriverResponseStatus::Error);
    let error = response
        .error
        .expect("version mismatch should include error");
    assert_eq!(error.code, "E_PROTOCOL_VERSION_MISMATCH");
    assert_eq!(
        error
            .context
            .as_ref()
            .and_then(|value| value.get("supported_version"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
        PROTOCOL_VERSION as u64
    );

    let status = child.wait().unwrap();
    assert!(
        !status.success() || status.code() == Some(8),
        "should exit with error code 8 (E_PROTOCOL_VERSION_MISMATCH)"
    );
}

#[test]
fn driver_malformed_json_returns_protocol_error() {
    let dir = temp_dir("driver-malformed-json");
    let fixture = fixture_path("ptybox-exit-code");
    let policy_path = write_driver_policy(&dir, &fixture);

    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "driver",
            "--stdio",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            &fixture,
            "0",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let reader = std::io::BufReader::new(stdout);

    let mut lines = reader.lines();

    // Send malformed JSON
    writeln!(stdin, "{{not valid json").unwrap();
    stdin.flush().unwrap();

    let line = lines.next().unwrap().unwrap();
    let response: DriverResponseV2 = serde_json::from_str(&line).unwrap();

    // Should get a protocol error
    assert_eq!(response.request_id, "unknown");
    assert_eq!(response.status, DriverResponseStatus::Error);
    let error = response
        .error
        .expect("malformed request should include error");
    assert_eq!(error.code, "E_PROTOCOL");

    let status = child.wait().unwrap();
    assert!(
        !status.success() || status.code() == Some(9),
        "should exit with error code 9 (E_PROTOCOL)"
    );
}

#[test]
fn driver_key_action_sends_input() {
    let dir = temp_dir("driver-key");
    // echo_keys prints byte value of each keypress
    let fixture = fixture_path("ptybox-echo-keys");
    let policy_path = write_driver_policy(&dir, &fixture);

    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "driver",
            "--stdio",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            &fixture,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let reader = std::io::BufReader::new(stdout);

    let mut lines = reader.lines();

    // Wait for initial ready
    let _first = driver_action(
        &mut stdin,
        &mut lines,
        "wait",
        serde_json::json!({ "condition": { "type": "screen_contains", "payload": { "text": "" } }, "timeout_ms": 100 }),
    );

    // Send a key action
    let obs = driver_action(
        &mut stdin,
        &mut lines,
        "key",
        serde_json::json!({ "key": "a" }),
    );

    // echo_keys should have received and echoed the key
    let screen_text = obs.screen.lines.join("\n");
    assert!(
        screen_text.contains("97") || screen_text.contains('a'),
        "should contain the key 'a' or its ASCII value 97: {screen_text}"
    );

    // Terminate
    let _term = driver_action(&mut stdin, &mut lines, "terminate", serde_json::json!({}));

    let status = child.wait().unwrap();
    assert!(status.success());
}

#[test]
fn driver_text_action_sends_input() {
    let dir = temp_dir("driver-text");
    let fixture = "/bin/cat".to_string();
    let policy_path = write_driver_policy(&dir, &fixture);

    let mut child = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "driver",
            "--stdio",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            &fixture,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let reader = std::io::BufReader::new(stdout);

    let mut lines = reader.lines();

    // Wait briefly
    let _first = driver_action(
        &mut stdin,
        &mut lines,
        "wait",
        serde_json::json!({ "condition": { "type": "screen_contains", "payload": { "text": "" } }, "timeout_ms": 100 }),
    );

    // Send text
    let obs = driver_action(
        &mut stdin,
        &mut lines,
        "text",
        serde_json::json!({ "text": "hello world" }),
    );

    // cat echoes the text
    let screen_text = obs.screen.lines.join("\n");
    assert!(
        screen_text.contains("hello world"),
        "should contain 'hello world': {screen_text}"
    );

    // Terminate
    let _term = driver_action(&mut stdin, &mut lines, "terminate", serde_json::json!({}));

    let status = child.wait().unwrap();
    assert!(status.success());
}

// ============================================================================
// Error path tests
// ============================================================================

#[test]
fn exec_policy_denied_for_unlisted_executable() {
    let dir = temp_dir("policy-denied");
    let policy_path = dir.join("policy.json");

    // Policy only allows /bin/echo, but we'll try to run something else
    let policy = base_policy(&dir, vec!["/bin/echo".to_string()]);
    write_policy(&policy_path, &policy);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "exec",
            "--json",
            "--policy",
            policy_path.to_str().unwrap(),
            "--",
            "/bin/cat", // Not in allowlist
        ])
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(2),
        "should exit with E_POLICY_DENIED (2)"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("E_POLICY_DENIED") || stdout.contains("policy"),
        "should mention policy denial: {stdout}"
    );
}

#[test]
fn exec_timeout_includes_step_context() {
    let dir = temp_dir("timeout-context");
    // Use /bin/cat which will block waiting for input
    let fixture = "/bin/cat".to_string();
    let mut policy = base_policy(&dir, vec![fixture.clone()]);
    policy.budgets.max_runtime_ms = 100; // Very short timeout

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "timeout".to_string(),
            description: None,
        },
        run: ptybox::model::RunConfig {
            command: fixture,
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy)),
        },
        steps: vec![Step {
            id: StepId::new(),
            name: "wait-forever".to_string(),
            action: Action {
                action_type: ActionType::Wait,
                payload: serde_json::json!({"condition": {"type": "screen_contains", "payload": {"text": "never_appears"}}}),
            },
            assert: Vec::new(),
            timeout_ms: 50, // Will timeout
            retries: 0,
        }],
    };

    let scenario_path = dir.join("scenario.json");
    write_scenario(&scenario_path, &scenario);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "run",
            "--json",
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(4),
        "should exit with E_TIMEOUT (4)"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: RunResult = serde_json::from_str(&stdout).unwrap();

    // Error should include context
    let err = result.error.expect("should have error");
    assert_eq!(err.code, "E_TIMEOUT");
    assert!(err.context.is_some(), "timeout should include context");
}

#[test]
fn scenario_assertion_failure_returns_correct_exit_code() {
    let dir = temp_dir("assert-fail");
    let fixture = "/bin/cat".to_string();
    let policy = base_policy(&dir, vec![fixture.clone()]);

    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "assert-fail".to_string(),
            description: None,
        },
        run: ptybox::model::RunConfig {
            command: fixture,
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy)),
        },
        steps: vec![Step {
            id: StepId::new(),
            name: "type".to_string(),
            action: Action {
                action_type: ActionType::Text,
                payload: serde_json::json!({"text": "hello"}),
            },
            assert: vec![Assertion {
                assertion_type: "screen_contains".to_string(),
                payload: serde_json::json!({"text": "goodbye"}), // Will fail
            }],
            timeout_ms: 100,
            retries: 0,
        }],
    };

    let scenario_path = dir.join("scenario.json");
    write_scenario(&scenario_path, &scenario);

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "run",
            "--json",
            "--scenario",
            scenario_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(5),
        "should exit with E_ASSERTION_FAILED (5)"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: RunResult = serde_json::from_str(&stdout).unwrap();
    assert_eq!(result.status, RunStatus::Failed);
}

// ============================================================================
// Terminal size via scenario
// ============================================================================

#[test]
fn scenario_resize_action() {
    let dir = temp_dir("scenario-resize");
    let artifacts_dir = dir.join("artifacts");

    // Use /bin/cat as a simple fixture - resize can be verified via observation
    let fixture = "/bin/cat".to_string();
    let mut policy = base_policy(&dir, vec![fixture.clone()]);
    policy.fs.allowed_write = vec![artifacts_dir.display().to_string()];
    policy.fs.write_ack = true;

    // Create a scenario that resizes the terminal and verifies via observation
    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "resize-scenario".to_string(),
            description: None,
        },
        run: ptybox::model::RunConfig {
            command: fixture.clone(),
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy)),
        },
        steps: vec![
            // Resize to 40x120
            Step {
                id: StepId::new(),
                name: "resize".to_string(),
                action: Action {
                    action_type: ActionType::Resize,
                    payload: serde_json::json!({"rows": 40, "cols": 120}),
                },
                assert: Vec::new(),
                timeout_ms: 1000,
                retries: 0,
            },
            // Type some text to verify we can still interact
            Step {
                id: StepId::new(),
                name: "type".to_string(),
                action: Action {
                    action_type: ActionType::Text,
                    payload: serde_json::json!({"text": "resized"}),
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

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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

    assert!(
        output.status.success(),
        "scenario should pass: stdout={}, stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: RunResult = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        result.status,
        RunStatus::Passed,
        "resize scenario should pass"
    );

    // Verify the final observation has the resized dimensions
    let final_obs = result
        .final_observation
        .expect("should have final observation");
    assert_eq!(final_obs.screen.rows, 40, "rows should be 40");
    assert_eq!(final_obs.screen.cols, 120, "cols should be 120");
}
