//! Artifact collection and persistence for run results.
//!
//! This module writes structured artifacts to disk during scenario execution,
//! driver sessions, and replay comparisons. Artifacts provide a complete
//! record of a run for debugging, replay regression testing, and audit.
//!
//! # Artifact Files
//!
//! | File | Contents |
//! |------|----------|
//! | `run.json` | [`RunResult`] with status, steps, final observation |
//! | `policy.json` | Effective [`Policy`] used for the run |
//! | `scenario.json` | Resolved [`Scenario`] (including driver-generated) |
//! | `transcript.log` | Raw terminal output (cumulative) |
//! | `events.jsonl` | NDJSON stream of [`Observation`](crate::model::Observation) records |
//! | `snapshots/*.json` | Sequential [`ScreenSnapshot`] captures |
//! | `normalization.json` | Applied normalization filters for replay |
//! | `checksums.json` | FNV-1a checksums for integrity verification |
//! | `sandbox.sb` | Seatbelt profile (when sandbox is enabled) |
//!
//! # Key Types
//!
//! - [`ArtifactsWriterConfig`] — Directory path and overwrite settings
//! - [`ArtifactsWriter`] — Stateful writer with transcript/event handles and checksum tracking
//!
//! # Atomic Writes
//!
//! JSON artifacts are written atomically via write-to-temp + rename to
//! prevent partial writes from leaving corrupt files on interruption.

use crate::model::{NormalizationRecord, Policy, RunId, RunResult, Scenario, ScreenSnapshot};
use crate::runner::{RunnerError, RunnerResult};
use crate::util::{compute_checksum, fnv1a_hash_incremental, FnvHashState};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Configuration for the artifacts writer.
///
/// Specifies the output directory and whether existing artifacts can be overwritten.
#[derive(Clone, Debug)]
pub struct ArtifactsWriterConfig {
    /// Directory path for artifact output. Must be an absolute path
    /// within the policy's `allowed_write` paths.
    pub dir: PathBuf,
    /// Whether to overwrite an existing artifacts directory.
    /// When `false`, returns `E_POLICY_DENIED` if the directory exists.
    pub overwrite: bool,
}

/// Stateful artifact writer that manages transcript, event, and snapshot output.
///
/// Maintains open file handles for streaming artifacts (transcript and events)
/// and tracks checksums for integrity verification. Checksums are flushed
/// lazily to reduce I/O overhead.
///
/// The writer implements [`Drop`] to flush pending checksums and file handles
/// on cleanup, ensuring artifact integrity even on early exit.
pub struct ArtifactsWriter {
    dir: PathBuf,
    transcript: fs::File,
    events: fs::File,
    snapshot_count: usize,
    checksums: BTreeMap<String, String>,
    /// Track whether checksums need to be written (dirty flag for batching)
    checksums_dirty: bool,
    /// Incremental hash state for streaming files (transcript, events)
    incremental_hashes: HashMap<String, FnvHashState>,
}

impl Drop for ArtifactsWriter {
    fn drop(&mut self) {
        // Best-effort flush of file handles before close
        let _ = self.transcript.flush();
        let _ = self.events.flush();

        // Write final checksums if dirty (batched writes optimization)
        if self.checksums_dirty {
            let _ = self.write_checksums_internal();
        }
    }
}

impl ArtifactsWriter {
    /// Create a new artifacts writer for the given run.
    ///
    /// Creates the output directory (and `snapshots/` subdirectory) if needed.
    /// Opens file handles for `transcript.log` and `events.jsonl`.
    ///
    /// # Errors
    ///
    /// - `E_POLICY_DENIED` if the directory exists and `overwrite` is false
    /// - `E_IO` if directory creation or file open fails
    pub fn new(_run_id: RunId, config: ArtifactsWriterConfig) -> RunnerResult<Self> {
        if config.dir.exists() {
            if !config.overwrite {
                return Err(RunnerError::policy_denied(
                    "E_POLICY_DENIED",
                    "artifacts directory exists and overwrite is disabled",
                    serde_json::json!({"dir": config.dir}),
                ));
            }
        } else {
            fs::create_dir_all(&config.dir)
                .map_err(|err| RunnerError::io("E_IO", "failed to create artifacts dir", err))?;
        }
        let transcript_path = config.dir.join("transcript.log");
        let transcript = fs::File::create(&transcript_path)
            .map_err(|err| RunnerError::io("E_IO", "failed to create transcript", err))?;
        let events_path = config.dir.join("events.jsonl");
        let events = fs::File::create(&events_path)
            .map_err(|err| RunnerError::io("E_IO", "failed to create events log", err))?;
        Ok(Self {
            dir: config.dir,
            transcript,
            events,
            snapshot_count: 0,
            checksums: BTreeMap::new(),
            checksums_dirty: false,
            incremental_hashes: HashMap::new(),
        })
    }

    /// Write the effective policy as `policy.json`.
    ///
    /// # Errors
    /// Returns `E_IO` on write failure, `E_PROTOCOL` on serialization failure.
    pub fn write_policy(&mut self, policy: &Policy) -> RunnerResult<()> {
        self.write_json("policy.json", policy)
    }

    /// Write the resolved scenario as `scenario.json`.
    ///
    /// # Errors
    /// Returns `E_IO` on write failure, `E_PROTOCOL` on serialization failure.
    pub fn write_scenario(&mut self, scenario: &Scenario) -> RunnerResult<()> {
        self.write_json("scenario.json", scenario)
    }

    /// Write the run result summary as `run.json`.
    ///
    /// # Errors
    /// Returns `E_IO` on write failure, `E_PROTOCOL` on serialization failure.
    pub fn write_run_result(&mut self, run_result: &RunResult) -> RunnerResult<()> {
        self.write_json("run.json", run_result)
    }

    /// Write the normalization record as `normalization.json`.
    ///
    /// Records which normalization filters and rules were applied,
    /// enabling replay to reproduce the same comparison settings.
    ///
    /// # Errors
    /// Returns `E_IO` on write failure, `E_PROTOCOL` on serialization failure.
    pub fn write_normalization(&mut self, record: &NormalizationRecord) -> RunnerResult<()> {
        self.write_json("normalization.json", record)
    }

    /// Write a screen snapshot as `snapshots/NNNNNN.json`.
    ///
    /// Snapshots are numbered sequentially starting from 1.
    ///
    /// # Errors
    /// Returns `E_IO` on write failure, `E_PROTOCOL` on serialization failure.
    pub fn write_snapshot(&mut self, snapshot: &ScreenSnapshot) -> RunnerResult<()> {
        self.snapshot_count += 1;
        let name = format!("snapshots/{:06}.json", self.snapshot_count);
        self.write_json(&name, snapshot)
    }

    /// Append raw terminal output to `transcript.log`.
    ///
    /// Each call appends the delta and flushes immediately.
    ///
    /// # Errors
    /// Returns `E_IO` on write or flush failure.
    pub fn write_transcript(&mut self, delta: &str) -> RunnerResult<()> {
        let bytes = delta.as_bytes();
        self.transcript
            .write_all(bytes)
            .map_err(|err| RunnerError::io("E_IO", "failed to write transcript", err))?;
        self.transcript
            .flush()
            .map_err(|err| RunnerError::io("E_IO", "failed to flush transcript", err))?;
        self.record_checksum_incremental("transcript.log", bytes);
        Ok(())
    }

    /// Append an observation record to `events.jsonl` as NDJSON.
    ///
    /// Each observation is serialized as a single JSON line and flushed.
    ///
    /// # Errors
    /// Returns `E_IO` on write failure, `E_PROTOCOL` on serialization failure.
    pub fn write_observation(
        &mut self,
        observation: &crate::model::Observation,
    ) -> RunnerResult<()> {
        let data = serde_json::to_vec(observation)
            .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to serialize observation", err))?;
        self.events
            .write_all(&data)
            .map_err(|err| RunnerError::io("E_IO", "failed to write events log", err))?;
        self.events
            .write_all(b"\n")
            .map_err(|err| RunnerError::io("E_IO", "failed to write events log", err))?;
        self.events
            .flush()
            .map_err(|err| RunnerError::io("E_IO", "failed to flush events log", err))?;
        self.record_checksum_incremental("events.jsonl", &data);
        self.record_checksum_incremental("events.jsonl", b"\n");
        Ok(())
    }

    /// Write a single JSON line to a named artifact file.
    ///
    /// The file is created if it does not exist and appended to when it does.
    pub fn write_json_line<T: Serialize>(&mut self, name: &str, value: &T) -> RunnerResult<()> {
        let path = self.dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| RunnerError::io("E_IO", "failed to create artifacts dir", err))?;
        }

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|err| RunnerError::io("E_IO", "failed to open jsonl artifact", err))?;
        let data = serde_json::to_vec(value)
            .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to serialize jsonl line", err))?;
        file.write_all(&data)
            .map_err(|err| RunnerError::io("E_IO", "failed to write jsonl artifact", err))?;
        file.write_all(b"\n")
            .map_err(|err| RunnerError::io("E_IO", "failed to write jsonl newline", err))?;
        file.flush()
            .map_err(|err| RunnerError::io("E_IO", "failed to flush jsonl artifact", err))?;
        self.record_checksum_incremental(name, &data);
        self.record_checksum_incremental(name, b"\n");
        Ok(())
    }

    /// Artifacts root directory for this writer.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn write_json<T: Serialize>(&mut self, name: &str, value: &T) -> RunnerResult<()> {
        let path = self.dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| RunnerError::io("E_IO", "failed to create artifacts dir", err))?;
        }
        let data = serde_json::to_vec_pretty(value)
            .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to serialize", err))?;
        atomic_write(&path, &data)?;
        self.record_checksum(name)?;
        Ok(())
    }

    /// Record a checksum for an artifact by re-reading the file from disk.
    /// Used for non-streaming artifacts (JSON files written atomically).
    /// The checksum file is written lazily to reduce I/O overhead (batched writes).
    fn record_checksum(&mut self, name: &str) -> RunnerResult<()> {
        if name == "checksums.json" {
            return Ok(());
        }
        let path = self.dir.join(name);
        let checksum = compute_checksum(&path)?;
        self.checksums.insert(name.to_string(), checksum);
        self.checksums_dirty = true;
        Ok(())
    }

    /// Incrementally update the checksum for a streaming artifact without
    /// re-reading the entire file. Used for transcript.log, events.jsonl,
    /// and other append-only files.
    fn record_checksum_incremental(&mut self, name: &str, data: &[u8]) {
        if name == "checksums.json" {
            return;
        }
        let state = self.incremental_hashes.entry(name.to_string()).or_default();
        fnv1a_hash_incremental(state, data);
        self.checksums
            .insert(name.to_string(), format!("{:016x}", state.hash));
        self.checksums_dirty = true;
    }

    /// Flush all pending checksums to disk. Call this after all artifacts
    /// have been written to ensure checksums.json is complete.
    pub fn flush_checksums(&mut self) -> RunnerResult<()> {
        if self.checksums_dirty {
            self.write_checksums_internal()?;
            self.checksums_dirty = false;
        }
        Ok(())
    }

    /// Internal method to write checksums (used by flush and Drop)
    fn write_checksums_internal(&self) -> RunnerResult<()> {
        let data = serde_json::to_vec_pretty(&self.checksums)
            .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to serialize checksums", err))?;
        let path = self.dir.join("checksums.json");
        atomic_write(&path, &data)?;
        Ok(())
    }
}

/// Write data to a file atomically via write-to-temp + rename.
///
/// Prevents partial writes from leaving corrupt artifacts when the
/// process is interrupted mid-write (e.g., SIGKILL, power loss).
fn atomic_write(path: &Path, data: &[u8]) -> RunnerResult<()> {
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, data)
        .map_err(|err| RunnerError::io("E_IO", "failed to write temp artifact", err))?;
    fs::rename(&tmp_path, path)
        .map_err(|err| RunnerError::io("E_IO", "failed to rename artifact into place", err))?;
    Ok(())
}
