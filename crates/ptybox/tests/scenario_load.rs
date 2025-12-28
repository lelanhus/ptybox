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

use ptybox::model::policy::{
    ArtifactsPolicy, Budgets, EnvPolicy, ExecPolicy, FsPolicy, NetworkEnforcementAck,
    NetworkPolicy, Policy, ReplayPolicy, SandboxMode, POLICY_VERSION,
};
use ptybox::model::scenario::{
    Action, ActionType, PolicyRef, RunConfig, Scenario, ScenarioMetadata, Step,
};
use ptybox::model::{StepId, TerminalSize};
use ptybox::runner::ErrorCode;

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("ptybox-test-{name}.json"));
    path
}

fn build_scenario() -> Scenario {
    let temp_dir = std::env::temp_dir().display().to_string();
    let policy = Policy {
        policy_version: POLICY_VERSION,
        sandbox: SandboxMode::Disabled { ack: true },
        network: NetworkPolicy::Disabled,
        network_enforcement: NetworkEnforcementAck {
            unenforced_ack: true,
        },
        fs: FsPolicy {
            allowed_read: vec![temp_dir],
            allowed_write: vec![],
            working_dir: None,
            write_ack: false,
            strict_write: false,
        },
        exec: ExecPolicy {
            allowed_executables: vec!["/bin/echo".to_string()],
            allow_shell: false,
        },
        env: EnvPolicy {
            allowlist: vec![],
            set: Default::default(),
            inherit: false,
        },
        budgets: Budgets::default(),
        artifacts: ArtifactsPolicy::default(),
        replay: ReplayPolicy::default(),
    };

    Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "test-scenario".to_string(),
            description: None,
        },
        run: RunConfig {
            command: "/bin/echo".to_string(),
            args: vec!["hello".to_string()],
            cwd: None,
            initial_size: TerminalSize::default(),
            policy: PolicyRef::Inline(policy),
        },
        steps: vec![Step {
            id: StepId::new(),
            name: "type".to_string(),
            action: Action {
                action_type: ActionType::Text,
                payload: serde_json::json!({"text": "hello"}),
            },
            assert: vec![],
            timeout_ms: 100,
            retries: 0,
        }],
    }
}

#[test]
fn load_scenario_from_json_and_yaml() {
    let scenario = build_scenario();
    let json_path = temp_path("scenario");
    let yaml_path = std::env::temp_dir().join("ptybox-test-scenario.yaml");

    let json = serde_json::to_string(&scenario).unwrap();
    let yaml = serde_yml::to_string(&scenario).unwrap();

    fs::write(&json_path, json).unwrap();
    fs::write(&yaml_path, yaml).unwrap();

    let loaded_json = ptybox::scenario::load_scenario_file(json_path.to_str().unwrap()).unwrap();
    let loaded_yaml = ptybox::scenario::load_scenario_file(yaml_path.to_str().unwrap()).unwrap();

    assert_eq!(loaded_json.scenario_version, 1);
    assert_eq!(loaded_json.metadata.name, "test-scenario");
    assert_eq!(loaded_json.run.command, "/bin/echo");
    assert_eq!(loaded_json.steps.len(), 1);

    assert_eq!(loaded_yaml.scenario_version, 1);
    assert_eq!(loaded_yaml.metadata.name, "test-scenario");
    assert_eq!(loaded_yaml.run.command, "/bin/echo");
    assert_eq!(loaded_yaml.steps.len(), 1);

    let _ = fs::remove_file(json_path);
    let _ = fs::remove_file(yaml_path);
}

#[test]
fn load_scenario_file_not_found() {
    let result = ptybox::scenario::load_scenario_file("/nonexistent/path/scenario.json");
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::Io);
    assert!(err.message.contains("failed to read"));
}

#[test]
fn load_scenario_malformed_json() {
    let path = temp_path("malformed-scenario");
    fs::write(&path, "{ invalid json }").unwrap();

    let result = ptybox::scenario::load_scenario_file(path.to_str().unwrap());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::Io);
    assert!(err.message.contains("parse"));

    let _ = fs::remove_file(path);
}

#[test]
fn load_scenario_malformed_yaml() {
    let path = std::env::temp_dir().join("ptybox-test-malformed.yaml");
    // Invalid YAML with bad indentation
    fs::write(&path, "foo:\n  bar\n baz: invalid").unwrap();

    let result = ptybox::scenario::load_scenario_file(path.to_str().unwrap());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::Io);

    let _ = fs::remove_file(path);
}

#[test]
fn load_scenario_missing_required_fields() {
    let path = temp_path("incomplete-scenario");
    // Missing required fields like run, steps
    fs::write(&path, r#"{"scenario_version": 1}"#).unwrap();

    let result = ptybox::scenario::load_scenario_file(path.to_str().unwrap());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::Io);

    let _ = fs::remove_file(path);
}

#[test]
fn load_policy_ref_inline() {
    let temp_dir = std::env::temp_dir().display().to_string();
    let policy = Policy {
        policy_version: POLICY_VERSION,
        sandbox: SandboxMode::Disabled { ack: true },
        network: NetworkPolicy::Disabled,
        network_enforcement: NetworkEnforcementAck {
            unenforced_ack: true,
        },
        fs: FsPolicy {
            allowed_read: vec![temp_dir],
            allowed_write: vec![],
            working_dir: None,
            write_ack: false,
            strict_write: false,
        },
        exec: ExecPolicy {
            allowed_executables: vec!["/bin/echo".to_string()],
            allow_shell: false,
        },
        env: EnvPolicy::default(),
        budgets: Budgets::default(),
        artifacts: ArtifactsPolicy::default(),
        replay: ReplayPolicy::default(),
    };

    let policy_ref = PolicyRef::Inline(policy.clone());
    let loaded = ptybox::scenario::load_policy_ref(&policy_ref).unwrap();

    assert_eq!(loaded.policy_version, policy.policy_version);
    assert_eq!(
        loaded.exec.allowed_executables,
        policy.exec.allowed_executables
    );
}

#[test]
fn load_policy_ref_file() {
    let temp_dir = std::env::temp_dir().display().to_string();
    let policy = Policy {
        policy_version: POLICY_VERSION,
        sandbox: SandboxMode::Disabled { ack: true },
        network: NetworkPolicy::Disabled,
        network_enforcement: NetworkEnforcementAck {
            unenforced_ack: true,
        },
        fs: FsPolicy {
            allowed_read: vec![temp_dir],
            allowed_write: vec![],
            working_dir: None,
            write_ack: false,
            strict_write: false,
        },
        exec: ExecPolicy {
            allowed_executables: vec!["/usr/bin/cat".to_string()],
            allow_shell: false,
        },
        env: EnvPolicy::default(),
        budgets: Budgets::default(),
        artifacts: ArtifactsPolicy::default(),
        replay: ReplayPolicy::default(),
    };

    let path = temp_path("policy-ref-file");
    let json = serde_json::to_string(&policy).unwrap();
    fs::write(&path, json).unwrap();

    let policy_ref = PolicyRef::File {
        path: path.to_str().unwrap().to_string(),
    };
    let loaded = ptybox::scenario::load_policy_ref(&policy_ref).unwrap();

    assert_eq!(loaded.policy_version, POLICY_VERSION);
    assert_eq!(
        loaded.exec.allowed_executables,
        vec!["/usr/bin/cat".to_string()]
    );

    let _ = fs::remove_file(path);
}

#[test]
fn load_policy_ref_file_not_found() {
    let policy_ref = PolicyRef::File {
        path: "/nonexistent/policy.json".to_string(),
    };
    let result = ptybox::scenario::load_policy_ref(&policy_ref);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::Io);
}

#[test]
fn load_policy_ref_file_invalid_json() {
    let path = temp_path("invalid-policy");
    fs::write(&path, "not valid json").unwrap();

    let policy_ref = PolicyRef::File {
        path: path.to_str().unwrap().to_string(),
    };
    let result = ptybox::scenario::load_policy_ref(&policy_ref);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::Io);

    let _ = fs::remove_file(path);
}

#[test]
fn load_policy_file_success() {
    let temp_dir = std::env::temp_dir().display().to_string();
    let policy = Policy {
        policy_version: POLICY_VERSION,
        sandbox: SandboxMode::Disabled { ack: true },
        network: NetworkPolicy::Disabled,
        network_enforcement: NetworkEnforcementAck {
            unenforced_ack: true,
        },
        fs: FsPolicy {
            allowed_read: vec![temp_dir],
            allowed_write: vec![],
            working_dir: None,
            write_ack: false,
            strict_write: false,
        },
        exec: ExecPolicy {
            allowed_executables: vec!["/bin/ls".to_string()],
            allow_shell: false,
        },
        env: EnvPolicy::default(),
        budgets: Budgets::default(),
        artifacts: ArtifactsPolicy::default(),
        replay: ReplayPolicy::default(),
    };

    let path = temp_path("policy-file-test");
    let json = serde_json::to_string(&policy).unwrap();
    fs::write(&path, json).unwrap();

    let loaded = ptybox::scenario::load_policy_file(&path).unwrap();
    assert_eq!(loaded.exec.allowed_executables, vec!["/bin/ls".to_string()]);

    let _ = fs::remove_file(path);
}

#[test]
fn load_policy_file_not_found() {
    let path = PathBuf::from("/nonexistent/policy.json");
    let result = ptybox::scenario::load_policy_file(&path);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code, ErrorCode::Io);
}

#[test]
fn to_json_value_serialization() {
    #[derive(serde::Serialize)]
    struct TestData {
        name: String,
        count: u32,
    }

    let data = TestData {
        name: "test".to_string(),
        count: 42,
    };

    let value = ptybox::scenario::to_json_value(&data).unwrap();
    assert_eq!(value["name"], "test");
    assert_eq!(value["count"], 42);
}

#[test]
fn scenario_yaml_with_inline_policy() {
    let yaml_content = r#"
scenario_version: 1
metadata:
  name: yaml-test
run:
  command: /bin/echo
  args: ["hello"]
  initial_size:
    cols: 80
    rows: 24
  policy:
    policy_version: 4
    sandbox: none
    sandbox_unsafe_ack: true
    network: disabled
    network_enforcement:
      unenforced_ack: true
    fs:
      allowed_read: []
      allowed_write: []
    exec:
      allowed_executables:
        - /bin/echo
      allow_shell: false
    env:
      allowlist: []
      set: {}
      inherit: false
    budgets:
      max_runtime_ms: 60000
      max_steps: 1000
      max_output_bytes: 1048576
      max_snapshot_bytes: 1048576
      max_wait_ms: 5000
    artifacts:
      enabled: false
      overwrite: false
steps: []
"#;

    let path = std::env::temp_dir().join("ptybox-test-yaml-inline.yaml");
    fs::write(&path, yaml_content).unwrap();

    let scenario = ptybox::scenario::load_scenario_file(path.to_str().unwrap()).unwrap();
    assert_eq!(scenario.metadata.name, "yaml-test");
    assert_eq!(scenario.run.command, "/bin/echo");

    let _ = fs::remove_file(path);
}

#[test]
fn scenario_with_file_policy_ref() {
    // First create a policy file
    let temp_dir = std::env::temp_dir().display().to_string();
    let policy = Policy {
        policy_version: POLICY_VERSION,
        sandbox: SandboxMode::Disabled { ack: true },
        network: NetworkPolicy::Disabled,
        network_enforcement: NetworkEnforcementAck {
            unenforced_ack: true,
        },
        fs: FsPolicy {
            allowed_read: vec![temp_dir],
            allowed_write: vec![],
            working_dir: None,
            write_ack: false,
            strict_write: false,
        },
        exec: ExecPolicy {
            allowed_executables: vec!["/bin/cat".to_string()],
            allow_shell: false,
        },
        env: EnvPolicy::default(),
        budgets: Budgets::default(),
        artifacts: ArtifactsPolicy::default(),
        replay: ReplayPolicy::default(),
    };

    let policy_path = temp_path("external-policy");
    let policy_json = serde_json::to_string(&policy).unwrap();
    fs::write(&policy_path, policy_json).unwrap();

    // Create scenario referencing the policy file
    let scenario = Scenario {
        scenario_version: 1,
        metadata: ScenarioMetadata {
            name: "file-ref-test".to_string(),
            description: None,
        },
        run: RunConfig {
            command: "/bin/cat".to_string(),
            args: vec![],
            cwd: None,
            initial_size: TerminalSize::default(),
            policy: PolicyRef::File {
                path: policy_path.to_str().unwrap().to_string(),
            },
        },
        steps: vec![],
    };

    let scenario_path = temp_path("scenario-with-file-ref");
    let scenario_json = serde_json::to_string(&scenario).unwrap();
    fs::write(&scenario_path, scenario_json).unwrap();

    // Load and verify
    let loaded = ptybox::scenario::load_scenario_file(scenario_path.to_str().unwrap()).unwrap();
    assert_eq!(loaded.metadata.name, "file-ref-test");

    // Load the policy reference
    let loaded_policy = ptybox::scenario::load_policy_ref(&loaded.run.policy).unwrap();
    assert_eq!(
        loaded_policy.exec.allowed_executables,
        vec!["/bin/cat".to_string()]
    );

    let _ = fs::remove_file(policy_path);
    let _ = fs::remove_file(scenario_path);
}
