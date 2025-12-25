use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::model::NormalizationFilter;

/// Current policy format version.
pub const POLICY_VERSION: u32 = 3;

/// Deny-by-default security policy for TUI execution.
///
/// All privileges must be explicitly granted. Use `Policy::default()` for maximum
/// security with sandbox enabled, network disabled, and minimal filesystem access.
///
/// # Safety Acknowledgements
/// Dangerous configurations require explicit acknowledgement flags:
/// - `sandbox_unsafe_ack` - required when `sandbox: none`
/// - `network_unsafe_ack` - required when `network: enabled` or `sandbox: none`
/// - `fs_write_unsafe_ack` - required when `allowed_write` is non-empty
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Policy {
    /// Policy format version for compatibility checking.
    pub policy_version: u32,
    /// Sandbox isolation mode.
    pub sandbox: SandboxMode,
    /// Acknowledgement for running without sandbox (required when sandbox is none).
    #[serde(default)]
    pub sandbox_unsafe_ack: bool,
    /// Network access policy.
    pub network: NetworkPolicy,
    /// Acknowledgement for network access (required when enabled or sandbox is none).
    #[serde(default)]
    pub network_unsafe_ack: bool,
    /// Filesystem access policy.
    pub fs: FsPolicy,
    /// Acknowledgement for write access (required when `allowed_write` is non-empty).
    #[serde(default)]
    pub fs_write_unsafe_ack: bool,
    /// Require write acknowledgement for any write access including artifacts.
    #[serde(default)]
    pub fs_strict_write: bool,
    /// Executable allowlist policy.
    pub exec: ExecPolicy,
    /// Environment variable policy.
    pub env: EnvPolicy,
    /// Resource budgets for timeouts and limits.
    pub budgets: Budgets,
    /// Artifact collection configuration.
    pub artifacts: ArtifactsPolicy,
    /// Replay comparison policy.
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

/// Sandbox isolation mode.
///
/// - `Seatbelt`: Default on macOS, uses `sandbox-exec` for process isolation
/// - `None`: No sandboxing, requires explicit `sandbox_unsafe_ack: true`
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxMode {
    /// macOS Seatbelt sandbox (default).
    Seatbelt,
    /// No sandboxing (requires acknowledgement).
    None,
}

/// Network access policy.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPolicy {
    /// Network access disabled (default).
    Disabled,
    /// Network access enabled (requires acknowledgement).
    Enabled,
}

/// Filesystem access policy with path allowlists.
///
/// All paths must be absolute. Broad paths like `/`, home directories,
/// and system roots are rejected with `E_POLICY_DENIED`.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct FsPolicy {
    /// Paths allowed for read access.
    pub allowed_read: Vec<String>,
    /// Paths allowed for write access (requires `fs_write_unsafe_ack`).
    pub allowed_write: Vec<String>,
    /// Working directory for command execution.
    pub working_dir: Option<String>,
}

/// Executable allowlist policy.
///
/// Commands must be in `allowed_executables` to run.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ExecPolicy {
    /// Absolute paths to allowed executables.
    pub allowed_executables: Vec<String>,
    /// Allow shell execution (e.g., `sh -c`). Default false.
    pub allow_shell: bool,
}

/// Environment variable policy.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct EnvPolicy {
    /// Environment variable names to allow through.
    pub allowlist: Vec<String>,
    /// Explicit environment variable values to set.
    pub set: BTreeMap<String, String>,
    /// Inherit environment from parent (filtered by allowlist).
    pub inherit: bool,
}

/// Resource budgets for execution limits.
///
/// Exceeding any budget results in `E_TIMEOUT` error.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Budgets {
    /// Maximum total runtime in milliseconds.
    pub max_runtime_ms: u64,
    /// Maximum number of scenario steps.
    pub max_steps: u64,
    /// Maximum combined transcript + terminal output bytes.
    pub max_output_bytes: u64,
    /// Maximum size of a single snapshot in bytes.
    pub max_snapshot_bytes: u64,
    /// Maximum wait time per wait action in milliseconds.
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

/// Artifact collection configuration.
///
/// When enabled, snapshots, transcripts, and run results are written to disk.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ArtifactsPolicy {
    /// Enable artifact collection.
    pub enabled: bool,
    /// Directory for artifacts (must be in `allowed_write`).
    pub dir: Option<String>,
    /// Overwrite existing artifacts directory.
    pub overwrite: bool,
}

/// Replay comparison policy.
///
/// Controls how artifacts are compared during replay for regression testing.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ReplayPolicy {
    /// Disable normalization for exact comparison.
    #[serde(default)]
    pub strict: bool,
    /// Normalization filters to apply (e.g., ignore timestamps).
    #[serde(default)]
    pub normalization_filters: Option<Vec<NormalizationFilter>>,
    /// Regex-based normalization rules.
    #[serde(default)]
    pub normalization_rules: Option<Vec<crate::model::NormalizationRule>>,
}
