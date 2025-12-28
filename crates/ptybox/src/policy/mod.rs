pub mod sandbox;

use crate::model::policy::{
    EnvPolicy, ExecPolicy, FsPolicy, NetworkPolicy, Policy, SandboxMode, POLICY_VERSION,
};
use crate::model::{Action, ActionType, RunConfig};
use crate::runner::RunnerError;
use std::path::{Component, Path, PathBuf};

/// Environment variables that could enable sandbox escape or library injection.
/// These are blocked even if explicitly added to the allowlist.
/// Note: Checking is case-insensitive to prevent bypass via mixed-case variants.
const DANGEROUS_ENV_VARS: &[&str] = &[
    // Linux library injection
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "LD_AUDIT",
    // macOS library injection
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
    "DYLD_FRAMEWORK_PATH",
    "DYLD_FALLBACK_LIBRARY_PATH",
    "DYLD_ROOT_PATH",
    // Language paths
    "PYTHONPATH",
    "RUBYLIB",
    "PERL5LIB",
    "CLASSPATH",
    // Shell/system
    "IFS",
    "GMON_OUT_PREFIX", // Profiling output directory
    "MALLOC_CONF",     // Memory allocator configuration
];

/// Check if an environment variable name is in the dangerous list.
/// Uses case-insensitive comparison to prevent bypass via mixed-case variants.
fn is_dangerous_env_var(key: &str) -> bool {
    DANGEROUS_ENV_VARS
        .iter()
        .any(|d| d.eq_ignore_ascii_case(key))
}

/// Well-known system paths that are symlinks by design.
/// These are allowed because they are controlled by the OS and cannot be manipulated by users.
const ALLOWED_SYSTEM_SYMLINKS: &[&str] = &[
    "/tmp",     // -> /private/tmp on macOS
    "/var",     // -> /private/var on macOS
    "/etc",     // -> /private/etc on macOS
    "/home",    // May be a symlink on some systems
    "/usr/bin", // Standard system path
    "/usr/lib", // Standard system path
    "/bin",     // May be symlink to /usr/bin on some systems
    "/lib",     // May be symlink to /usr/lib on some systems
    "/sbin",    // May be symlink to /usr/sbin on some systems
];

/// Validates that a path is not a user-created symlink.
///
/// Symlinks in policy paths could be used to escape sandbox restrictions by
/// pointing to locations outside the intended allowlist. This is a defense-in-depth
/// check; the Seatbelt sandbox provides primary protection.
///
/// System symlinks (like `/tmp -> /private/tmp` on macOS) are allowed because
/// they are controlled by the OS and cannot be manipulated by unprivileged users.
///
/// # Security Warning: TOCTOU Race Condition
///
/// **CRITICAL**: This check is vulnerable to Time-of-Check-Time-of-Use (TOCTOU) attacks.
/// A malicious actor could:
/// 1. Create a regular file at the allowed path
/// 2. Wait for this validation to pass
/// 3. Replace the file with a symlink to a sensitive location (e.g., `/etc/shadow`)
/// 4. The process would then access the symlinked location
///
/// **Mitigation**: The Seatbelt sandbox provides runtime protection against this attack.
/// When sandbox is disabled (`--no-sandbox --ack-unsafe-sandbox`), this TOCTOU race
/// becomes exploitable. For high-security scenarios, always run with the sandbox enabled.
///
/// This limitation is inherent to the Unix filesystem model and cannot be fully
/// mitigated without OS-level support (which Seatbelt provides on macOS).
fn validate_path_not_symlink(path: &Path) -> Result<(), RunnerError> {
    // Allow well-known system symlinks (e.g., /tmp -> /private/tmp on macOS)
    let path_str = path.to_string_lossy();
    for allowed in ALLOWED_SYSTEM_SYMLINKS {
        if path_str == *allowed || path_str.starts_with(&format!("{allowed}/")) {
            return Ok(());
        }
    }

    // Only check if the path exists - non-existent paths will fail at access time
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() {
            return Err(RunnerError::policy_denied(
                "E_POLICY_DENIED",
                "symlinks are not allowed in policy paths",
                Some(serde_json::json!({
                    "path": path.display().to_string(),
                    "fix": "Use the real path instead of a symlink",
                    "note": "Symlinks could bypass sandbox restrictions",
                    "allowed_system_symlinks": ALLOWED_SYSTEM_SYMLINKS
                })),
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct EffectivePolicy {
    pub policy: Policy,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PolicyExplanation {
    pub allowed: bool,
    pub errors: Vec<crate::model::ErrorInfo>,
}

impl EffectivePolicy {
    pub fn new(policy: Policy) -> Self {
        Self { policy }
    }

    pub fn validate_run_config(&self, run: &RunConfig) -> Result<(), RunnerError> {
        let exec = &self.policy.exec;
        if exec.allowed_executables.is_empty() {
            return Err(RunnerError::policy_denied(
                "E_POLICY_DENIED",
                "no executables are allowed by policy",
                serde_json::json!({
                    "requested": run.command,
                    "fix": "Add the executable path to policy.exec.allowed_executables",
                    "example": {"exec": {"allowed_executables": [run.command.clone()]}}
                }),
            ));
        }

        if !Path::new(&run.command).is_absolute() {
            return Err(RunnerError::policy_denied(
                "E_POLICY_DENIED",
                "command must be an absolute path",
                serde_json::json!({
                    "requested": run.command,
                    "fix": "Provide the full path to the executable",
                    "example": format!("/usr/bin/{}", run.command)
                }),
            ));
        }

        for allowed in &exec.allowed_executables {
            if !Path::new(allowed).is_absolute() {
                return Err(RunnerError::policy_denied(
                    "E_POLICY_DENIED",
                    "allowed executable paths must be absolute",
                    serde_json::json!({"path": allowed}),
                ));
            }
            // Check for symlinks (defense-in-depth against sandbox escape)
            validate_path_not_symlink(Path::new(allowed))?;
        }

        if !exec.allowed_executables.iter().any(|p| p == &run.command) {
            return Err(RunnerError::policy_denied(
                "E_POLICY_DENIED",
                "executable is not allowlisted",
                serde_json::json!({
                    "requested": run.command,
                    "allowed_executables": exec.allowed_executables,
                    "fix": "Add the executable path to policy.exec.allowed_executables"
                }),
            ));
        }

        if is_shell_command(&run.command, &run.args) && !exec.allow_shell {
            return Err(RunnerError::policy_denied(
                "E_POLICY_DENIED",
                "shell execution is disabled by policy",
                serde_json::json!({
                    "requested": run.command,
                    "args": run.args,
                    "reason": "Shell commands are blocked for security",
                    "fix": "Set policy.exec.allow_shell to true (not recommended)",
                    "alternative": "Use direct executable paths instead of shell wrappers"
                }),
            ));
        }

        let fs = &self.policy.fs;
        if let Some(cwd) = &run.cwd {
            if !Path::new(cwd).is_absolute() {
                return Err(RunnerError::policy_denied(
                    "E_POLICY_DENIED",
                    "working directory must be an absolute path",
                    serde_json::json!({"cwd": cwd}),
                ));
            }
            if !path_allowed(cwd, &fs.allowed_read, &fs.allowed_write) {
                return Err(RunnerError::policy_denied(
                    "E_POLICY_DENIED",
                    "working directory is not within allowlisted paths",
                    serde_json::json!({"cwd": cwd}),
                ));
            }
        }

        if let Some(policy_cwd) = &fs.working_dir {
            if !path_allowed(policy_cwd, &fs.allowed_read, &fs.allowed_write) {
                return Err(RunnerError::policy_denied(
                    "E_POLICY_DENIED",
                    "policy working_dir is not within allowlisted paths",
                    serde_json::json!({"working_dir": policy_cwd}),
                ));
            }
        }

        Ok(())
    }

    pub fn validate_action(&self, action: &Action) -> Result<(), RunnerError> {
        if matches!(action.action_type, ActionType::Terminate) {
            return Ok(());
        }
        if matches!(action.action_type, ActionType::Wait) {
            return Ok(());
        }
        Ok(())
    }

    pub fn apply_env_policy(
        &self,
        cmd: &mut portable_pty::CommandBuilder,
    ) -> Result<(), RunnerError> {
        apply_env_policy(&self.policy.env, cmd)
    }
}

pub fn explain_policy_for_run_config(policy: &Policy, run: &RunConfig) -> PolicyExplanation {
    let mut errors = Vec::new();
    if let Err(err) = validate_policy_version(policy) {
        errors.push(err.to_error_info());
    }
    if let Err(err) = validate_sandbox_mode(&policy.sandbox) {
        errors.push(err.to_error_info());
    }
    if let Err(err) = validate_network_policy(policy) {
        errors.push(err.to_error_info());
    }
    if let Err(err) = validate_env_policy(&policy.env) {
        errors.push(err.to_error_info());
    }
    if let Err(err) = validate_fs_policy(&policy.fs) {
        errors.push(err.to_error_info());
    }
    if let Err(err) = validate_artifacts_policy(policy) {
        errors.push(err.to_error_info());
    }
    if let Err(err) = validate_write_access(policy, None) {
        errors.push(err.to_error_info());
    }
    let effective_policy = EffectivePolicy::new(policy.clone());
    if let Err(err) = effective_policy.validate_run_config(run) {
        errors.push(err.to_error_info());
    }

    PolicyExplanation {
        allowed: errors.is_empty(),
        errors,
    }
}

pub fn validate_policy_version(policy: &Policy) -> Result<(), RunnerError> {
    if policy.policy_version != POLICY_VERSION {
        return Err(RunnerError::protocol(
            "E_PROTOCOL",
            format!(
                "unsupported policy_version {}, expected {}",
                policy.policy_version, POLICY_VERSION
            ),
            serde_json::json!({
                "received_version": policy.policy_version,
                "expected_version": POLICY_VERSION,
                "fix": format!("Set policy_version to {}", POLICY_VERSION),
                "hint": "Run 'ptybox protocol-help --json' to see the current protocol versions"
            }),
        ));
    }
    Ok(())
}

fn path_allowed(path: &str, allowed_read: &[String], allowed_write: &[String]) -> bool {
    let path = canonicalize_for_policy(Path::new(path));
    allowed_read
        .iter()
        .chain(allowed_write.iter())
        .any(|allowed| {
            let allowed = canonicalize_for_policy(Path::new(allowed));
            path.starts_with(&allowed)
        })
}

pub fn validate_shell_policy(exec: &ExecPolicy) -> Result<(), RunnerError> {
    if exec.allow_shell {
        Ok(())
    } else {
        Err(RunnerError::policy_denied(
            "E_POLICY_DENIED",
            "shell execution is disabled by policy",
            None,
        ))
    }
}

pub fn validate_network_policy(policy: &Policy) -> Result<(), RunnerError> {
    // Check if network is enabled without acknowledgement
    if let NetworkPolicy::Enabled { ack } = &policy.network {
        if !ack {
            return Err(RunnerError::policy_denied(
                "E_POLICY_DENIED",
                "network enabled without explicit acknowledgement",
                serde_json::json!({
                    "network": "enabled",
                    "fix": "Set network_unsafe_ack to true to acknowledge the security implications",
                    "note": "Network access allows the process to make external connections"
                }),
            ));
        }
    }
    // Check if sandbox is disabled - network policy cannot be enforced without sandbox
    if policy.sandbox.is_disabled() && !policy.network_enforcement.unenforced_ack {
        return Err(RunnerError::policy_denied(
            "E_POLICY_DENIED",
            "network policy cannot be enforced without sandbox",
            serde_json::json!({
                "sandbox": "disabled",
                "fix": "Set network_unsafe_ack to true to acknowledge that network restrictions cannot be enforced",
                "alternative": "Use sandbox: 'seatbelt' (macOS) to enforce network restrictions"
            }),
        ));
    }
    Ok(())
}

pub fn validate_artifacts_policy(policy: &Policy) -> Result<(), RunnerError> {
    if policy.artifacts.enabled {
        let dir = policy.artifacts.dir.as_ref().ok_or_else(|| {
            RunnerError::policy_denied(
                "E_POLICY_DENIED",
                "artifacts enabled without directory",
                serde_json::json!({
                    "artifacts_enabled": true,
                    "artifacts_dir": null,
                    "fix": "Set policy.artifacts.dir to an absolute path within allowed_write paths",
                    "example": {"artifacts": {"enabled": true, "dir": "/tmp/artifacts"}}
                }),
            )
        })?;
        validate_artifacts_dir(Path::new(dir), &policy.fs)?;
    }
    Ok(())
}

pub fn validate_write_access(
    policy: &Policy,
    artifacts_dir: Option<&Path>,
) -> Result<(), RunnerError> {
    if !policy.fs.strict_write || policy.fs.write_ack {
        return Ok(());
    }
    if policy.sandbox.is_disabled() && artifacts_dir.is_none() && !policy.artifacts.enabled {
        return Ok(());
    }
    let mut reasons = Vec::new();
    if matches!(policy.sandbox, SandboxMode::Seatbelt) {
        reasons.push("sandbox_profile");
    }
    if policy.artifacts.enabled {
        reasons.push("artifacts");
    }
    if artifacts_dir.is_some() {
        reasons.push("artifacts_cli");
    }
    if reasons.is_empty() {
        return Ok(());
    }
    Err(RunnerError::policy_denied(
        "E_POLICY_DENIED",
        "write access requires explicit acknowledgement",
        serde_json::json!({ "reasons": reasons }),
    ))
}

pub fn validate_sandbox_mode(mode: &SandboxMode) -> Result<(), RunnerError> {
    match mode {
        SandboxMode::Seatbelt => {
            crate::policy::sandbox::ensure_sandbox_available()?;
            Ok(())
        }
        SandboxMode::Disabled { ack } => {
            if *ack {
                Ok(())
            } else {
                Err(RunnerError::policy_denied(
                    "E_POLICY_DENIED",
                    "sandbox disabled without explicit acknowledgement",
                    serde_json::json!({
                        "sandbox": "disabled",
                        "fix": "Set sandbox_unsafe_ack to true to acknowledge that no sandbox will be used",
                        "note": "Without sandbox, filesystem and network policies cannot be enforced by the OS",
                        "alternative": "Use sandbox: 'seatbelt' on macOS for OS-level enforcement"
                    }),
                ))
            }
        }
    }
}

pub fn validate_env_policy(env: &EnvPolicy) -> Result<(), RunnerError> {
    for key in env.set.keys() {
        if !env.allowlist.iter().any(|allowed| allowed == key) {
            return Err(RunnerError::policy_denied(
                "E_POLICY_DENIED",
                "env var set without allowlist entry",
                serde_json::json!({
                    "var": key,
                    "current_allowlist": env.allowlist,
                    "fix": format!("Add '{}' to policy.env.allowlist", key)
                }),
            ));
        }
    }
    Ok(())
}

pub fn validate_policy(policy: &Policy) -> Result<(), RunnerError> {
    validate_policy_version(policy)?;
    validate_sandbox_mode(&policy.sandbox)?;
    validate_network_policy(policy)?;
    validate_env_policy(&policy.env)?;
    validate_fs_policy(&policy.fs)?;
    validate_artifacts_policy(policy)?;
    validate_write_access(policy, None)?;
    Ok(())
}

pub fn apply_env_policy(
    env_policy: &EnvPolicy,
    cmd: &mut portable_pty::CommandBuilder,
) -> Result<(), RunnerError> {
    cmd.env_clear();

    if env_policy.inherit {
        for key in &env_policy.allowlist {
            // Block dangerous environment variables that could enable sandbox escape.
            // Uses case-insensitive comparison to prevent bypass via mixed-case variants.
            if is_dangerous_env_var(key) {
                return Err(RunnerError::policy_denied(
                    "E_POLICY_DENIED",
                    "dangerous environment variable blocked",
                    Some(serde_json::json!({
                        "var": key,
                        "reason": "This variable could enable sandbox escape or library injection",
                        "blocked_vars": DANGEROUS_ENV_VARS,
                        "fix": format!("Remove '{}' from policy.env.allowlist", key)
                    })),
                ));
            }
            if let Ok(value) = std::env::var(key) {
                cmd.env(key, value);
            }
        }
    }

    for (key, value) in &env_policy.set {
        // Block dangerous environment variables that could enable sandbox escape.
        // Uses case-insensitive comparison to prevent bypass via mixed-case variants.
        if is_dangerous_env_var(key) {
            return Err(RunnerError::policy_denied(
                "E_POLICY_DENIED",
                "dangerous environment variable blocked",
                Some(serde_json::json!({
                    "var": key,
                    "reason": "This variable could enable sandbox escape or library injection",
                    "blocked_vars": DANGEROUS_ENV_VARS,
                    "fix": format!("Remove '{}' from policy.env.set", key)
                })),
            ));
        }
        if env_policy.allowlist.iter().any(|allowed| allowed == key) {
            cmd.env(key, value);
        }
    }

    Ok(())
}

fn is_shell_command(command: &str, args: &[String]) -> bool {
    // Suppress unused warning - args kept for API compatibility and potential future use
    let _ = args;

    let shell_names = ["sh", "bash", "zsh", "dash", "fish", "ksh", "tcsh", "csh"];

    // Block shell scripts by extension
    if command.ends_with(".sh") {
        return true;
    }

    // Resolve symlinks to get the real executable path, preventing bypass via symlinked shells.
    // Example attack: `ln -s /bin/bash /tmp/mycommand` would bypass basename-only checking.
    let resolved_path = std::fs::canonicalize(command).unwrap_or_else(|_| PathBuf::from(command));
    let base = resolved_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(command);

    // Block any invocation of a known shell.
    // Note: We intentionally do NOT block `-c` flag for non-shell interpreters.
    // Python, Ruby, Perl use -c for legitimate purposes:
    // - Python -c: execute inline code (not shell execution)
    // - Ruby -c: syntax check only
    // - Perl -c: compile only (syntax check)
    // We only block shell executables themselves, not arbitrary -c usage.
    shell_names.contains(&base)
}

/// Blocked filesystem roots for security. Paths under these roots cannot be allowlisted.
const BLOCKED_FS_ROOTS: &[&str] = &["/", "/System", "/Library", "/Users", "/private", "/Volumes"];

pub fn validate_fs_policy(fs: &FsPolicy) -> Result<(), RunnerError> {
    let home_dir = std::env::var_os("HOME").map(PathBuf::from);
    let denied_roots: [(&Path, &str); 5] = [
        (Path::new("/System"), "system"),
        (Path::new("/Library"), "library"),
        (Path::new("/Users"), "users"),
        (Path::new("/private"), "private"),
        (Path::new("/Volumes"), "volumes"),
    ];
    if !fs.allowed_write.is_empty() && !fs.write_ack {
        return Err(RunnerError::policy_denied(
            "E_POLICY_DENIED",
            "write allowlist requires explicit acknowledgement",
            serde_json::json!({
                "paths": fs.allowed_write,
                "fix": "Set fs_write_unsafe_ack to true to acknowledge write access",
                "note": "Write access allows the process to modify files in allowlisted paths"
            }),
        ));
    }
    for allowed in fs.allowed_read.iter().chain(fs.allowed_write.iter()) {
        if !Path::new(allowed).is_absolute() {
            return Err(RunnerError::policy_denied(
                "E_POLICY_DENIED",
                "allowlist paths must be absolute",
                serde_json::json!({
                    "path": allowed,
                    "fix": "Use an absolute path starting with /",
                    "example": format!("/tmp/{}", allowed)
                }),
            ));
        }
        // Check for symlinks (defense-in-depth against sandbox escape)
        validate_path_not_symlink(Path::new(allowed))?;
        let allowed_path = canonicalize_for_policy(Path::new(allowed));
        if let Some(reason) =
            disallowed_allowlist_reason(&allowed_path, home_dir.as_deref(), &denied_roots)
        {
            return Err(RunnerError::policy_denied(
                "E_POLICY_DENIED",
                "disallowed allowlist path",
                serde_json::json!({
                    "path": allowed,
                    "reason": reason,
                    "blocked_roots": BLOCKED_FS_ROOTS,
                    "suggestion": "Use /tmp or another non-system path",
                    "fix": "Copy files to /tmp and use /tmp paths instead",
                    "example": {
                        "allowed_read": ["/tmp"],
                        "allowed_write": ["/tmp"]
                    }
                }),
            ));
        }
    }

    if let Some(cwd) = &fs.working_dir {
        if !Path::new(cwd).is_absolute() {
            return Err(RunnerError::policy_denied(
                "E_POLICY_DENIED",
                "working_dir must be an absolute path",
                serde_json::json!({"working_dir": cwd}),
            ));
        }
        if !path_allowed(cwd, &fs.allowed_read, &fs.allowed_write) {
            return Err(RunnerError::policy_denied(
                "E_POLICY_DENIED",
                "policy working_dir is not within allowlisted paths",
                serde_json::json!({"working_dir": cwd}),
            ));
        }
    }
    Ok(())
}

pub fn validate_artifacts_dir(dir: &Path, fs: &FsPolicy) -> Result<(), RunnerError> {
    if !dir.is_absolute() {
        return Err(RunnerError::policy_denied(
            "E_POLICY_DENIED",
            "artifacts dir must be an absolute path",
            serde_json::json!({
                "dir": dir,
                "fix": "Use an absolute path starting with /",
                "example": "/tmp/artifacts"
            }),
        ));
    }
    let dir = canonicalize_for_policy(dir);
    if !path_allowed_write(&dir, &fs.allowed_write) {
        return Err(RunnerError::policy_denied(
            "E_POLICY_DENIED",
            "artifacts dir is not within allowlisted write paths",
            serde_json::json!({
                "dir": dir,
                "allowed_write": fs.allowed_write,
                "fix": "Add the artifacts directory to policy.fs.allowed_write",
                "example": {"fs": {"allowed_write": [dir.to_string_lossy()]}}
            }),
        ));
    }
    Ok(())
}

fn disallowed_allowlist_reason(
    path: &Path,
    home_dir: Option<&Path>,
    denied_roots: &[(&Path, &'static str)],
) -> Option<&'static str> {
    if is_root_path(path) {
        return Some("root");
    }
    if let Some(home) = home_dir {
        if path == home {
            return Some("home");
        }
    }
    for (root, reason) in denied_roots {
        if path.starts_with(root) {
            return Some(*reason);
        }
    }
    None
}

fn is_root_path(path: &Path) -> bool {
    let mut components = path.components();
    matches!(components.next(), Some(Component::RootDir)) && components.next().is_none()
}

/// Normalizes a path for policy comparison by removing `.` and resolving `..` components.
///
/// This function does NOT follow symlinks - it performs lexical normalization only.
/// Parent directory components (`..`) at the root level are ignored (cannot escape root).
///
/// # Security Note
/// Symlinks must be validated separately before trusting paths. This function only
/// handles lexical path traversal attempts like `/foo/../bar`.
fn canonicalize_for_policy(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    let mut depth = 0usize; // Track depth below root to prevent escape

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                // Only pop if we're below root level (depth > 0)
                if depth > 0 {
                    normalized.pop();
                    depth -= 1;
                }
                // At root level, ignore the .. (cannot go above root)
            }
            Component::Normal(part) => {
                normalized.push(part);
                depth += 1;
            }
            Component::RootDir => normalized.push(Component::RootDir.as_os_str()),
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
        }
    }

    // If normalization results in empty path (shouldn't happen with absolute paths),
    // return root instead of the potentially unsafe original path
    if normalized.as_os_str().is_empty() {
        PathBuf::from("/")
    } else {
        normalized
    }
}

fn path_allowed_write(path: &Path, allowed_write: &[String]) -> bool {
    let path = canonicalize_for_policy(path);
    allowed_write.iter().any(|allowed| {
        let allowed = canonicalize_for_policy(Path::new(allowed));
        path.starts_with(&allowed)
    })
}
