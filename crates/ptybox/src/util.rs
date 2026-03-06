//! Shared utility functions used across runner, driver, session, artifacts, and replay modules.

use crate::artifacts::ArtifactsWriterConfig;
use crate::model::policy::{Policy, SandboxMode};
use crate::model::{ExitStatus, RunId, ScreenSnapshot};
use crate::policy::sandbox;
use crate::runner::{RunnerError, RunnerResult};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Compute elapsed milliseconds since `started_at`.
///
/// In practice, elapsed time is always well under `u64::MAX` milliseconds.
pub fn elapsed_ms(started_at: &Instant) -> u64 {
    // Elapsed time in practice is always well under u64::MAX milliseconds
    #[allow(clippy::cast_possible_truncation)]
    let ms = started_at.elapsed().as_millis() as u64;
    ms
}

/// Sleep until `deadline`, capped at `max_step` per sleep.
///
/// Yields immediately if within 500µs of the deadline to avoid oversleeping.
pub fn pause_until(deadline: Instant, max_step: Duration) {
    let now = Instant::now();
    if now >= deadline {
        return;
    }
    let remaining = deadline.saturating_duration_since(now);
    if remaining <= Duration::from_micros(500) {
        std::thread::yield_now();
        return;
    }
    std::thread::sleep(remaining.min(max_step));
}

/// Convert a `portable_pty` exit status to our [`ExitStatus`] type.
///
/// On Unix, detects signal-based termination by checking the raw exit code.
/// Conventionally, processes killed by signal N report exit code 128+N.
pub fn convert_exit_status(
    status: portable_pty::ExitStatus,
    terminated_by_harness: bool,
) -> ExitStatus {
    #[allow(clippy::cast_possible_wrap)]
    let code = status.exit_code() as i32;

    // On Unix, exit codes > 128 typically indicate signal-based termination
    // (shell convention: exit code = 128 + signal number)
    let signal = if !status.success() && code > 128 {
        Some(code - 128)
    } else {
        None
    };

    // If killed by signal, report signal info; exit_code is still available
    ExitStatus {
        success: status.success(),
        exit_code: Some(code),
        signal,
        terminated_by_harness,
    }
}

/// A writer that counts bytes written without allocating.
struct CountingWriter {
    count: u64,
}

impl CountingWriter {
    fn new() -> Self {
        Self { count: 0 }
    }
}

impl std::io::Write for CountingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        #[allow(clippy::cast_possible_truncation)]
        let len = buf.len();
        self.count += len as u64;
        Ok(len)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Estimate the serialized size of a snapshot in bytes.
///
/// Uses a counting writer to avoid allocating the full JSON payload.
///
/// # Errors
/// Returns `E_PROTOCOL` if the snapshot cannot be serialized.
pub fn snapshot_bytes(snapshot: &ScreenSnapshot) -> RunnerResult<u64> {
    debug_assert!(snapshot.rows > 0, "snapshot must have positive rows");
    debug_assert!(snapshot.cols > 0, "snapshot must have positive cols");

    let mut writer = CountingWriter::new();
    serde_json::to_writer(&mut writer, snapshot)
        .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to encode snapshot", err))?;
    Ok(writer.count)
}

/// FNV-1a 64-bit hash constants.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0100_0000_01b3;

/// FNV-1a 64-bit hash.
pub fn fnv1a_hash(data: &[u8]) -> u64 {
    let mut hash: u64 = FNV_OFFSET_BASIS;
    for byte in data {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Incremental FNV-1a hash state for streaming data.
pub struct FnvHashState {
    /// Current hash value.
    pub hash: u64,
}

impl Default for FnvHashState {
    fn default() -> Self {
        Self {
            hash: FNV_OFFSET_BASIS,
        }
    }
}

impl FnvHashState {
    /// Create a new hash state with the FNV offset basis.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Feed additional data into an incremental FNV-1a hash.
pub fn fnv1a_hash_incremental(state: &mut FnvHashState, data: &[u8]) {
    for byte in data {
        state.hash ^= u64::from(*byte);
        state.hash = state.hash.wrapping_mul(FNV_PRIME);
    }
}

/// Compute an FNV-1a checksum of a file and return as a hex string.
///
/// # Errors
/// Returns `E_IO` if the file cannot be read.
pub fn compute_checksum(path: &Path) -> RunnerResult<String> {
    let data =
        std::fs::read(path).map_err(|err| RunnerError::io("E_IO", "failed to read file", err))?;
    Ok(format!("{:016x}", fnv1a_hash(&data)))
}

// =============================================================================
// Spawn Infrastructure
// =============================================================================

/// Resolved command to spawn (possibly wrapped by sandbox-exec).
pub struct SpawnCommand {
    /// The command to execute.
    pub command: String,
    /// Arguments to the command.
    pub args: Vec<String>,
    /// Temporary sandbox profile path to clean up when done.
    pub cleanup_path: Option<PathBuf>,
}

/// Build the spawn command, wrapping in sandbox-exec if policy requires it.
///
/// # Errors
/// Returns `E_IO` if the sandbox profile cannot be written.
pub fn build_spawn_command(
    policy: &Policy,
    command: &str,
    args: &[String],
    artifacts_dir: Option<&PathBuf>,
    run_id: RunId,
) -> RunnerResult<SpawnCommand> {
    debug_assert!(!command.is_empty(), "command must not be empty");

    match policy.sandbox {
        SandboxMode::Seatbelt => {
            let profile_path = if let Some(dir) = artifacts_dir {
                dir.join("sandbox.sb")
            } else {
                std::env::temp_dir().join(format!("ptybox-{run_id}.sb"))
            };
            sandbox::write_profile(&profile_path, policy)?;
            let mut sandbox_args = vec!["-f".to_string(), profile_path.display().to_string()];
            sandbox_args.push(command.to_string());
            sandbox_args.extend(args.iter().cloned());
            let cleanup = if artifacts_dir.is_some() {
                None
            } else {
                Some(profile_path)
            };
            Ok(SpawnCommand {
                command: "/usr/bin/sandbox-exec".to_string(),
                args: sandbox_args,
                cleanup_path: cleanup,
            })
        }
        SandboxMode::Disabled { .. } => Ok(SpawnCommand {
            command: command.to_string(),
            args: args.to_vec(),
            cleanup_path: None,
        }),
    }
}

/// RAII guard for sandbox profile cleanup.
///
/// Ensures the sandbox profile file is deleted when the guard is dropped,
/// even on panic. This prevents temporary files from accumulating.
pub struct SandboxCleanupGuard {
    /// The path to clean up, if any.
    pub path: Option<PathBuf>,
}

impl SandboxCleanupGuard {
    /// Create a new cleanup guard for the given path.
    pub fn new(path: Option<PathBuf>) -> Self {
        Self { path }
    }
}

impl Drop for SandboxCleanupGuard {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

// =============================================================================
// Config Resolution
// =============================================================================

/// Resolve artifacts configuration from policy or explicit options.
pub fn resolve_artifacts_config(
    policy: &Policy,
    options: Option<ArtifactsWriterConfig>,
) -> Option<ArtifactsWriterConfig> {
    if options.is_some() {
        return options;
    }
    if policy.artifacts.enabled {
        if let Some(dir) = policy.artifacts.dir.as_ref() {
            return Some(ArtifactsWriterConfig {
                dir: PathBuf::from(dir),
                overwrite: policy.artifacts.overwrite,
            });
        }
    }
    None
}
