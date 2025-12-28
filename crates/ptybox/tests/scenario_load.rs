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
