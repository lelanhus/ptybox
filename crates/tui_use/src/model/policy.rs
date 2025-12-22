use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Policy {
    pub policy_version: u32,
    pub sandbox: SandboxMode,
    pub network: NetworkPolicy,
    pub fs: FsPolicy,
    pub exec: ExecPolicy,
    pub env: EnvPolicy,
    pub budgets: Budgets,
    pub artifacts: ArtifactsPolicy,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            policy_version: 1,
            sandbox: SandboxMode::Seatbelt,
            network: NetworkPolicy::Disabled,
            fs: FsPolicy::default(),
            exec: ExecPolicy::default(),
            env: EnvPolicy::default(),
            budgets: Budgets::default(),
            artifacts: ArtifactsPolicy::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxMode {
    Seatbelt,
    None,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPolicy {
    Disabled,
    Enabled,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct FsPolicy {
    pub allowed_read: Vec<String>,
    pub allowed_write: Vec<String>,
    pub working_dir: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ExecPolicy {
    pub allowed_executables: Vec<String>,
    pub allow_shell: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct EnvPolicy {
    pub allowlist: Vec<String>,
    pub set: BTreeMap<String, String>,
    pub inherit: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Budgets {
    pub max_runtime_ms: u64,
    pub max_steps: u64,
    pub max_output_bytes: u64,
    pub max_snapshot_bytes: u64,
    pub max_wait_ms: u64,
}

impl Default for Budgets {
    fn default() -> Self {
        Self {
            max_runtime_ms: 60_000,
            max_steps: 10_000,
            max_output_bytes: 8 * 1024 * 1024,
            max_snapshot_bytes: 2 * 1024 * 1024,
            max_wait_ms: 10_000,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ArtifactsPolicy {
    pub enabled: bool,
    pub dir: Option<String>,
    pub overwrite: bool,
}
