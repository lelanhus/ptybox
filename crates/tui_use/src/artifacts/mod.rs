use crate::model::{Policy, RunId, RunResult, Scenario, ScreenSnapshot};
use crate::runner::{RunnerError, RunnerResult};
use serde::Serialize;
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
    snapshot_count: usize,
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
        Ok(Self {
            dir: config.dir,
            transcript,
            snapshot_count: 0,
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

    pub fn write_snapshot(&mut self, snapshot: &ScreenSnapshot) -> RunnerResult<()> {
        self.snapshot_count += 1;
        let name = format!("snapshots/{:04}.json", self.snapshot_count);
        self.write_json(&name, snapshot)
    }

    pub fn write_transcript(&mut self, delta: &str) -> RunnerResult<()> {
        self.transcript
            .write_all(delta.as_bytes())
            .map_err(|err| RunnerError::io("E_IO", "failed to write transcript", err))?;
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
        Ok(())
    }
}
