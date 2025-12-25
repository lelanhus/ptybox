pub mod sandbox;

use crate::model::policy::{
    EnvPolicy, ExecPolicy, FsPolicy, NetworkPolicy, Policy, SandboxMode, POLICY_VERSION,
};
use crate::model::{Action, ActionType, RunConfig};
use crate::runner::RunnerError;
use std::path::{Component, Path, PathBuf};

/// Environment variables that could enable sandbox escape or library injection.
/// These are blocked even if explicitly added to the allowlist.
const DANGEROUS_ENV_VARS: &[&str] = &[
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "LD_AUDIT",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
    "DYLD_FRAMEWORK_PATH",
    "DYLD_FALLBACK_LIBRARY_PATH",
    "DYLD_ROOT_PATH",
    "PYTHONPATH",
    "RUBYLIB",
    "PERL5LIB",
    "CLASSPATH",
    "IFS",
];

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
    if let Err(err) = validate_sandbox_mode(&policy.sandbox, policy.sandbox_unsafe_ack) {
        errors.push(err.to_error_info());
    }
    if let Err(err) = validate_network_policy(policy) {
        errors.push(err.to_error_info());
    }
    if let Err(err) = validate_env_policy(&policy.env) {
        errors.push(err.to_error_info());
    }
    if let Err(err) = validate_fs_policy(&policy.fs, policy.fs_write_unsafe_ack) {
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
        return Err(RunnerError::protocol_with_context(
            "E_PROTOCOL",
            format!(
                "unsupported policy_version {}, expected {}",
                policy.policy_version, POLICY_VERSION
            ),
            serde_json::json!({
                "received_version": policy.policy_version,
                "expected_version": POLICY_VERSION,
                "fix": format!("Set policy_version to {}", POLICY_VERSION),
                "hint": "Run 'tui-use protocol-help --json' to see the current protocol versions"
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
    if matches!(policy.network, NetworkPolicy::Enabled) && !policy.network_unsafe_ack {
        return Err(RunnerError::policy_denied(
            "E_POLICY_DENIED",
            "network enabled without explicit acknowledgement",
            serde_json::json!({
                "network": "enabled",
                "fix": "Set policy.network_unsafe_ack to true to acknowledge the security implications",
                "note": "Network access allows the process to make external connections"
            }),
        ));
    }
    if matches!(policy.sandbox, SandboxMode::None) && !policy.network_unsafe_ack {
        return Err(RunnerError::policy_denied(
            "E_POLICY_DENIED",
            "network policy cannot be enforced without sandbox",
            serde_json::json!({
                "sandbox": "none",
                "fix": "Set policy.network_unsafe_ack to true to acknowledge that network restrictions cannot be enforced",
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
    if !policy.fs_strict_write || policy.fs_write_unsafe_ack {
        return Ok(());
    }
    if matches!(policy.sandbox, SandboxMode::None)
        && artifacts_dir.is_none()
        && !policy.artifacts.enabled
    {
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

pub fn validate_sandbox_mode(mode: &SandboxMode, unsafe_ack: bool) -> Result<(), RunnerError> {
    match mode {
        SandboxMode::Seatbelt => {
            crate::policy::sandbox::ensure_sandbox_available()?;
            Ok(())
        }
        SandboxMode::None => {
            if unsafe_ack {
                Ok(())
            } else {
                Err(RunnerError::policy_denied(
                    "E_POLICY_DENIED",
                    "sandbox disabled without explicit acknowledgement",
                    serde_json::json!({
                        "sandbox": "none",
                        "fix": "Set policy.sandbox_unsafe_ack to true to acknowledge that no sandbox will be used",
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
    validate_sandbox_mode(&policy.sandbox, policy.sandbox_unsafe_ack)?;
    validate_network_policy(policy)?;
    validate_env_policy(&policy.env)?;
    validate_fs_policy(&policy.fs, policy.fs_write_unsafe_ack)?;
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
            // Block dangerous environment variables that could enable sandbox escape
            if DANGEROUS_ENV_VARS
                .iter()
                .any(|d| d.eq_ignore_ascii_case(key))
            {
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
        // Block dangerous environment variables that could enable sandbox escape
        if DANGEROUS_ENV_VARS
            .iter()
            .any(|d| d.eq_ignore_ascii_case(key))
        {
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
    let shell_names = ["sh", "bash", "zsh", "dash", "fish", "ksh", "tcsh", "csh"];
    let base = Path::new(command)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(command);

    // Block shell scripts by extension
    if command.ends_with(".sh") {
        return true;
    }

    // Block any invocation of a known shell, not just -c flag
    if shell_names.iter().any(|name| name == &base) {
        return true;
    }

    // Also check for -c flag with any shell-like command for extra safety
    let _ = args; // Keep args parameter for potential future use
    false
}

/// Blocked filesystem roots for security. Paths under these roots cannot be allowlisted.
const BLOCKED_FS_ROOTS: &[&str] = &["/", "/System", "/Library", "/Users", "/private", "/Volumes"];

pub fn validate_fs_policy(fs: &FsPolicy, fs_write_unsafe_ack: bool) -> Result<(), RunnerError> {
    let home_dir = std::env::var_os("HOME").map(PathBuf::from);
    let denied_roots: [(&Path, &str); 5] = [
        (Path::new("/System"), "system"),
        (Path::new("/Library"), "library"),
        (Path::new("/Users"), "users"),
        (Path::new("/private"), "private"),
        (Path::new("/Volumes"), "volumes"),
    ];
    if !fs.allowed_write.is_empty() && !fs_write_unsafe_ack {
        return Err(RunnerError::policy_denied(
            "E_POLICY_DENIED",
            "write allowlist requires explicit acknowledgement",
            serde_json::json!({
                "paths": fs.allowed_write,
                "fix": "Set policy.fs_write_unsafe_ack to true to acknowledge write access",
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

fn canonicalize_for_policy(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
            Component::RootDir => normalized.push(Component::RootDir.as_os_str()),
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
        }
    }
    if normalized.as_os_str().is_empty() {
        path.to_path_buf()
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
