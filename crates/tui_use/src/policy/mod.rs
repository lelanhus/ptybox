pub mod sandbox;

use crate::model::policy::{EnvPolicy, ExecPolicy, FsPolicy, NetworkPolicy, Policy, SandboxMode};
use crate::model::{Action, ActionType, RunConfig};
use crate::runner::RunnerError;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct EffectivePolicy {
    pub policy: Policy,
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
                serde_json::json!({"requested": run.command}),
            ));
        }

        if !Path::new(&run.command).is_absolute() {
            return Err(RunnerError::policy_denied(
                "E_POLICY_DENIED",
                "command must be an absolute path",
                serde_json::json!({"requested": run.command}),
            ));
        }

        if !exec.allowed_executables.iter().any(|p| p == &run.command) {
            return Err(RunnerError::policy_denied(
                "E_POLICY_DENIED",
                "executable is not allowlisted",
                serde_json::json!({"requested": run.command}),
            ));
        }

        if is_shell_command(&run.command, &run.args) && !exec.allow_shell {
            return Err(RunnerError::policy_denied(
                "E_POLICY_DENIED",
                "shell execution is disabled by policy",
                serde_json::json!({"requested": run.command, "args": run.args}),
            ));
        }

        let fs = &self.policy.fs;
        if let Some(cwd) = &run.cwd {
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

fn path_allowed(path: &str, allowed_read: &[String], allowed_write: &[String]) -> bool {
    let path = PathBuf::from(path);
    allowed_read
        .iter()
        .chain(allowed_write.iter())
        .any(|allowed| is_within(&path, allowed))
}

fn is_within(path: &Path, allowed: &str) -> bool {
    let allowed = PathBuf::from(allowed);
    path.starts_with(allowed)
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

pub fn validate_network_policy(network: &NetworkPolicy) -> Result<(), RunnerError> {
    if matches!(network, NetworkPolicy::Disabled) {
        return Err(RunnerError::policy_denied(
            "E_POLICY_DENIED",
            "network access is disabled by policy",
            None,
        ));
    }
    Ok(())
}

pub fn validate_sandbox_mode(mode: &SandboxMode) -> Result<(), RunnerError> {
    match mode {
        SandboxMode::Seatbelt => {
            crate::policy::sandbox::ensure_sandbox_available()?;
            Ok(())
        }
        SandboxMode::None => Err(RunnerError::policy_denied(
            "E_POLICY_DENIED",
            "sandbox disabled without explicit acknowledgement",
            None,
        )),
    }
}

pub fn validate_env_policy(env: &EnvPolicy) -> Result<(), RunnerError> {
    for key in env.set.keys() {
        if !env.allowlist.iter().any(|allowed| allowed == key) {
            return Err(RunnerError::policy_denied(
                "E_POLICY_DENIED",
                "env var set without allowlist entry",
                serde_json::json!({"var": key}),
            ));
        }
    }
    Ok(())
}

pub fn apply_env_policy(
    env_policy: &EnvPolicy,
    cmd: &mut portable_pty::CommandBuilder,
) -> Result<(), RunnerError> {
    cmd.env_clear();

    if env_policy.inherit {
        for key in &env_policy.allowlist {
            if let Ok(value) = std::env::var(key) {
                cmd.env(key, value);
            }
        }
    }

    for (key, value) in &env_policy.set {
        if env_policy.allowlist.iter().any(|allowed| allowed == key) {
            cmd.env(key, value);
        }
    }

    Ok(())
}

fn is_shell_command(command: &str, args: &[String]) -> bool {
    let shell_names = ["sh", "bash", "zsh", "dash", "fish"];
    let base = Path::new(command)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(command);
    if shell_names.iter().any(|name| name == &base) {
        return args.iter().any(|arg| arg == "-c" || arg == "--command");
    }
    false
}

pub fn validate_fs_policy(fs: &FsPolicy) -> Result<(), RunnerError> {
    if let Some(cwd) = &fs.working_dir {
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
