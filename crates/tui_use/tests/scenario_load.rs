use std::fs;
use std::path::PathBuf;

use tui_use::model::policy::{
    ArtifactsPolicy, Budgets, EnvPolicy, ExecPolicy, FsPolicy, NetworkPolicy, Policy, SandboxMode,
};
use tui_use::model::scenario::{
    Action, ActionType, PolicyRef, RunConfig, Scenario, ScenarioMetadata, Step,
};
use tui_use::model::{StepId, TerminalSize};

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("tui-use-test-{name}.json"));
    path
}

fn build_scenario() -> Scenario {
    let policy = Policy {
        policy_version: 1,
        sandbox: SandboxMode::None,
        network: NetworkPolicy::Disabled,
        fs: FsPolicy {
            allowed_read: vec!["/".to_string()],
            allowed_write: vec![],
            working_dir: None,
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
    let yaml_path = std::env::temp_dir().join("tui-use-test-scenario.yaml");

    let json = serde_json::to_string(&scenario).unwrap();
    let yaml = serde_yaml::to_string(&scenario).unwrap();

    fs::write(&json_path, json).unwrap();
    fs::write(&yaml_path, yaml).unwrap();

    let loaded_json = tui_use::scenario::load_scenario_file(json_path.to_str().unwrap()).unwrap();
    let loaded_yaml = tui_use::scenario::load_scenario_file(yaml_path.to_str().unwrap()).unwrap();

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
