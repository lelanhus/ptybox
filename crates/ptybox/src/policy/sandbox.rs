use crate::runner::{RunnerError, RunnerResult};
use std::fmt::Write as FmtWrite;
use std::fs::OpenOptions;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Clone, Debug)]
pub struct SandboxConfig {
    pub profile_path: String,
}

/// Validates that a path is safe to embed in a Seatbelt profile.
/// Uses a whitelist approach: only allows characters known to be safe in S-expression string literals.
/// This is more secure than a blacklist because it rejects any unknown/unexpected characters.
fn validate_seatbelt_path(s: &str) -> Result<(), RunnerError> {
    // Whitelist: alphanumeric, -, _, ., /, @, space
    // These are the only characters allowed in paths for sandbox profiles
    let is_valid = s.chars().all(|ch| {
        ch.is_ascii_alphanumeric()
            || ch == '-'
            || ch == '_'
            || ch == '.'
            || ch == '/'
            || ch == '@'
            || ch == ' '
    });

    if !is_valid {
        return Err(RunnerError::policy_denied(
            "E_POLICY_DENIED",
            "path contains characters unsafe for sandbox profiles (only alphanumeric, -, _, ., /, @, space allowed)",
            Some(serde_json::json!({ "path": s })),
        ));
    }
    Ok(())
}

pub fn ensure_sandbox_available() -> RunnerResult<()> {
    let status = Command::new("/usr/bin/sandbox-exec")
        .arg("-p")
        .arg("(version 1)(allow default)")
        .arg("/usr/bin/true")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    match status {
        Ok(exit) if exit.success() => Ok(()),
        Ok(_) => Err(RunnerError::policy_denied(
            "E_SANDBOX_UNAVAILABLE",
            "sandbox-exec failed to run",
            None,
        )),
        Err(err) => Err(RunnerError::io(
            "E_SANDBOX_UNAVAILABLE",
            "sandbox-exec not available",
            err,
        )),
    }
}

pub fn write_profile(path: &Path, policy: &crate::model::policy::Policy) -> RunnerResult<()> {
    let content = build_profile(policy)?;

    // Write with restrictive permissions (0600) to prevent other users from reading sandbox rules
    #[cfg(unix)]
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|err| RunnerError::io("E_IO", "failed to create sandbox profile", err))?;
        file.write_all(content.as_bytes())
            .map_err(|err| RunnerError::io("E_IO", "failed to write sandbox profile", err))?;
    }

    #[cfg(not(unix))]
    std::fs::write(path, content)
        .map_err(|err| RunnerError::io("E_IO", "failed to write sandbox profile", err))?;

    Ok(())
}

fn build_profile(policy: &crate::model::policy::Policy) -> RunnerResult<String> {
    let mut profile = String::new();
    profile.push_str("(version 1)\n");
    profile.push_str("(deny default)\n");
    profile.push_str("(import \"system.sb\")\n");
    profile.push_str("(import \"bsd.sb\")\n");

    if policy.network.is_enabled() {
        profile.push_str("(allow network-outbound (remote ip))\n");
    }

    for path in &policy.fs.allowed_read {
        validate_seatbelt_path(path)?;
        // write! to String is infallible, ignore result
        let _ = writeln!(profile, "(allow file-read* (subpath \"{path}\"))");
    }
    for path in &policy.fs.allowed_write {
        validate_seatbelt_path(path)?;
        let _ = writeln!(profile, "(allow file-write* (subpath \"{path}\"))");
    }

    for exe in &policy.exec.allowed_executables {
        validate_seatbelt_path(exe)?;
        let _ = writeln!(profile, "(allow process-exec (literal \"{exe}\"))");
    }

    Ok(profile)
}
