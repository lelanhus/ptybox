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

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use ptybox::model::policy::{
    EnvPolicy, ExecPolicy, FsPolicy, NetworkEnforcementAck, NetworkPolicy, Policy, ReplayPolicy,
    SandboxMode, POLICY_VERSION,
};
use ptybox::model::{
    Action, ActionType, Assertion, NormalizationRule, NormalizationRuleTarget, Scenario,
    ScenarioMetadata, Step, StepId, TerminalSize,
};

fn temp_dir(prefix: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    dir.push(format!("ptybox-cli-replay-{prefix}-{stamp}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_scenario(path: &Path, scenario: &Scenario) {
    fs::write(path, serde_json::to_vec_pretty(scenario).unwrap()).unwrap();
}

fn latest_replay_dir(artifacts_dir: &Path) -> PathBuf {
    let mut candidates: Vec<PathBuf> = fs::read_dir(artifacts_dir)
        .unwrap()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("replay-"))
        })
        .collect();
    candidates.sort();
    candidates
        .last()
        .cloned()
        .expect("expected replay directory to be created")
}

fn update_checksum(artifacts_dir: &Path, relative: &str) {
    let checksums_path = artifacts_dir.join("checksums.json");
    let mut checksums: BTreeMap<String, String> =
        serde_json::from_str(&fs::read_to_string(&checksums_path).unwrap()).unwrap();
    let data = fs::read(artifacts_dir.join(relative)).unwrap();
    let checksum = format!("{:016x}", fnv1a_hash(&data));
    checksums.insert(relative.to_string(), checksum);
    fs::write(
        &checksums_path,
        serde_json::to_vec_pretty(&checksums).unwrap(),
    )
    .unwrap();
}

fn fnv1a_hash(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in data {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn base_policy(work_dir: &Path, artifacts_dir: &Path) -> Policy {
    Policy {
        policy_version: POLICY_VERSION,
        sandbox: SandboxMode::Disabled { ack: true },
        network: NetworkPolicy::Disabled,
        network_enforcement: NetworkEnforcementAck {
            unenforced_ack: true,
        },
        fs: FsPolicy {
            allowed_read: vec![work_dir.display().to_string()],
            allowed_write: vec![artifacts_dir.display().to_string()],
            working_dir: Some(work_dir.display().to_string()),
            write_ack: true,
            strict_write: false,
        },
        exec: ExecPolicy {
            allowed_executables: vec!["/bin/cat".to_string()],
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

fn build_scenario(dir: &Path, policy: Policy) -> Scenario {
    Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "replay".to_string(),
            description: None,
        },
        run: ptybox::model::RunConfig {
            command: "/bin/cat".to_string(),
            args: Vec::new(),
            cwd: Some(dir.display().to_string()),
            initial_size: TerminalSize::default(),
            policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy)),
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
    }
}

#[test]
fn replay_succeeds_for_matching_snapshots() {
    let dir = temp_dir("ok");
    let artifacts_dir = dir.join("artifacts");
    let scenario_path = dir.join("scenario.json");
    let policy = base_policy(&dir, &artifacts_dir);
    let scenario = build_scenario(&dir, policy);
    write_scenario(&scenario_path, &scenario);

    let run_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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
    assert!(run_output.status.success());

    let replay_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "replay",
            "--json",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(replay_output.status.success());
}

#[test]
fn replay_detects_snapshot_mismatch() {
    let dir = temp_dir("mismatch");
    let artifacts_dir = dir.join("artifacts");
    let scenario_path = dir.join("scenario.json");
    let policy = base_policy(&dir, &artifacts_dir);
    let scenario = build_scenario(&dir, policy);
    write_scenario(&scenario_path, &scenario);

    let run_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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
    assert!(run_output.status.success());

    let snapshot_path = artifacts_dir.join("snapshots/000001.json");
    let mut snapshot =
        serde_json::from_str::<serde_json::Value>(&fs::read_to_string(&snapshot_path).unwrap())
            .unwrap();
    snapshot["lines"][0] = serde_json::Value::String("corrupt".to_string());
    fs::write(snapshot_path, serde_json::to_vec_pretty(&snapshot).unwrap()).unwrap();
    update_checksum(&artifacts_dir, "snapshots/000001.json");

    let replay_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "replay",
            "--json",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(replay_output.status.code(), Some(11));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&replay_output.stdout).unwrap();
    assert_eq!(err.code, "E_REPLAY_MISMATCH");
}

#[test]
fn replay_explain_uses_policy_defaults() {
    let dir = temp_dir("explain");
    let artifacts_dir = dir.join("artifacts");
    let scenario_path = dir.join("scenario.json");
    let mut policy = base_policy(&dir, &artifacts_dir);
    policy.replay.normalization_filters =
        Some(vec![ptybox::model::NormalizationFilter::SnapshotId]);
    let scenario = build_scenario(&dir, policy);
    write_scenario(&scenario_path, &scenario);

    let run_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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
    assert!(run_output.status.success());

    let replay_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "replay",
            "--json",
            "--explain",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(replay_output.status.success());
    let explanation: serde_json::Value = serde_json::from_slice(&replay_output.stdout).unwrap();
    assert_eq!(explanation["source"], "policy");
    assert_eq!(explanation["strict"], false);
    assert_eq!(explanation["filters"], serde_json::json!(["snapshot_id"]));
}

#[test]
fn replay_detects_transcript_mismatch() {
    let dir = temp_dir("transcript-mismatch");
    let artifacts_dir = dir.join("artifacts");
    let scenario_path = dir.join("scenario.json");
    let policy = base_policy(&dir, &artifacts_dir);
    let scenario = build_scenario(&dir, policy);
    write_scenario(&scenario_path, &scenario);

    let run_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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
    assert!(run_output.status.success());

    let transcript_path = artifacts_dir.join("transcript.log");
    fs::write(&transcript_path, "corrupt").unwrap();
    update_checksum(&artifacts_dir, "transcript.log");

    let replay_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "replay",
            "--json",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(replay_output.status.code(), Some(11));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&replay_output.stdout).unwrap();
    assert_eq!(err.code, "E_REPLAY_MISMATCH");
}

#[test]
fn replay_report_reads_latest_summary() {
    let dir = temp_dir("report");
    let artifacts_dir = dir.join("artifacts");
    let scenario_path = dir.join("scenario.json");
    let policy = base_policy(&dir, &artifacts_dir);
    let scenario = build_scenario(&dir, policy);
    write_scenario(&scenario_path, &scenario);

    let run_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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
    assert!(run_output.status.success());

    let replay_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "replay",
            "--json",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(replay_output.status.success());

    let report_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "replay-report",
            "--json",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(report_output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&report_output.stdout).unwrap();
    assert_eq!(report["replay"]["status"], "passed");
    assert!(report["dir"].as_str().unwrap().contains("replay-"));
}

#[test]
fn replay_detects_run_result_mismatch() {
    let dir = temp_dir("run-mismatch");
    let artifacts_dir = dir.join("artifacts");
    let scenario_path = dir.join("scenario.json");
    let policy = base_policy(&dir, &artifacts_dir);
    let scenario = build_scenario(&dir, policy);
    write_scenario(&scenario_path, &scenario);

    let run_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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
    assert!(run_output.status.success());

    let run_path = artifacts_dir.join("run.json");
    let mut run: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&run_path).unwrap()).unwrap();
    run["status"] = serde_json::Value::String("failed".to_string());
    fs::write(run_path, serde_json::to_vec_pretty(&run).unwrap()).unwrap();
    update_checksum(&artifacts_dir, "run.json");

    let replay_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "replay",
            "--json",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(replay_output.status.code(), Some(11));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&replay_output.stdout).unwrap();
    assert_eq!(err.code, "E_REPLAY_MISMATCH");
}

#[test]
fn replay_strict_mode_detects_snapshot_id_mismatch() {
    let dir = temp_dir("strict");
    let artifacts_dir = dir.join("artifacts");
    let scenario_path = dir.join("scenario.json");
    let policy = base_policy(&dir, &artifacts_dir);
    let scenario = build_scenario(&dir, policy);
    write_scenario(&scenario_path, &scenario);

    let run_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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
    assert!(run_output.status.success());

    let replay_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "replay",
            "--json",
            "--strict",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(replay_output.status.code(), Some(11));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&replay_output.stdout).unwrap();
    assert_eq!(err.code, "E_REPLAY_MISMATCH");
}

#[test]
fn replay_detects_event_stream_mismatch() {
    let dir = temp_dir("events-mismatch");
    let artifacts_dir = dir.join("artifacts");
    let scenario_path = dir.join("scenario.json");
    let policy = base_policy(&dir, &artifacts_dir);
    let scenario = build_scenario(&dir, policy);
    write_scenario(&scenario_path, &scenario);

    let run_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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
    assert!(run_output.status.success());

    let events_path = artifacts_dir.join("events.jsonl");
    let mut events = fs::read_to_string(&events_path).unwrap();
    events.push_str("{\"corrupt\":true}\n");
    fs::write(&events_path, events).unwrap();
    update_checksum(&artifacts_dir, "events.jsonl");

    let replay_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "replay",
            "--json",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(replay_output.status.code(), Some(11));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&replay_output.stdout).unwrap();
    assert_eq!(err.code, "E_REPLAY_MISMATCH");
}

#[test]
fn replay_normalize_subset_is_respected() {
    let dir = temp_dir("normalize-subset");
    let artifacts_dir = dir.join("artifacts");
    let scenario_path = dir.join("scenario.json");
    let policy = base_policy(&dir, &artifacts_dir);
    let scenario = build_scenario(&dir, policy);
    write_scenario(&scenario_path, &scenario);

    let run_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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
    assert!(run_output.status.success());

    let replay_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "replay",
            "--json",
            "--normalize",
            "snapshot_id",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(replay_output.status.code(), Some(11));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&replay_output.stdout).unwrap();
    assert_eq!(err.code, "E_REPLAY_MISMATCH");
}

#[test]
fn replay_normalize_all_is_accepted() {
    let dir = temp_dir("normalize-all");
    let artifacts_dir = dir.join("artifacts");
    let scenario_path = dir.join("scenario.json");
    let policy = base_policy(&dir, &artifacts_dir);
    let scenario = build_scenario(&dir, policy);
    write_scenario(&scenario_path, &scenario);

    let run_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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
    assert!(run_output.status.success());

    let replay_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "replay",
            "--json",
            "--normalize",
            "all",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(replay_output.status.success());
}

#[test]
fn replay_normalize_none_is_accepted() {
    let dir = temp_dir("normalize-none");
    let artifacts_dir = dir.join("artifacts");
    let scenario_path = dir.join("scenario.json");
    let policy = base_policy(&dir, &artifacts_dir);
    let scenario = build_scenario(&dir, policy);
    write_scenario(&scenario_path, &scenario);

    let run_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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
    assert!(run_output.status.success());

    let replay_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "replay",
            "--json",
            "--normalize",
            "none",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(replay_output.status.code(), Some(11));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&replay_output.stdout).unwrap();
    assert_eq!(err.code, "E_REPLAY_MISMATCH");
}

#[test]
fn replay_writes_normalization_record_with_source() {
    let dir = temp_dir("normalization-record");
    let artifacts_dir = dir.join("artifacts");
    let scenario_path = dir.join("scenario.json");
    let policy = base_policy(&dir, &artifacts_dir);
    let scenario = build_scenario(&dir, policy);
    write_scenario(&scenario_path, &scenario);

    let run_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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
    assert!(run_output.status.success());

    let replay_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "replay",
            "--json",
            "--normalize",
            "snapshot_id",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(replay_output.status.code(), Some(11));

    let replay_dir = latest_replay_dir(&artifacts_dir);
    let normalization: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(replay_dir.join("normalization.json")).unwrap())
            .unwrap();
    assert_eq!(normalization["source"], "cli");
    assert_eq!(normalization["strict"], false);
    assert_eq!(normalization["filters"], serde_json::json!(["snapshot_id"]));
}

#[test]
fn replay_writes_summary_on_mismatch() {
    let dir = temp_dir("replay-summary");
    let artifacts_dir = dir.join("artifacts");
    let scenario_path = dir.join("scenario.json");
    let policy = base_policy(&dir, &artifacts_dir);
    let scenario = build_scenario(&dir, policy);
    write_scenario(&scenario_path, &scenario);

    let run_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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
    assert!(run_output.status.success());

    let snapshot_path = artifacts_dir.join("snapshots/000001.json");
    let mut snapshot =
        serde_json::from_str::<serde_json::Value>(&fs::read_to_string(&snapshot_path).unwrap())
            .unwrap();
    snapshot["lines"][0] = serde_json::Value::String("corrupt".to_string());
    fs::write(snapshot_path, serde_json::to_vec_pretty(&snapshot).unwrap()).unwrap();
    update_checksum(&artifacts_dir, "snapshots/000001.json");

    let replay_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "replay",
            "--json",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(replay_output.status.code(), Some(11));

    let replay_dir = latest_replay_dir(&artifacts_dir);
    let summary: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(replay_dir.join("replay.json")).unwrap()).unwrap();
    assert_eq!(summary["status"], "failed");
    assert_eq!(summary["mismatch"]["kind"], "snapshot");
    let diff: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(replay_dir.join("diff.json")).unwrap()).unwrap();
    assert_eq!(diff["kind"], "snapshot");
}

#[test]
fn replay_rejects_conflicting_normalize_flags() {
    let dir = temp_dir("replay-normalize-conflict");
    let artifacts_dir = dir.join("artifacts");
    fs::create_dir_all(&artifacts_dir).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "replay",
            "--json",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
            "--normalize",
            "none",
            "--normalize",
            "snapshot_id",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(12));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(err.code, "E_CLI_INVALID_ARG");
}

#[test]
fn replay_policy_rules_allow_nondeterministic_output() {
    let dir = temp_dir("rules");
    let artifacts_dir = dir.join("artifacts");
    let scenario_path = dir.join("scenario.json");
    let fixture = "/bin/date".to_string();
    let mut policy = base_policy(&dir, &artifacts_dir);
    policy.exec.allowed_executables = vec![fixture.clone()];
    policy.replay.normalization_rules = Some(vec![
        NormalizationRule {
            target: NormalizationRuleTarget::Transcript,
            pattern: "\\d+".to_string(),
            replace: "<ts>".to_string(),
            terminated_by_harness: false,
        },
        NormalizationRule {
            target: NormalizationRuleTarget::SnapshotLines,
            pattern: "\\d+".to_string(),
            replace: "<ts>".to_string(),
            terminated_by_harness: false,
        },
    ]);
    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "rules".to_string(),
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
            name: "wait_exit".to_string(),
            action: Action {
                action_type: ActionType::Wait,
                payload: serde_json::json!({
                    "condition": {
                        "type": "process_exited",
                        "payload": {}
                    }
                }),
            },
            assert: Vec::new(),
            timeout_ms: 1000,
            retries: 0,
        }],
    };
    write_scenario(&scenario_path, &scenario);

    let run_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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
    assert!(run_output.status.success());

    let replay_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "replay",
            "--json",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(replay_output.status.success());
}

#[test]
fn replay_detects_checksum_mismatch() {
    let dir = temp_dir("checksum-mismatch");
    let artifacts_dir = dir.join("artifacts");
    let scenario_path = dir.join("scenario.json");
    let policy = base_policy(&dir, &artifacts_dir);
    let scenario = build_scenario(&dir, policy);
    write_scenario(&scenario_path, &scenario);

    let run_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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
    assert!(run_output.status.success());

    let transcript_path = artifacts_dir.join("transcript.log");
    fs::write(&transcript_path, "corrupt").unwrap();

    let replay_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "replay",
            "--json",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(replay_output.status.code(), Some(11));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&replay_output.stdout).unwrap();
    assert_eq!(err.code, "E_REPLAY_MISMATCH");
    let context = err.context.unwrap();
    assert_eq!(context["kind"], "checksum");
}

#[test]
fn replay_requires_events_when_flag_set() {
    let dir = temp_dir("require-events");
    let artifacts_dir = dir.join("artifacts");
    let scenario_path = dir.join("scenario.json");
    let policy = base_policy(&dir, &artifacts_dir);
    let scenario = build_scenario(&dir, policy);
    write_scenario(&scenario_path, &scenario);

    let run_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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
    assert!(run_output.status.success());

    fs::remove_file(artifacts_dir.join("events.jsonl")).unwrap();

    let replay_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "replay",
            "--json",
            "--require-events",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(replay_output.status.code(), Some(11));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&replay_output.stdout).unwrap();
    assert_eq!(err.code, "E_REPLAY_MISMATCH");
    let context = err.context.unwrap();
    assert_eq!(context["kind"], "events");
}

#[test]
fn replay_requires_checksums_when_flag_set() {
    let dir = temp_dir("require-checksums");
    let artifacts_dir = dir.join("artifacts");
    let scenario_path = dir.join("scenario.json");
    let policy = base_policy(&dir, &artifacts_dir);
    let scenario = build_scenario(&dir, policy);
    write_scenario(&scenario_path, &scenario);

    let run_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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
    assert!(run_output.status.success());

    fs::remove_file(artifacts_dir.join("checksums.json")).unwrap();

    let replay_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "replay",
            "--json",
            "--require-checksums",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(replay_output.status.code(), Some(11));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&replay_output.stdout).unwrap();
    assert_eq!(err.code, "E_REPLAY_MISMATCH");
    let context = err.context.unwrap();
    assert_eq!(context["kind"], "checksum");
}

#[test]
fn replay_policy_strict_is_respected() {
    let dir = temp_dir("policy-strict");
    let artifacts_dir = dir.join("artifacts");
    let scenario_path = dir.join("scenario.json");
    let mut policy = base_policy(&dir, &artifacts_dir);
    policy.replay.strict = true;
    let scenario = build_scenario(&dir, policy);
    write_scenario(&scenario_path, &scenario);

    let run_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
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
    assert!(run_output.status.success());

    let replay_output = Command::new(env!("CARGO_BIN_EXE_ptybox"))
        .args([
            "replay",
            "--json",
            "--artifacts",
            artifacts_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(replay_output.status.code(), Some(11));
    let err: ptybox::model::ErrorInfo = serde_json::from_slice(&replay_output.stdout).unwrap();
    assert_eq!(err.code, "E_REPLAY_MISMATCH");
}
