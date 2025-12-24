use crate::artifacts::ArtifactsWriterConfig;
use crate::model::{
    NormalizationFilter, NormalizationRecord, NormalizationRule, NormalizationRuleTarget,
    NormalizationSource, RunId, RunResult, ScreenSnapshot, NORMALIZATION_VERSION,
};
use crate::runner::{run_scenario, RunnerError, RunnerOptions, RunnerResult};
use crate::scenario::load_scenario_file;
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub struct ReplayOptions {
    pub strict: bool,
    pub filters: Option<Vec<NormalizationFilter>>,
    pub require_events: bool,
    pub require_checksums: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReplayExplanation {
    pub strict: bool,
    pub filters: Vec<NormalizationFilter>,
    pub rules: Vec<NormalizationRule>,
    pub source: NormalizationSource,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReplayReport {
    pub replay: Value,
    pub diff: Option<Value>,
    pub dir: String,
}

#[derive(Clone, Debug)]
struct ReplaySettings {
    strict: bool,
    filters: Vec<NormalizationFilter>,
    rules: Vec<NormalizationRule>,
    source: NormalizationSource,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReplaySummary {
    pub replay_version: u32,
    pub status: String,
    pub source: NormalizationSource,
    pub strict: bool,
    pub filters: Vec<NormalizationFilter>,
    pub rules: Vec<NormalizationRule>,
    pub mismatch: Option<ReplayMismatch>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReplayMismatch {
    pub kind: String,
    pub index: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReplayDiff {
    pub kind: String,
    pub index: Option<usize>,
    pub code: String,
    pub message: String,
    pub context: Option<Value>,
}

pub fn explain_replay(
    artifacts_dir: &Path,
    options: ReplayOptions,
) -> RunnerResult<ReplayExplanation> {
    let policy = load_policy_from_artifacts(artifacts_dir)?;
    let settings = resolve_replay_settings(&policy.replay, &options);
    Ok(ReplayExplanation {
        strict: settings.strict,
        filters: settings.filters,
        rules: settings.rules,
        source: settings.source,
    })
}

pub fn read_replay_report(artifacts_dir: &Path) -> RunnerResult<ReplayReport> {
    let replay_dir = latest_replay_dir(artifacts_dir)?;
    let replay_path = replay_dir.join("replay.json");
    let replay_data = fs::read_to_string(&replay_path)
        .map_err(|err| RunnerError::io("E_IO", "failed to read replay.json", err))?;
    let replay_value: Value = serde_json::from_str(&replay_data)
        .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to parse replay.json", err))?;
    let diff_path = replay_dir.join("diff.json");
    let diff_value = if diff_path.exists() {
        let diff_data = fs::read_to_string(&diff_path)
            .map_err(|err| RunnerError::io("E_IO", "failed to read diff.json", err))?;
        Some(
            serde_json::from_str(&diff_data)
                .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to parse diff.json", err))?,
        )
    } else {
        None
    };
    Ok(ReplayReport {
        replay: replay_value,
        diff: diff_value,
        dir: replay_dir.display().to_string(),
    })
}

pub fn replay_artifacts(artifacts_dir: &Path, options: ReplayOptions) -> RunnerResult<RunResult> {
    let policy = load_policy_from_artifacts(artifacts_dir)?;
    let policy_replay = policy.replay.clone();
    let mut scenario = load_scenario_from_artifacts(artifacts_dir)?;
    scenario.run.policy = crate::model::scenario::PolicyRef::Inline(policy);

    let replay_dir = artifacts_dir.join(format!("replay-{}", RunId::new()));
    let runner_options = RunnerOptions {
        artifacts: Some(ArtifactsWriterConfig {
            dir: replay_dir.clone(),
            overwrite: true,
        }),
    };
    let run_result = run_scenario(scenario, runner_options)?;

    let settings = resolve_replay_settings(&policy_replay, &options);
    write_normalization_record(&replay_dir, &settings)?;

    let original_snapshots = load_snapshots(
        &artifacts_dir.join("snapshots"),
        &settings.filters,
        &settings.rules,
    )?;
    let replay_snapshots = load_snapshots(
        &replay_dir.join("snapshots"),
        &settings.filters,
        &settings.rules,
    )?;

    let mut summary = ReplaySummary {
        replay_version: 1,
        status: "passed".to_string(),
        source: settings.source.clone(),
        strict: settings.strict,
        filters: settings.filters.clone(),
        rules: settings.rules.clone(),
        mismatch: None,
    };

    let compare_result = (|| {
        if options.require_events {
            let original_events = artifacts_dir.join("events.jsonl");
            let replay_events = replay_dir.join("events.jsonl");
            if !original_events.exists() || !replay_events.exists() {
                return Err(RunnerError::policy_denied(
                    "E_REPLAY_MISMATCH",
                    "event stream missing",
                    serde_json::json!({ "kind": "events" }),
                ));
            }
        }
        validate_checksums(artifacts_dir, options.require_checksums)?;
        validate_checksums(&replay_dir, options.require_checksums)?;
        compare_snapshots(&original_snapshots, &replay_snapshots)?;
        compare_transcript(
            &artifacts_dir.join("transcript.log"),
            &replay_dir.join("transcript.log"),
            &settings.rules,
        )?;
        compare_run_results(
            &artifacts_dir.join("run.json"),
            &replay_dir.join("run.json"),
            &settings.filters,
            &settings.rules,
        )?;
        compare_events(
            &artifacts_dir.join("events.jsonl"),
            &replay_dir.join("events.jsonl"),
            &settings.filters,
            &settings.rules,
            options.require_events,
        )?;
        Ok::<(), RunnerError>(())
    })();
    match compare_result {
        Ok(()) => {
            write_replay_summary(&replay_dir, &summary)?;
            Ok(run_result)
        }
        Err(err) => {
            summary.status = "failed".to_string();
            summary.mismatch = mismatch_from_error(&err);
            write_replay_summary(&replay_dir, &summary)?;
            write_replay_diff(&replay_dir, &err)?;
            Err(err)
        }
    }
}

fn load_scenario_from_artifacts(artifacts_dir: &Path) -> RunnerResult<crate::model::Scenario> {
    let scenario_path = artifacts_dir.join("scenario.json");
    if !scenario_path.exists() {
        return Err(RunnerError::io(
            "E_IO",
            "artifacts missing scenario.json",
            "missing scenario",
        ));
    }
    load_scenario_file(
        scenario_path
            .to_str()
            .ok_or_else(|| RunnerError::io("E_IO", "invalid scenario path", "path"))?,
    )
}

fn latest_replay_dir(artifacts_dir: &Path) -> RunnerResult<PathBuf> {
    let mut candidates: Vec<(PathBuf, std::time::SystemTime)> = fs::read_dir(artifacts_dir)
        .map_err(|err| RunnerError::io("E_IO", "failed to read artifacts dir", err))?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            let name = path.file_name()?.to_string_lossy();
            if !name.starts_with("replay-") {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((path, modified))
        })
        .collect();
    candidates.sort_by_key(|(_, modified)| *modified);
    let dir = candidates
        .last()
        .map(|(path, _)| path.clone())
        .ok_or_else(|| RunnerError::io("E_IO", "no replay artifacts found", "replay"))?;
    Ok(dir)
}

fn load_policy_from_artifacts(artifacts_dir: &Path) -> RunnerResult<crate::model::policy::Policy> {
    let policy_path = artifacts_dir.join("policy.json");
    if !policy_path.exists() {
        return Err(RunnerError::io(
            "E_IO",
            "artifacts missing policy.json",
            "missing policy",
        ));
    }
    let policy_data = fs::read_to_string(&policy_path)
        .map_err(|err| RunnerError::io("E_IO", "failed to read policy", err))?;
    let policy: crate::model::policy::Policy = serde_json::from_str(&policy_data)
        .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to parse policy", err))?;
    Ok(policy)
}

fn resolve_replay_settings(
    policy: &crate::model::policy::ReplayPolicy,
    options: &ReplayOptions,
) -> ReplaySettings {
    if options.strict {
        return ReplaySettings {
            strict: true,
            filters: Vec::new(),
            rules: Vec::new(),
            source: NormalizationSource::Cli,
        };
    }
    if let Some(filters) = options.filters.clone() {
        return ReplaySettings {
            strict: false,
            filters,
            rules: policy.normalization_rules.clone().unwrap_or_default(),
            source: NormalizationSource::Cli,
        };
    }
    if policy.strict {
        return ReplaySettings {
            strict: true,
            filters: Vec::new(),
            rules: Vec::new(),
            source: NormalizationSource::Policy,
        };
    }
    if let Some(filters) = policy.normalization_filters.clone() {
        return ReplaySettings {
            strict: false,
            filters,
            rules: policy.normalization_rules.clone().unwrap_or_default(),
            source: NormalizationSource::Policy,
        };
    }
    ReplaySettings {
        strict: false,
        filters: default_replay_filters(),
        rules: policy.normalization_rules.clone().unwrap_or_default(),
        source: NormalizationSource::Default,
    }
}

fn default_replay_filters() -> Vec<NormalizationFilter> {
    vec![
        NormalizationFilter::SnapshotId,
        NormalizationFilter::RunId,
        NormalizationFilter::RunTimestamps,
        NormalizationFilter::StepTimestamps,
        NormalizationFilter::ObservationTimestamp,
        NormalizationFilter::SessionId,
    ]
}

fn write_normalization_record(dir: &Path, settings: &ReplaySettings) -> RunnerResult<()> {
    let record = NormalizationRecord {
        normalization_version: NORMALIZATION_VERSION,
        filters: settings.filters.clone(),
        strict: settings.strict,
        source: settings.source.clone(),
        rules: settings.rules.clone(),
    };
    let data = serde_json::to_vec_pretty(&record)
        .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to serialize normalization", err))?;
    let path = dir.join("normalization.json");
    fs::write(&path, data)
        .map_err(|err| RunnerError::io("E_IO", "failed to write normalization", err))?;
    update_checksum_entry(dir, "normalization.json")?;
    Ok(())
}

fn write_replay_summary(dir: &Path, summary: &ReplaySummary) -> RunnerResult<()> {
    let data = serde_json::to_vec_pretty(summary)
        .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to serialize replay summary", err))?;
    let path = dir.join("replay.json");
    fs::write(&path, data)
        .map_err(|err| RunnerError::io("E_IO", "failed to write replay summary", err))?;
    Ok(())
}

fn write_replay_diff(dir: &Path, err: &RunnerError) -> RunnerResult<()> {
    let mismatch = mismatch_from_error(err);
    let diff = ReplayDiff {
        kind: mismatch
            .as_ref()
            .map(|value| value.kind.clone())
            .unwrap_or_else(|| "unknown".to_string()),
        index: mismatch.and_then(|value| value.index),
        code: err.code.clone(),
        message: err.message.clone(),
        context: err.context.clone(),
    };
    let data = serde_json::to_vec_pretty(&diff)
        .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to serialize replay diff", err))?;
    let path = dir.join("diff.json");
    fs::write(&path, data)
        .map_err(|err| RunnerError::io("E_IO", "failed to write replay diff", err))?;
    Ok(())
}

fn load_snapshots(
    dir: &Path,
    filters: &[NormalizationFilter],
    rules: &[NormalizationRule],
) -> RunnerResult<Vec<Value>> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|err| RunnerError::io("E_IO", "failed to read snapshots dir", err))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    entries.sort();

    let mut snapshots = Vec::new();
    for path in entries {
        let data = fs::read_to_string(&path)
            .map_err(|err| RunnerError::io("E_IO", "failed to read snapshot", err))?;
        let snapshot: ScreenSnapshot = serde_json::from_str(&data)
            .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to parse snapshot", err))?;
        let value = serde_json::to_value(snapshot)
            .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to serialize snapshot", err))?;
        let value = if has_filter(filters, NormalizationFilter::SnapshotId) {
            strip_snapshot_id(value)
        } else {
            value
        };
        snapshots.push(apply_rules_to_snapshot(value, rules));
    }
    Ok(snapshots)
}

fn strip_snapshot_id(mut value: Value) -> Value {
    if let Value::Object(ref mut obj) = value {
        obj.remove("snapshot_id");
    }
    value
}

fn apply_rules_to_text(
    mut text: String,
    rules: &[NormalizationRule],
    target: NormalizationRuleTarget,
) -> String {
    for rule in rules {
        if rule.target != target {
            continue;
        }
        if let Ok(re) = regex::Regex::new(&rule.pattern) {
            text = re.replace_all(&text, rule.replace.as_str()).to_string();
        }
    }
    text
}

fn apply_rules_to_snapshot(mut value: Value, rules: &[NormalizationRule]) -> Value {
    if let Value::Object(ref mut obj) = value {
        apply_rules_to_snapshot_object(obj, rules);
    }
    value
}

fn apply_rules_to_snapshot_object(
    obj: &mut serde_json::Map<String, Value>,
    rules: &[NormalizationRule],
) {
    let Some(lines) = obj.get_mut("lines").and_then(|val| val.as_array_mut()) else {
        return;
    };
    for line in lines.iter_mut() {
        if let Some(text) = line.as_str() {
            let normalized = apply_rules_to_text(
                text.to_string(),
                rules,
                NormalizationRuleTarget::SnapshotLines,
            );
            *line = Value::String(normalized);
        }
    }
}

fn compare_snapshots(original: &[Value], replay: &[Value]) -> RunnerResult<()> {
    if original.len() != replay.len() {
        return Err(RunnerError::policy_denied(
            "E_REPLAY_MISMATCH",
            "snapshot count mismatch",
            serde_json::json!({
                "kind": "snapshot",
                "expected": original.len(),
                "actual": replay.len()
            }),
        ));
    }
    for (idx, (left, right)) in original.iter().zip(replay.iter()).enumerate() {
        if left != right {
            return Err(RunnerError::policy_denied(
                "E_REPLAY_MISMATCH",
                "snapshot content mismatch",
                serde_json::json!({ "kind": "snapshot", "index": idx }),
            ));
        }
    }

    Ok(())
}

fn compare_transcript(
    original: &Path,
    replay: &Path,
    rules: &[NormalizationRule],
) -> RunnerResult<()> {
    let original_text = fs::read_to_string(original)
        .map_err(|err| RunnerError::io("E_IO", "failed to read transcript", err))?;
    let replay_text = fs::read_to_string(replay)
        .map_err(|err| RunnerError::io("E_IO", "failed to read replay transcript", err))?;
    let original_text =
        apply_rules_to_text(original_text, rules, NormalizationRuleTarget::Transcript);
    let replay_text = apply_rules_to_text(replay_text, rules, NormalizationRuleTarget::Transcript);
    if original_text != replay_text {
        return Err(RunnerError::policy_denied(
            "E_REPLAY_MISMATCH",
            "transcript mismatch",
            serde_json::json!({ "kind": "transcript" }),
        ));
    }
    Ok(())
}

fn compare_run_results(
    original: &Path,
    replay: &Path,
    filters: &[NormalizationFilter],
    rules: &[NormalizationRule],
) -> RunnerResult<()> {
    let mut original_value = load_run_value(original)?;
    let mut replay_value = load_run_value(replay)?;
    normalize_run_value(&mut original_value, filters, rules);
    normalize_run_value(&mut replay_value, filters, rules);
    if original_value != replay_value {
        return Err(RunnerError::policy_denied(
            "E_REPLAY_MISMATCH",
            "run result mismatch",
            serde_json::json!({ "kind": "run_result" }),
        ));
    }
    Ok(())
}

fn load_run_value(path: &Path) -> RunnerResult<Value> {
    let data = fs::read_to_string(path)
        .map_err(|err| RunnerError::io("E_IO", "failed to read run result", err))?;
    let value: Value = serde_json::from_str(&data)
        .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to parse run result", err))?;
    Ok(value)
}

fn normalize_run_value(
    value: &mut Value,
    filters: &[NormalizationFilter],
    rules: &[NormalizationRule],
) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    if has_filter(filters, NormalizationFilter::RunId) {
        obj.remove("run_id");
    }
    if has_filter(filters, NormalizationFilter::RunTimestamps) {
        obj.remove("started_at_ms");
        obj.remove("ended_at_ms");
    }
    if has_filter(filters, NormalizationFilter::StepTimestamps) {
        if let Some(steps) = obj.get_mut("steps").and_then(|val| val.as_array_mut()) {
            for step in steps {
                if let Some(step_obj) = step.as_object_mut() {
                    step_obj.remove("started_at_ms");
                    step_obj.remove("ended_at_ms");
                }
            }
        }
    }
    if let Some(final_obs) = obj
        .get_mut("final_observation")
        .and_then(|val| val.as_object_mut())
    {
        if has_filter(filters, NormalizationFilter::RunId) {
            final_obs.remove("run_id");
        }
        if has_filter(filters, NormalizationFilter::SessionId) {
            final_obs.remove("session_id");
        }
        if has_filter(filters, NormalizationFilter::ObservationTimestamp) {
            final_obs.remove("timestamp_ms");
        }
        if let Some(transcript) = final_obs
            .get("transcript_delta")
            .and_then(|val| val.as_str())
        {
            let normalized = apply_rules_to_text(
                transcript.to_string(),
                rules,
                NormalizationRuleTarget::Transcript,
            );
            final_obs.insert("transcript_delta".to_string(), Value::String(normalized));
        }
        if let Some(screen) = final_obs
            .get_mut("screen")
            .and_then(|val| val.as_object_mut())
        {
            if has_filter(filters, NormalizationFilter::SnapshotId) {
                screen.remove("snapshot_id");
            }
            apply_rules_to_snapshot_object(screen, rules);
        }
    }
}

fn has_filter(filters: &[NormalizationFilter], filter: NormalizationFilter) -> bool {
    filters.iter().any(|item| item == &filter)
}

fn compare_events(
    original: &Path,
    replay: &Path,
    filters: &[NormalizationFilter],
    rules: &[NormalizationRule],
    require: bool,
) -> RunnerResult<()> {
    let original_events = load_events_if_present(original, filters, rules)?;
    let replay_events = load_events_if_present(replay, filters, rules)?;
    match (original_events, replay_events) {
        (None, None) => {
            if require {
                return Err(RunnerError::policy_denied(
                    "E_REPLAY_MISMATCH",
                    "event stream missing",
                    serde_json::json!({ "kind": "events" }),
                ));
            }
            Ok(())
        }
        (Some(_), None) | (None, Some(_)) => Err(RunnerError::policy_denied(
            "E_REPLAY_MISMATCH",
            "event stream presence mismatch",
            serde_json::json!({ "kind": "events" }),
        )),
        (Some(original_events), Some(replay_events)) => {
            if original_events.len() != replay_events.len() {
                return Err(RunnerError::policy_denied(
                    "E_REPLAY_MISMATCH",
                    "event stream length mismatch",
                    serde_json::json!({
                        "kind": "events",
                        "expected": original_events.len(),
                        "actual": replay_events.len(),
                    }),
                ));
            }
            for (idx, (left, right)) in original_events.iter().zip(replay_events.iter()).enumerate()
            {
                if left != right {
                    return Err(RunnerError::policy_denied(
                        "E_REPLAY_MISMATCH",
                        "event stream mismatch",
                        serde_json::json!({ "kind": "events", "index": idx }),
                    ));
                }
            }
            Ok(())
        }
    }
}

fn load_events_if_present(
    path: &Path,
    filters: &[NormalizationFilter],
    rules: &[NormalizationRule],
) -> RunnerResult<Option<Vec<Value>>> {
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read_to_string(path)
        .map_err(|err| RunnerError::io("E_IO", "failed to read events log", err))?;
    let mut events = Vec::new();
    for (line_no, line) in data.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let mut value: Value = serde_json::from_str(line).map_err(|err| {
            RunnerError::io(
                "E_PROTOCOL",
                format!("failed to parse event line {}", line_no + 1),
                err,
            )
        })?;
        normalize_observation_value(&mut value, filters, rules);
        events.push(value);
    }
    Ok(Some(events))
}

fn validate_checksums(dir: &Path, require: bool) -> RunnerResult<()> {
    let path = dir.join("checksums.json");
    if !path.exists() {
        if require {
            return Err(RunnerError::policy_denied(
                "E_REPLAY_MISMATCH",
                "checksums missing",
                serde_json::json!({ "kind": "checksum", "path": "checksums.json" }),
            ));
        }
        return Ok(());
    }
    let data = fs::read_to_string(&path)
        .map_err(|err| RunnerError::io("E_IO", "failed to read checksums", err))?;
    let checksums: std::collections::BTreeMap<String, String> = serde_json::from_str(&data)
        .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to parse checksums", err))?;
    for (relative, expected) in checksums {
        let file_path = dir.join(&relative);
        if !file_path.exists() {
            return Err(RunnerError::policy_denied(
                "E_REPLAY_MISMATCH",
                "checksum target missing",
                serde_json::json!({ "kind": "checksum", "path": relative }),
            ));
        }
        let actual = compute_checksum(&file_path)?;
        if actual != expected {
            return Err(RunnerError::policy_denied(
                "E_REPLAY_MISMATCH",
                "checksum mismatch",
                serde_json::json!({
                    "kind": "checksum",
                    "path": relative,
                    "expected": expected,
                    "actual": actual
                }),
            ));
        }
    }
    Ok(())
}

fn compute_checksum(path: &Path) -> RunnerResult<String> {
    let data = fs::read(path).map_err(|err| RunnerError::io("E_IO", "failed to read file", err))?;
    Ok(format!("{:016x}", fnv1a_hash(&data)))
}

fn update_checksum_entry(dir: &Path, relative: &str) -> RunnerResult<()> {
    let path = dir.join("checksums.json");
    if !path.exists() {
        return Ok(());
    }
    let data = fs::read_to_string(&path)
        .map_err(|err| RunnerError::io("E_IO", "failed to read checksums", err))?;
    let mut checksums: std::collections::BTreeMap<String, String> = serde_json::from_str(&data)
        .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to parse checksums", err))?;
    let checksum = compute_checksum(&dir.join(relative))?;
    checksums.insert(relative.to_string(), checksum);
    let data = serde_json::to_vec_pretty(&checksums)
        .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to serialize checksums", err))?;
    fs::write(&path, data)
        .map_err(|err| RunnerError::io("E_IO", "failed to write checksums", err))?;
    Ok(())
}

fn fnv1a_hash(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in data {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn normalize_observation_value(
    value: &mut Value,
    filters: &[NormalizationFilter],
    rules: &[NormalizationRule],
) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    if let Some(transcript) = obj.get("transcript_delta").and_then(|val| val.as_str()) {
        let normalized = apply_rules_to_text(
            transcript.to_string(),
            rules,
            NormalizationRuleTarget::Transcript,
        );
        obj.insert("transcript_delta".to_string(), Value::String(normalized));
    }
    if has_filter(filters, NormalizationFilter::RunId) {
        obj.remove("run_id");
    }
    if has_filter(filters, NormalizationFilter::SessionId) {
        obj.remove("session_id");
    }
    if has_filter(filters, NormalizationFilter::ObservationTimestamp) {
        obj.remove("timestamp_ms");
    }
    if let Some(screen) = obj.get_mut("screen").and_then(|val| val.as_object_mut()) {
        if has_filter(filters, NormalizationFilter::SnapshotId) {
            screen.remove("snapshot_id");
        }
        apply_rules_to_snapshot_object(screen, rules);
    }
}

fn mismatch_from_error(err: &RunnerError) -> Option<ReplayMismatch> {
    let context = err.context.as_ref()?;
    let kind = context
        .get("kind")
        .and_then(|val| val.as_str())
        .unwrap_or("unknown")
        .to_string();
    let index = context
        .get("index")
        .and_then(|val| val.as_u64())
        .map(|val| val as usize);
    Some(ReplayMismatch { kind, index })
}
