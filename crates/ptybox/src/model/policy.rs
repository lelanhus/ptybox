use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::model::NormalizationFilter;

/// Current policy format version.
///
/// Version 4 introduces type-level safety acknowledgements where dangerous
/// configurations embed their acknowledgement directly in the type.
pub const POLICY_VERSION: u32 = 4;

// =============================================================================
// Core Policy Types with Embedded Acknowledgements
// =============================================================================

/// Sandbox isolation mode with embedded acknowledgement.
///
/// - `Seatbelt`: Default on macOS, uses `sandbox-exec` for process isolation
/// - `Disabled { ack }`: No sandboxing, requires `ack: true` to proceed
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SandboxMode {
    /// macOS Seatbelt sandbox (default).
    Seatbelt,
    /// No sandboxing (requires acknowledgement).
    Disabled {
        /// Explicit acknowledgement that no sandbox will be used.
        /// When true, acknowledges that filesystem and network policies
        /// cannot be enforced by the OS.
        ack: bool,
    },
}

impl Default for SandboxMode {
    fn default() -> Self {
        Self::Seatbelt
    }
}

/// Network access policy with embedded acknowledgement.
///
/// - `Disabled`: Network access disabled (default)
/// - `Enabled { ack }`: Network access enabled, requires `ack: true`
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetworkPolicy {
    /// Network access disabled (default).
    Disabled,
    /// Network access enabled (requires acknowledgement).
    Enabled {
        /// Explicit acknowledgement of network access security implications.
        ack: bool,
    },
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self::Disabled
    }
}

/// Acknowledgement for unenforced network policy.
///
/// When sandbox is disabled, network policy cannot be enforced by the OS.
/// This acknowledgement is separate from `NetworkPolicy::Enabled { ack }`
/// because it covers a different risk: policy *cannot* be enforced vs
/// policy *intentionally* allows network.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkEnforcementAck {
    /// Acknowledge that network restrictions cannot be enforced without sandbox.
    #[serde(default)]
    pub unenforced_ack: bool,
}

/// Filesystem access policy with path allowlists and embedded write acknowledgement.
///
/// All paths must be absolute. Broad paths like `/`, home directories,
/// and system roots are rejected with `E_POLICY_DENIED`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FsPolicy {
    /// Paths allowed for read access.
    pub allowed_read: Vec<String>,
    /// Paths allowed for write access.
    pub allowed_write: Vec<String>,
    /// Working directory for command execution.
    pub working_dir: Option<String>,
    /// Acknowledgement for write access.
    /// Required when `allowed_write` is non-empty.
    pub write_ack: bool,
    /// Require write acknowledgement for any write access including artifacts.
    pub strict_write: bool,
}

/// Deny-by-default security policy for TUI execution (v4).
///
/// All privileges must be explicitly granted. Use `Policy::default()` for maximum
/// security with sandbox enabled, network disabled, and minimal filesystem access.
///
/// # Type-Level Safety
/// Dangerous configurations embed their acknowledgements directly:
/// - `SandboxMode::Disabled { ack: true }` - required to disable sandbox
/// - `NetworkPolicy::Enabled { ack: true }` - required for network access
/// - `FsPolicy { write_ack: true, .. }` - required for write access
/// - `network_enforcement` - required when sandbox disabled (unenforced network)
#[derive(Clone, Debug)]
pub struct Policy {
    /// Policy format version for compatibility checking.
    pub policy_version: u32,
    /// Sandbox isolation mode (with embedded ack for disabled state).
    pub sandbox: SandboxMode,
    /// Network access policy (with embedded ack for enabled state).
    pub network: NetworkPolicy,
    /// Acknowledgement for unenforced network policy (when sandbox disabled).
    pub network_enforcement: NetworkEnforcementAck,
    /// Filesystem access policy (with embedded write ack).
    pub fs: FsPolicy,
    /// Executable allowlist policy.
    pub exec: ExecPolicy,
    /// Environment variable policy.
    pub env: EnvPolicy,
    /// Resource budgets for timeouts and limits.
    pub budgets: Budgets,
    /// Artifact collection configuration.
    pub artifacts: ArtifactsPolicy,
    /// Replay comparison policy.
    pub replay: ReplayPolicy,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            policy_version: POLICY_VERSION,
            sandbox: SandboxMode::Seatbelt,
            network: NetworkPolicy::Disabled,
            network_enforcement: NetworkEnforcementAck::default(),
            fs: FsPolicy::default(),
            exec: ExecPolicy::default(),
            env: EnvPolicy::default(),
            budgets: Budgets::default(),
            artifacts: ArtifactsPolicy::default(),
            replay: ReplayPolicy::default(),
        }
    }
}

// =============================================================================
// Legacy Serde Compatibility Layer
// =============================================================================

/// Legacy JSON representation for v3 compatibility.
///
/// The v3 format uses flat boolean acknowledgement fields at the Policy level.
/// This struct enables seamless deserialization of v3 JSON into v4 types.
#[derive(Deserialize, Serialize)]
struct LegacyPolicy {
    policy_version: u32,
    sandbox: LegacySandboxMode,
    #[serde(default)]
    sandbox_unsafe_ack: bool,
    network: LegacyNetworkPolicy,
    #[serde(default)]
    network_unsafe_ack: bool,
    fs: LegacyFsPolicy,
    #[serde(default)]
    fs_write_unsafe_ack: bool,
    #[serde(default)]
    fs_strict_write: bool,
    exec: ExecPolicy,
    env: EnvPolicy,
    budgets: Budgets,
    artifacts: ArtifactsPolicy,
    #[serde(default)]
    replay: ReplayPolicy,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum LegacySandboxMode {
    Seatbelt,
    None,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum LegacyNetworkPolicy {
    Disabled,
    Enabled,
}

#[derive(Deserialize, Serialize, Default)]
struct LegacyFsPolicy {
    #[serde(default)]
    allowed_read: Vec<String>,
    #[serde(default)]
    allowed_write: Vec<String>,
    #[serde(default)]
    working_dir: Option<String>,
}

impl From<LegacyPolicy> for Policy {
    fn from(legacy: LegacyPolicy) -> Self {
        let sandbox = match legacy.sandbox {
            LegacySandboxMode::Seatbelt => SandboxMode::Seatbelt,
            LegacySandboxMode::None => SandboxMode::Disabled {
                ack: legacy.sandbox_unsafe_ack,
            },
        };

        let network = match legacy.network {
            LegacyNetworkPolicy::Disabled => NetworkPolicy::Disabled,
            LegacyNetworkPolicy::Enabled => NetworkPolicy::Enabled {
                ack: legacy.network_unsafe_ack,
            },
        };

        // network_unsafe_ack serves dual purpose in v3:
        // 1. Ack for network enabled
        // 2. Ack for unenforced network when sandbox disabled
        let network_enforcement = NetworkEnforcementAck {
            unenforced_ack: legacy.network_unsafe_ack,
        };

        let fs = FsPolicy {
            allowed_read: legacy.fs.allowed_read,
            allowed_write: legacy.fs.allowed_write,
            working_dir: legacy.fs.working_dir,
            write_ack: legacy.fs_write_unsafe_ack,
            strict_write: legacy.fs_strict_write,
        };

        Policy {
            policy_version: legacy.policy_version,
            sandbox,
            network,
            network_enforcement,
            fs,
            exec: legacy.exec,
            env: legacy.env,
            budgets: legacy.budgets,
            artifacts: legacy.artifacts,
            replay: legacy.replay,
        }
    }
}

impl From<Policy> for LegacyPolicy {
    fn from(policy: Policy) -> Self {
        let (sandbox, sandbox_unsafe_ack) = match policy.sandbox {
            SandboxMode::Seatbelt => (LegacySandboxMode::Seatbelt, false),
            SandboxMode::Disabled { ack } => (LegacySandboxMode::None, ack),
        };

        let (network, network_enabled_ack) = match policy.network {
            NetworkPolicy::Disabled => (LegacyNetworkPolicy::Disabled, false),
            NetworkPolicy::Enabled { ack } => (LegacyNetworkPolicy::Enabled, ack),
        };

        // Combine network enabled ack with enforcement ack for backward compat
        let network_unsafe_ack = network_enabled_ack || policy.network_enforcement.unenforced_ack;

        LegacyPolicy {
            policy_version: policy.policy_version,
            sandbox,
            sandbox_unsafe_ack,
            network,
            network_unsafe_ack,
            fs: LegacyFsPolicy {
                allowed_read: policy.fs.allowed_read,
                allowed_write: policy.fs.allowed_write,
                working_dir: policy.fs.working_dir,
            },
            fs_write_unsafe_ack: policy.fs.write_ack,
            fs_strict_write: policy.fs.strict_write,
            exec: policy.exec,
            env: policy.env,
            budgets: policy.budgets,
            artifacts: policy.artifacts,
            replay: policy.replay,
        }
    }
}

impl Serialize for Policy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        LegacyPolicy::from(self.clone()).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Policy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        LegacyPolicy::deserialize(deserializer).map(Policy::from)
    }
}

// =============================================================================
// Helper Methods for Type-Level Acknowledgements
// =============================================================================

impl SandboxMode {
    /// Check if sandbox is disabled.
    #[must_use]
    pub fn is_disabled(&self) -> bool {
        matches!(self, Self::Disabled { .. })
    }

    /// Check if sandbox is disabled with proper acknowledgement.
    #[must_use]
    pub fn is_disabled_with_ack(&self) -> bool {
        matches!(self, Self::Disabled { ack: true })
    }

    /// Get the acknowledgement status (false for Seatbelt mode).
    #[must_use]
    pub fn ack(&self) -> bool {
        match self {
            Self::Seatbelt => false,
            Self::Disabled { ack } => *ack,
        }
    }
}

impl NetworkPolicy {
    /// Check if network is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled { .. })
    }

    /// Check if network is enabled with proper acknowledgement.
    #[must_use]
    pub fn is_enabled_with_ack(&self) -> bool {
        matches!(self, Self::Enabled { ack: true })
    }

    /// Get the acknowledgement status (false for Disabled mode).
    #[must_use]
    pub fn ack(&self) -> bool {
        match self {
            Self::Disabled => false,
            Self::Enabled { ack } => *ack,
        }
    }
}

impl FsPolicy {
    /// Check if write access is configured.
    #[must_use]
    pub fn has_write_access(&self) -> bool {
        !self.allowed_write.is_empty()
    }

    /// Check if write access is configured with proper acknowledgement.
    #[must_use]
    pub fn has_write_access_with_ack(&self) -> bool {
        !self.allowed_write.is_empty() && self.write_ack
    }
}

// =============================================================================
// Backward Compatibility Accessor Methods
// =============================================================================

impl Policy {
    /// Get `sandbox_unsafe_ack` for backward compatibility.
    ///
    /// Returns true if sandbox is disabled with acknowledgement.
    #[must_use]
    pub fn sandbox_unsafe_ack(&self) -> bool {
        self.sandbox.ack()
    }

    /// Get `network_unsafe_ack` for backward compatibility.
    ///
    /// Returns true if network is enabled with ack OR network enforcement is acked.
    #[must_use]
    pub fn network_unsafe_ack(&self) -> bool {
        self.network.ack() || self.network_enforcement.unenforced_ack
    }

    /// Get `fs_write_unsafe_ack` for backward compatibility.
    #[must_use]
    pub fn fs_write_unsafe_ack(&self) -> bool {
        self.fs.write_ack
    }

    /// Get `fs_strict_write` for backward compatibility.
    #[must_use]
    pub fn fs_strict_write(&self) -> bool {
        self.fs.strict_write
    }
}

// =============================================================================
// Other Policy Types (unchanged)
// =============================================================================

/// Executable allowlist policy.
///
/// Commands must be in `allowed_executables` to run.
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ExecPolicy {
    /// Absolute paths to allowed executables.
    #[serde(default)]
    pub allowed_executables: Vec<String>,
    /// Allow shell execution (e.g., `sh -c`). Default false.
    #[serde(default)]
    pub allow_shell: bool,
}

/// Environment variable policy.
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct EnvPolicy {
    /// Environment variable names to allow through.
    #[serde(default)]
    pub allowlist: Vec<String>,
    /// Explicit environment variable values to set.
    #[serde(default)]
    pub set: BTreeMap<String, String>,
    /// Inherit environment from parent (filtered by allowlist).
    #[serde(default)]
    pub inherit: bool,
}

/// Resource budgets for execution limits.
///
/// Exceeding any budget results in `E_TIMEOUT` error.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ArtifactsPolicy {
    /// Enable artifact collection.
    #[serde(default)]
    pub enabled: bool,
    /// Directory for artifacts (must be in `allowed_write`).
    #[serde(default)]
    pub dir: Option<String>,
    /// Overwrite existing artifacts directory.
    #[serde(default)]
    pub overwrite: bool,
}

/// Replay comparison policy.
///
/// Controls how artifacts are compared during replay for regression testing.
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
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

// =============================================================================
// PolicyBuilder
// =============================================================================

/// Fluent builder for constructing [`Policy`] objects.
///
/// The builder automatically handles acknowledgement flags when you use
/// methods that require them, reducing boilerplate and preventing common mistakes.
///
/// # Example
///
/// ```ignore
/// use ptybox::model::policy::PolicyBuilder;
///
/// let policy = PolicyBuilder::new()
///     .sandbox_disabled()  // Auto-sets ack
///     .network_enabled()   // Auto-sets ack + enforcement ack
///     .allowed_read(vec!["/tmp".into()])
///     .allowed_write(vec!["/tmp/output".into()])  // Auto-sets write_ack
///     .allowed_executables(vec!["/bin/echo".into()])
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct PolicyBuilder {
    policy: Policy,
}

impl Default for PolicyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PolicyBuilder {
    /// Create a new builder with default policy values.
    ///
    /// Default policy has:
    /// - Sandbox: `Seatbelt` (enabled)
    /// - Network: `Disabled`
    /// - No read/write paths allowed
    /// - No executables allowed
    #[must_use]
    pub fn new() -> Self {
        Self {
            policy: Policy::default(),
        }
    }

    // =========================================================================
    // Sandbox Configuration
    // =========================================================================

    /// Enable Seatbelt sandbox (default on macOS).
    #[must_use]
    pub fn sandbox_seatbelt(mut self) -> Self {
        self.policy.sandbox = SandboxMode::Seatbelt;
        self
    }

    /// Disable sandbox with automatic acknowledgement.
    ///
    /// This also sets `network_enforcement.unenforced_ack` since network
    /// cannot be enforced without sandbox.
    #[must_use]
    pub fn sandbox_disabled(mut self) -> Self {
        self.policy.sandbox = SandboxMode::Disabled { ack: true };
        self.policy.network_enforcement.unenforced_ack = true;
        self
    }

    // =========================================================================
    // Network Configuration
    // =========================================================================

    /// Disable network access (default).
    #[must_use]
    pub fn network_disabled(mut self) -> Self {
        self.policy.network = NetworkPolicy::Disabled;
        self
    }

    /// Enable network access with automatic acknowledgements.
    ///
    /// This also sets `network_enforcement.unenforced_ack` since enabling
    /// network is a security-sensitive operation.
    #[must_use]
    pub fn network_enabled(mut self) -> Self {
        self.policy.network = NetworkPolicy::Enabled { ack: true };
        self.policy.network_enforcement.unenforced_ack = true;
        self
    }

    // =========================================================================
    // Filesystem Configuration
    // =========================================================================

    /// Set the allowed read paths.
    #[must_use]
    pub fn allowed_read(mut self, paths: Vec<String>) -> Self {
        self.policy.fs.allowed_read = paths;
        self
    }

    /// Add a path to the read allowlist.
    #[must_use]
    pub fn add_read_path(mut self, path: String) -> Self {
        self.policy.fs.allowed_read.push(path);
        self
    }

    /// Set the allowed write paths with automatic acknowledgement.
    #[must_use]
    pub fn allowed_write(mut self, paths: Vec<String>) -> Self {
        if !paths.is_empty() {
            self.policy.fs.write_ack = true;
        }
        self.policy.fs.allowed_write = paths;
        self
    }

    /// Add a path to the write allowlist with automatic acknowledgement.
    #[must_use]
    pub fn add_write_path(mut self, path: String) -> Self {
        self.policy.fs.write_ack = true;
        self.policy.fs.allowed_write.push(path);
        self
    }

    /// Set the working directory.
    #[must_use]
    pub fn working_dir(mut self, dir: String) -> Self {
        self.policy.fs.working_dir = Some(dir);
        self
    }

    /// Enable strict write mode (artifacts path must be in write allowlist).
    #[must_use]
    pub fn strict_write(mut self) -> Self {
        self.policy.fs.strict_write = true;
        self.policy.fs.write_ack = true;
        self
    }

    // =========================================================================
    // Executable Configuration
    // =========================================================================

    /// Set the allowed executables.
    #[must_use]
    pub fn allowed_executables(mut self, execs: Vec<String>) -> Self {
        self.policy.exec.allowed_executables = execs;
        self
    }

    /// Add an executable to the allowlist.
    #[must_use]
    pub fn add_executable(mut self, exec: String) -> Self {
        self.policy.exec.allowed_executables.push(exec);
        self
    }

    /// Allow shell execution (use with caution).
    #[must_use]
    pub fn allow_shell(mut self) -> Self {
        self.policy.exec.allow_shell = true;
        self
    }

    // =========================================================================
    // Environment Configuration
    // =========================================================================

    /// Set the environment variable allowlist.
    #[must_use]
    pub fn env_allowlist(mut self, vars: Vec<String>) -> Self {
        self.policy.env.allowlist = vars;
        self
    }

    /// Add an environment variable to the allowlist.
    #[must_use]
    pub fn add_env_var(mut self, var: String) -> Self {
        self.policy.env.allowlist.push(var);
        self
    }

    /// Set environment variables to inject.
    #[must_use]
    pub fn env_set(mut self, vars: std::collections::BTreeMap<String, String>) -> Self {
        self.policy.env.set = vars;
        self
    }

    /// Enable environment inheritance.
    #[must_use]
    pub fn inherit_env(mut self) -> Self {
        self.policy.env.inherit = true;
        self
    }

    // =========================================================================
    // Budget Configuration
    // =========================================================================

    /// Set maximum runtime in milliseconds.
    #[must_use]
    pub fn max_runtime_ms(mut self, ms: u64) -> Self {
        self.policy.budgets.max_runtime_ms = ms;
        self
    }

    /// Set maximum number of steps.
    #[must_use]
    pub fn max_steps(mut self, steps: u64) -> Self {
        self.policy.budgets.max_steps = steps;
        self
    }

    /// Set maximum output bytes.
    #[must_use]
    pub fn max_output_bytes(mut self, bytes: u64) -> Self {
        self.policy.budgets.max_output_bytes = bytes;
        self
    }

    /// Set maximum snapshot bytes.
    #[must_use]
    pub fn max_snapshot_bytes(mut self, bytes: u64) -> Self {
        self.policy.budgets.max_snapshot_bytes = bytes;
        self
    }

    /// Set maximum wait time per action in milliseconds.
    #[must_use]
    pub fn max_wait_ms(mut self, ms: u64) -> Self {
        self.policy.budgets.max_wait_ms = ms;
        self
    }

    // =========================================================================
    // Artifacts Configuration
    // =========================================================================

    /// Enable artifact collection to the specified directory.
    #[must_use]
    pub fn artifacts_dir(mut self, dir: String) -> Self {
        self.policy.artifacts.enabled = true;
        self.policy.artifacts.dir = Some(dir);
        self
    }

    /// Allow overwriting existing artifacts.
    #[must_use]
    pub fn artifacts_overwrite(mut self) -> Self {
        self.policy.artifacts.overwrite = true;
        self
    }

    // =========================================================================
    // Build
    // =========================================================================

    /// Build the policy, consuming the builder.
    ///
    /// This returns the policy directly. Use `ptybox::policy::validate_policy`
    /// to validate the policy before use.
    #[must_use]
    pub fn build(self) -> Policy {
        self.policy
    }

    /// Get a reference to the policy being built.
    #[must_use]
    pub fn as_policy(&self) -> &Policy {
        &self.policy
    }
}
