use crate::runner::{RunnerError, RunnerResult};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Clone, Debug)]
pub struct SandboxConfig {
    pub profile_path: String,
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
    let content = build_profile(policy);
    fs::write(path, content)
        .map_err(|err| RunnerError::io("E_IO", "failed to write sandbox profile", err))?;
    Ok(())
}

fn build_profile(policy: &crate::model::policy::Policy) -> String {
    let mut profile = String::new();
    profile.push_str("(version 1)\n");
    profile.push_str("(deny default)\n");
    profile.push_str("(import \"system.sb\")\n");
    profile.push_str("(import \"bsd.sb\")\n");

    if matches!(policy.network, crate::model::policy::NetworkPolicy::Enabled) {
        profile.push_str("(allow network-outbound (remote ip))\n");
    }

    for path in &policy.fs.allowed_read {
        profile.push_str(&format!("(allow file-read* (subpath \"{}\"))\n", path));
    }
    for path in &policy.fs.allowed_write {
        profile.push_str(&format!("(allow file-write* (subpath \"{}\"))\n", path));
    }

    for exe in &policy.exec.allowed_executables {
        profile.push_str(&format!("(allow process-exec (literal \"{}\"))\n", exe));
    }

    profile
}
