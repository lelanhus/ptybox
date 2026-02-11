//! Interactive driver loop for protocol v2 NDJSON clients.

use crate::artifacts::{ArtifactsWriter, ArtifactsWriterConfig};
use crate::model::policy::{Policy, SandboxMode};
use crate::model::{
    driver::{
        DriverActionMetrics, DriverActionRecord, DriverRequestV2, DriverResponseStatus,
        DriverResponseV2,
    },
    Action, ActionType, ErrorInfo, ExitStatus, NormalizationRecord, RunConfig, RunId, RunResult,
    RunStatus, Scenario, ScenarioMetadata, Step, StepId, StepResult, StepStatus, TerminalSize,
    NORMALIZATION_VERSION, PROTOCOL_VERSION, RUN_RESULT_VERSION, SCENARIO_VERSION,
};
use crate::policy::{
    sandbox, validate_artifacts_dir, validate_artifacts_policy, validate_policy,
    validate_write_access, EffectivePolicy,
};
use crate::runner::{compile_safe_regex, RunnerError, RunnerResult};
use crate::session::{Session, SessionConfig};
use serde::Deserialize;
use serde_json::Value;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Driver runtime configuration.
#[derive(Clone, Debug)]
pub struct DriverConfig {
    /// Command to execute.
    pub command: String,
    /// Command arguments.
    pub args: Vec<String>,
    /// Optional working directory.
    pub cwd: Option<String>,
    /// Security policy for the run.
    pub policy: Policy,
    /// Optional artifacts configuration.
    pub artifacts: Option<ArtifactsWriterConfig>,
}

/// Run the protocol v2 driver loop against stdin/stdout.
pub fn run_driver(config: DriverConfig) -> RunnerResult<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_driver_with_io(config, stdin.lock(), stdout.lock())
}

#[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
fn run_driver_with_io<R, W>(config: DriverConfig, input: R, mut output: W) -> RunnerResult<()>
where
    R: BufRead,
    W: Write,
{
    let DriverConfig {
        command,
        args,
        cwd,
        policy,
        artifacts,
    } = config;

    validate_policy(&policy)?;
    validate_artifacts_policy(&policy)?;
    let effective_policy = EffectivePolicy::new(policy.clone());
    let run_config = RunConfig {
        command: command.clone(),
        args: args.clone(),
        cwd: cwd.clone(),
        initial_size: TerminalSize::default(),
        policy: crate::model::scenario::PolicyRef::Inline(Box::new(policy.clone())),
    };
    effective_policy.validate_run_config(&run_config)?;

    let artifacts_config = resolve_artifacts_config(&policy, artifacts);
    let artifacts_dir = artifacts_config.as_ref().map(|cfg| cfg.dir.clone());
    validate_write_access(&policy, artifacts_dir.as_deref())?;
    if let Some(cfg) = artifacts_config.as_ref() {
        validate_artifacts_dir(&cfg.dir, &policy.fs)?;
    }

    let run_id = RunId::new();
    let run_started = Instant::now();
    let mut writer = if let Some(cfg) = artifacts_config {
        Some(ArtifactsWriter::new(run_id, cfg)?)
    } else {
        None
    };
    if let Some(writer) = writer.as_mut() {
        writer.write_policy(&policy)?;
        writer.write_normalization(&NormalizationRecord {
            normalization_version: NORMALIZATION_VERSION,
            filters: Vec::new(),
            strict: false,
            source: crate::model::NormalizationSource::None,
            rules: Vec::new(),
        })?;
    }

    let spawn = build_spawn_command(&policy, &command, &args, artifacts_dir.as_ref(), run_id)?;
    let cleanup_guard = SandboxCleanupGuard::new(spawn.cleanup_path.clone());

    let mut session = Session::spawn(SessionConfig {
        command: spawn.command,
        args: spawn.args,
        cwd: cwd.clone(),
        size: TerminalSize::default(),
        run_id,
        env: policy.env.clone(),
    })?;

    let mut output_bytes: u64 = 0;
    let mut sequence: u64 = 0;
    let mut scenario_steps: Vec<Step> = Vec::new();
    let mut step_results: Vec<StepResult> = Vec::new();
    let mut final_observation = None;
    let mut final_error: Option<RunnerError> = None;

    for line in input.lines() {
        let line =
            line.map_err(|err| RunnerError::io("E_IO", "failed to read driver input", err))?;
        if line.trim().is_empty() {
            continue;
        }

        let request: DriverRequestV2 = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(err) => {
                let response = DriverResponseV2 {
                    protocol_version: PROTOCOL_VERSION,
                    request_id: "unknown".to_string(),
                    status: DriverResponseStatus::Error,
                    observation: None,
                    error: Some(ErrorInfo {
                        code: "E_PROTOCOL".to_string(),
                        message: "invalid json request".to_string(),
                        context: Some(serde_json::json!({
                            "parse_error": err.to_string(),
                            "received": line.chars().take(200).collect::<String>(),
                            "hint": "request must be DriverRequestV2: protocol_version, request_id, action, timeout_ms?"
                        })),
                    }),
                    action_metrics: None,
                };
                emit_driver_response(&mut output, &response)?;
                let err = RunnerError::protocol("E_PROTOCOL", "invalid json request", None);
                final_error = Some(err);
                break;
            }
        };

        if request.protocol_version != PROTOCOL_VERSION {
            let response = DriverResponseV2 {
                protocol_version: PROTOCOL_VERSION,
                request_id: request.request_id.clone(),
                status: DriverResponseStatus::Error,
                observation: None,
                error: Some(ErrorInfo {
                    code: "E_PROTOCOL_VERSION_MISMATCH".to_string(),
                    message: "unsupported protocol version".to_string(),
                    context: Some(serde_json::json!({
                        "provided_version": request.protocol_version,
                        "supported_version": PROTOCOL_VERSION
                    })),
                }),
                action_metrics: None,
            };
            emit_driver_response(&mut output, &response)?;
            final_error = Some(RunnerError::protocol_version_mismatch(
                "unsupported protocol version",
            ));
            break;
        }

        if sequence >= policy.budgets.max_steps {
            let response = DriverResponseV2 {
                protocol_version: PROTOCOL_VERSION,
                request_id: request.request_id.clone(),
                status: DriverResponseStatus::Error,
                observation: None,
                error: Some(ErrorInfo {
                    code: "E_TIMEOUT".to_string(),
                    message: "action budget exceeded".to_string(),
                    context: Some(serde_json::json!({ "max_steps": policy.budgets.max_steps })),
                }),
                action_metrics: None,
            };
            emit_driver_response(&mut output, &response)?;
            final_error = Some(RunnerError::timeout(
                "E_TIMEOUT",
                "action budget exceeded",
                Some(serde_json::json!({ "max_steps": policy.budgets.max_steps })),
            ));
            break;
        }

        if elapsed_ms(&run_started) > policy.budgets.max_runtime_ms {
            let response = DriverResponseV2 {
                protocol_version: PROTOCOL_VERSION,
                request_id: request.request_id.clone(),
                status: DriverResponseStatus::Error,
                observation: None,
                error: Some(ErrorInfo {
                    code: "E_TIMEOUT".to_string(),
                    message: "run exceeded max runtime budget".to_string(),
                    context: Some(serde_json::json!({
                        "max_runtime_ms": policy.budgets.max_runtime_ms
                    })),
                }),
                action_metrics: None,
            };
            emit_driver_response(&mut output, &response)?;
            final_error = Some(RunnerError::timeout(
                "E_TIMEOUT",
                "run exceeded max runtime budget",
                Some(serde_json::json!({ "max_runtime_ms": policy.budgets.max_runtime_ms })),
            ));
            break;
        }

        let timeout_ms = request.timeout_ms.unwrap_or(50);
        let started_at_ms = elapsed_ms(&run_started);
        let action_started = Instant::now();
        let action = request.action.clone();
        let observation = match perform_action(
            &mut session,
            &action,
            Duration::from_millis(timeout_ms),
            &policy,
        ) {
            Ok(obs) => obs,
            Err(err) => {
                let response = DriverResponseV2 {
                    protocol_version: PROTOCOL_VERSION,
                    request_id: request.request_id.clone(),
                    status: DriverResponseStatus::Error,
                    observation: None,
                    error: Some(err.to_error_info()),
                    action_metrics: Some(DriverActionMetrics {
                        sequence: sequence + 1,
                        duration_ms: elapsed_ms_from(&action_started),
                    }),
                };
                emit_driver_response(&mut output, &response)?;
                final_error = Some(err);
                break;
            }
        };

        output_bytes += observation
            .transcript_delta
            .as_ref()
            .map_or(0, |delta| delta.len() as u64);
        if output_bytes > policy.budgets.max_output_bytes {
            let err = RunnerError::timeout(
                "E_TIMEOUT",
                "output budget exceeded",
                Some(serde_json::json!({
                    "max_output_bytes": policy.budgets.max_output_bytes
                })),
            );
            let response = DriverResponseV2 {
                protocol_version: PROTOCOL_VERSION,
                request_id: request.request_id.clone(),
                status: DriverResponseStatus::Error,
                observation: None,
                error: Some(err.to_error_info()),
                action_metrics: Some(DriverActionMetrics {
                    sequence: sequence + 1,
                    duration_ms: elapsed_ms_from(&action_started),
                }),
            };
            emit_driver_response(&mut output, &response)?;
            final_error = Some(err);
            break;
        }
        if snapshot_bytes(&observation.screen)? > policy.budgets.max_snapshot_bytes {
            let err = RunnerError::timeout(
                "E_TIMEOUT",
                "snapshot budget exceeded",
                Some(serde_json::json!({
                    "max_snapshot_bytes": policy.budgets.max_snapshot_bytes
                })),
            );
            let response = DriverResponseV2 {
                protocol_version: PROTOCOL_VERSION,
                request_id: request.request_id.clone(),
                status: DriverResponseStatus::Error,
                observation: None,
                error: Some(err.to_error_info()),
                action_metrics: Some(DriverActionMetrics {
                    sequence: sequence + 1,
                    duration_ms: elapsed_ms_from(&action_started),
                }),
            };
            emit_driver_response(&mut output, &response)?;
            final_error = Some(err);
            break;
        }

        sequence += 1;
        let ended_at_ms = elapsed_ms(&run_started);
        let duration_ms = elapsed_ms_from(&action_started);

        let step_id = StepId::new();
        scenario_steps.push(Step {
            id: step_id,
            name: format!("driver-step-{sequence}"),
            action: action.clone(),
            assert: Vec::new(),
            timeout_ms,
            retries: 0,
        });
        step_results.push(StepResult {
            step_id,
            name: format!("driver-step-{sequence}"),
            status: StepStatus::Passed,
            attempts: 1,
            started_at_ms,
            ended_at_ms,
            action: action.clone(),
            assertions: Vec::new(),
            error: None,
        });

        if let Some(writer) = writer.as_mut() {
            writer.write_snapshot(&observation.screen)?;
            if let Some(delta) = &observation.transcript_delta {
                writer.write_transcript(delta)?;
            }
            writer.write_observation(&observation)?;
            writer.write_json_line(
                "driver-actions.jsonl",
                &DriverActionRecord {
                    sequence,
                    request_id: request.request_id.clone(),
                    action: action.clone(),
                    timeout_ms,
                    started_at_ms,
                    ended_at_ms,
                },
            )?;
        }

        let response = DriverResponseV2 {
            protocol_version: PROTOCOL_VERSION,
            request_id: request.request_id.clone(),
            status: DriverResponseStatus::Ok,
            observation: Some(observation.clone()),
            error: None,
            action_metrics: Some(DriverActionMetrics {
                sequence,
                duration_ms,
            }),
        };
        emit_driver_response(&mut output, &response)?;
        final_observation = Some(observation);

        if matches!(action.action_type, ActionType::Terminate) {
            break;
        }
    }

    if final_observation.is_none() {
        final_observation = session.observe(Duration::from_millis(10)).ok();
    }

    let exit_status = match session.wait_for_exit(Duration::from_millis(50)) {
        Ok(Some(status)) => Some(convert_exit_status(status, false)),
        Ok(None) | Err(_) => session
            .terminate_process_group(Duration::from_millis(200))
            .ok()
            .flatten()
            .map(|status| convert_exit_status(status, true)),
    };

    let status = if final_error.is_none() {
        RunStatus::Passed
    } else {
        RunStatus::Errored
    };
    let run_result = RunResult {
        run_result_version: RUN_RESULT_VERSION,
        protocol_version: PROTOCOL_VERSION,
        run_id,
        status,
        started_at_ms: 0,
        ended_at_ms: elapsed_ms(&run_started),
        command: command.clone(),
        args: args.clone(),
        cwd: cwd
            .clone()
            .or_else(|| policy.fs.working_dir.clone())
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|_| "<unknown>".to_string())
            }),
        policy: policy.clone(),
        scenario: Some(Scenario {
            scenario_version: SCENARIO_VERSION,
            metadata: ScenarioMetadata {
                name: "driver-session".to_string(),
                description: Some("generated from driver-actions.jsonl".to_string()),
            },
            run: RunConfig {
                command,
                args,
                cwd,
                initial_size: TerminalSize::default(),
                policy: crate::model::scenario::PolicyRef::Inline(Box::new(policy.clone())),
            },
            steps: scenario_steps,
        }),
        steps: Some(step_results),
        final_observation,
        exit_status,
        error: final_error.as_ref().map(RunnerError::to_error_info),
    };

    if let Some(writer) = writer.as_mut() {
        if let Some(observation) = run_result.final_observation.as_ref() {
            // Keep driver artifacts aligned with scenario-run artifacts, which
            // include a terminal final observation record at run completion.
            writer.write_observation(observation)?;
        }
        if let Some(scenario) = &run_result.scenario {
            writer.write_scenario(scenario)?;
        }
        writer.write_run_result(&run_result)?;
        writer.flush_checksums()?;
    }

    drop(cleanup_guard);
    if let Some(err) = final_error {
        return Err(err);
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct WaitPayload {
    condition: Condition,
}

#[derive(Debug, Deserialize)]
struct Condition {
    #[serde(rename = "type")]
    condition_type: String,
    #[serde(default)]
    payload: Value,
}

fn perform_action(
    session: &mut Session,
    action: &Action,
    timeout: Duration,
    policy: &Policy,
) -> RunnerResult<crate::model::Observation> {
    match action.action_type {
        ActionType::Wait => wait_for_condition(session, action, timeout, policy),
        ActionType::Terminate => {
            session.terminate()?;
            session.observe(Duration::from_millis(10))
        }
        _ => {
            session.send(action)?;
            session.observe(timeout)
        }
    }
}

fn wait_for_condition(
    session: &mut Session,
    action: &Action,
    timeout: Duration,
    policy: &Policy,
) -> RunnerResult<crate::model::Observation> {
    let wait_payload: WaitPayload =
        serde_json::from_value(action.payload.clone()).map_err(|err| {
            RunnerError::protocol(
                "E_PROTOCOL",
                "invalid wait action payload",
                Some(serde_json::json!({
                    "parse_error": err.to_string(),
                    "received_payload": action.payload,
                })),
            )
        })?;
    let max_wait = Duration::from_millis(policy.budgets.max_wait_ms);
    let wait_timeout = timeout.min(max_wait);
    let deadline = Instant::now() + wait_timeout;

    let compiled_regex = if wait_payload.condition.condition_type == "screen_matches" {
        let pattern = wait_payload
            .condition
            .payload
            .get("pattern")
            .and_then(Value::as_str)
            .unwrap_or("");
        Some(compile_safe_regex(pattern)?)
    } else {
        None
    };

    loop {
        if Instant::now() > deadline {
            return Err(RunnerError::timeout(
                "E_TIMEOUT",
                "wait condition timed out",
                Some(serde_json::json!({
                    "condition": wait_payload.condition.condition_type
                })),
            ));
        }

        let observation = session.observe(Duration::from_millis(50))?;
        if session.wait_for_exit(Duration::from_millis(0))?.is_some() {
            if wait_payload.condition.condition_type == "process_exited" {
                return Ok(observation);
            }
            return Err(RunnerError::process_exit(
                "E_PROCESS_EXIT",
                "process exited during wait",
            ));
        }

        if condition_satisfied(
            &observation,
            &wait_payload.condition,
            compiled_regex.as_ref(),
        )? {
            return Ok(observation);
        }
        pause_until(deadline, Duration::from_millis(10));
    }
}

fn condition_satisfied(
    observation: &crate::model::Observation,
    condition: &Condition,
    compiled_regex: Option<&regex::Regex>,
) -> RunnerResult<bool> {
    match condition.condition_type.as_str() {
        "screen_contains" => {
            let text = condition
                .payload
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("");
            Ok(observation.screen.lines.join("\n").contains(text))
        }
        "screen_matches" => {
            let screen_text = observation.screen.lines.join("\n");
            if let Some(re) = compiled_regex {
                Ok(re.is_match(&screen_text))
            } else {
                let pattern = condition
                    .payload
                    .get("pattern")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let re = compile_safe_regex(pattern)?;
                Ok(re.is_match(&screen_text))
            }
        }
        "cursor_at" => {
            #[allow(clippy::cast_possible_truncation)]
            let row = condition
                .payload
                .get("row")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u16;
            #[allow(clippy::cast_possible_truncation)]
            let col = condition
                .payload
                .get("col")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u16;
            Ok(observation.screen.cursor.row == row && observation.screen.cursor.col == col)
        }
        "process_exited" => Ok(false),
        other => Err(RunnerError::protocol(
            "E_PROTOCOL",
            format!("unsupported wait condition '{other}'"),
            None,
        )),
    }
}

fn emit_driver_response(output: &mut impl Write, response: &DriverResponseV2) -> RunnerResult<()> {
    let payload = serde_json::to_string(response)
        .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to serialize driver response", err))?;
    writeln!(output, "{payload}")
        .map_err(|err| RunnerError::io("E_IO", "failed to write driver response", err))?;
    output
        .flush()
        .map_err(|err| RunnerError::io("E_IO", "failed to flush driver response", err))?;
    Ok(())
}

fn resolve_artifacts_config(
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

fn elapsed_ms(started_at: &Instant) -> u64 {
    #[allow(clippy::cast_possible_truncation)]
    let value = started_at.elapsed().as_millis() as u64;
    value
}

fn elapsed_ms_from(started_at: &Instant) -> u64 {
    #[allow(clippy::cast_possible_truncation)]
    let value = started_at.elapsed().as_millis() as u64;
    value
}

fn snapshot_bytes(snapshot: &crate::model::ScreenSnapshot) -> RunnerResult<u64> {
    let data = serde_json::to_vec(snapshot)
        .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to encode snapshot", err))?;
    Ok(data.len() as u64)
}

fn pause_until(deadline: Instant, max_step: Duration) {
    let now = Instant::now();
    if now >= deadline {
        return;
    }
    let remaining = deadline.saturating_duration_since(now);
    if remaining <= Duration::from_micros(500) {
        std::thread::yield_now();
        return;
    }
    let step = remaining.min(max_step);
    std::thread::sleep(step);
}

fn convert_exit_status(
    status: portable_pty::ExitStatus,
    terminated_by_harness: bool,
) -> ExitStatus {
    #[allow(clippy::cast_possible_wrap)]
    let code = status.exit_code() as i32;
    ExitStatus {
        success: status.success(),
        exit_code: Some(code),
        signal: None,
        terminated_by_harness,
    }
}

struct SpawnCommand {
    command: String,
    args: Vec<String>,
    cleanup_path: Option<PathBuf>,
}

fn build_spawn_command(
    policy: &Policy,
    command: &str,
    args: &[String],
    artifacts_dir: Option<&PathBuf>,
    run_id: RunId,
) -> RunnerResult<SpawnCommand> {
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

struct SandboxCleanupGuard {
    path: Option<PathBuf>,
}

impl SandboxCleanupGuard {
    fn new(path: Option<PathBuf>) -> Self {
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
