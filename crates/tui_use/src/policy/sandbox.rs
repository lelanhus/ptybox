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
/// Rejects paths containing characters that could escape the S-expression string literal.
fn validate_seatbelt_path(s: &str) -> Result<(), RunnerError> {
    if s.contains('"')
        || s.contains('(')
        || s.contains(')')
        || s.contains('\n')
        || s.contains('\r')
        || s.contains('\0')
    {
        return Err(RunnerError::policy_denied(
            "E_POLICY_DENIED",
            "path contains characters unsafe for sandbox profiles",
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

    if matches!(policy.network, crate::model::policy::NetworkPolicy::Enabled) {
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
