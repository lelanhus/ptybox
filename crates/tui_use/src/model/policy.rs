use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::model::NormalizationFilter;

pub const POLICY_VERSION: u32 = 3;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Policy {
    pub policy_version: u32,
    pub sandbox: SandboxMode,
    #[serde(default)]
    pub sandbox_unsafe_ack: bool,
    pub network: NetworkPolicy,
    #[serde(default)]
    pub network_unsafe_ack: bool,
    pub fs: FsPolicy,
    #[serde(default)]
    pub fs_write_unsafe_ack: bool,
    #[serde(default)]
    pub fs_strict_write: bool,
    pub exec: ExecPolicy,
    pub env: EnvPolicy,
    pub budgets: Budgets,
    pub artifacts: ArtifactsPolicy,
    #[serde(default)]
    pub replay: ReplayPolicy,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            policy_version: POLICY_VERSION,
            sandbox: SandboxMode::Seatbelt,
            sandbox_unsafe_ack: false,
            network: NetworkPolicy::Disabled,
            network_unsafe_ack: false,
            fs: FsPolicy::default(),
            fs_write_unsafe_ack: false,
            fs_strict_write: false,
            exec: ExecPolicy::default(),
            env: EnvPolicy::default(),
            budgets: Budgets::default(),
            artifacts: ArtifactsPolicy::default(),
            replay: ReplayPolicy::default(),
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

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ReplayPolicy {
    #[serde(default)]
    pub strict: bool,
    #[serde(default)]
    pub normalization_filters: Option<Vec<NormalizationFilter>>,
    #[serde(default)]
    pub normalization_rules: Option<Vec<crate::model::NormalizationRule>>,
}
