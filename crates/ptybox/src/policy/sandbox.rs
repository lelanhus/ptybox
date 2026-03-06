//! macOS Seatbelt sandbox profile generation and availability checking.
//!
//! This module generates `sandbox-exec` profiles from a [`Policy`](crate::model::policy::Policy)
//! and verifies that the Seatbelt sandbox is available on the current platform.
//!
//! # Security
//!
//! - Profiles use a deny-default strategy: `(deny default)` with explicit allows
//! - Path characters are validated against a strict whitelist to prevent injection
//! - Profile files are written with `0600` permissions (owner-only read/write)

use crate::runner::{RunnerError, RunnerResult};
use std::fmt::Write as FmtWrite;
use std::fs::OpenOptions;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

/// Configuration for sandbox profile generation.
#[derive(Clone, Debug)]
pub struct SandboxConfig {
    /// Absolute path where the sandbox profile will be written.
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

/// Check that Seatbelt (`sandbox-exec`) is available and functional.
///
/// Runs a minimal sandbox profile against `/usr/bin/true` to verify
/// the sandbox subsystem works on this platform.
///
/// # Errors
/// - `E_SANDBOX_UNAVAILABLE` if `sandbox-exec` is not found or fails
/// - `E_POLICY_DENIED` if `sandbox-exec` exists but cannot execute
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
        Err(_) => Err(RunnerError::sandbox_unavailable(
            "sandbox-exec not available (Seatbelt requires macOS)",
        )),
    }
}

/// Generate and write a Seatbelt sandbox profile to disk.
///
/// The profile encodes the policy's filesystem, network, and executable
/// allowlists as Seatbelt S-expressions. All paths are validated against
/// a character whitelist before embedding.
///
/// On Unix, the file is created with mode `0600` to prevent other users
/// from reading the sandbox rules.
///
/// # Errors
/// - `E_POLICY_DENIED` if any path contains unsafe characters
/// - `E_IO` if the file cannot be created or written
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
