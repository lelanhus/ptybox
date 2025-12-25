use crate::model::{NormalizationRecord, Policy, RunId, RunResult, Scenario, ScreenSnapshot};
use crate::runner::{RunnerError, RunnerResult};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ArtifactsWriterConfig {
    pub dir: PathBuf,
    pub overwrite: bool,
}

pub struct ArtifactsWriter {
    dir: PathBuf,
    transcript: fs::File,
    events: fs::File,
    snapshot_count: usize,
    checksums: BTreeMap<String, String>,
}

impl ArtifactsWriter {
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
        })
    }

    pub fn write_policy(&mut self, policy: &Policy) -> RunnerResult<()> {
        self.write_json("policy.json", policy)
    }

    pub fn write_scenario(&mut self, scenario: &Scenario) -> RunnerResult<()> {
        self.write_json("scenario.json", scenario)
    }

    pub fn write_run_result(&mut self, run_result: &RunResult) -> RunnerResult<()> {
        self.write_json("run.json", run_result)
    }

    pub fn write_normalization(&mut self, record: &NormalizationRecord) -> RunnerResult<()> {
        self.write_json("normalization.json", record)
    }

    pub fn write_snapshot(&mut self, snapshot: &ScreenSnapshot) -> RunnerResult<()> {
        self.snapshot_count += 1;
        let name = format!("snapshots/{:06}.json", self.snapshot_count);
        self.write_json(&name, snapshot)
    }

    pub fn write_transcript(&mut self, delta: &str) -> RunnerResult<()> {
        self.transcript
            .write_all(delta.as_bytes())
            .map_err(|err| RunnerError::io("E_IO", "failed to write transcript", err))?;
        self.transcript
            .flush()
            .map_err(|err| RunnerError::io("E_IO", "failed to flush transcript", err))?;
        self.record_checksum("transcript.log")?;
        Ok(())
    }

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
        self.record_checksum("events.jsonl")?;
        Ok(())
    }

    fn write_json<T: Serialize>(&mut self, name: &str, value: &T) -> RunnerResult<()> {
        let path = self.dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| RunnerError::io("E_IO", "failed to create artifacts dir", err))?;
        }
        let data = serde_json::to_vec_pretty(value)
            .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to serialize", err))?;
        fs::write(&path, data)
            .map_err(|err| RunnerError::io("E_IO", "failed to write artifact", err))?;
        self.record_checksum(name)?;
        Ok(())
    }

    fn record_checksum(&mut self, name: &str) -> RunnerResult<()> {
        if name == "checksums.json" {
            return Ok(());
        }
        let path = self.dir.join(name);
        let checksum = compute_checksum(&path)?;
        self.checksums.insert(name.to_string(), checksum);
        self.write_checksums()
    }

    fn write_checksums(&mut self) -> RunnerResult<()> {
        let data = serde_json::to_vec_pretty(&self.checksums)
            .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to serialize checksums", err))?;
        let path = self.dir.join("checksums.json");
        fs::write(&path, data)
            .map_err(|err| RunnerError::io("E_IO", "failed to write checksums", err))?;
        Ok(())
    }
}

fn compute_checksum(path: &PathBuf) -> RunnerResult<String> {
    let data = fs::read(path).map_err(|err| RunnerError::io("E_IO", "failed to read file", err))?;
    Ok(format!("{:016x}", fnv1a_hash(&data)))
}

fn fnv1a_hash(data: &[u8]) -> u64 {
    // FNV-1a constants (64-bit)
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0100_0000_01b3;

    let mut hash: u64 = FNV_OFFSET_BASIS;
    for byte in data {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
