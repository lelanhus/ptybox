//! Interactive NDJSON driver loop for agent-controlled TUI sessions.
//!
//! This module implements the protocol v2 driver, which reads
//! [`DriverRequestV2`](crate::model::driver::DriverRequestV2) messages from
//! stdin and writes [`DriverResponseV2`](crate::model::driver::DriverResponseV2)
//! responses to stdout. Each request sends one action and receives one
//! observation with correlated `request_id`.
//!
//! # Key Types
//!
//! - [`DriverConfig`] — Runtime configuration (command, policy, artifacts)
//!
//! # Key Functions
//!
//! - [`run_driver`] — Start the stdin/stdout driver loop
//!
//! # Protocol Flow
//!
//! 1. Client sends a JSON line: `{"protocol_version":2, "request_id":"req-1", "action":{...}}`
//! 2. Driver validates the request, performs the action, observes the terminal
//! 3. Driver writes a JSON line response with the observation or error
//! 4. Loop ends when client sends `terminate` or an error occurs
//!
//! # Artifacts
//!
//! When artifacts are enabled, the driver writes:
//! - `driver-actions.jsonl` — log of all actions with timing
//! - `scenario.json` — generated scenario from the action sequence
//! - Standard artifacts (snapshots, transcript, events, run.json, checksums)

use crate::artifacts::{ArtifactsWriter, ArtifactsWriterConfig};
use crate::model::policy::Policy;
use crate::model::{
    driver::{
        BudgetStatus, DriverActionMetrics, DriverActionRecord, DriverRequestV2,
        DriverResponseStatus, DriverResponseV2,
    },
    Action, ActionType, ErrorInfo, NormalizationRecord, RunConfig, RunId, RunResult, RunStatus,
    Scenario, ScenarioMetadata, Step, StepId, StepResult, StepStatus, TerminalSize,
    NORMALIZATION_VERSION, PROTOCOL_VERSION, RUN_RESULT_VERSION, SCENARIO_VERSION,
};
use crate::policy::{
    validate_artifacts_dir, validate_artifacts_policy, validate_policy, validate_write_access,
    EffectivePolicy,
};
use crate::runner::{compile_safe_regex, RunnerError, RunnerResult};
use crate::session::{Session, SessionConfig};
use crate::util::{
    build_spawn_command, convert_exit_status, elapsed_ms, pause_until, resolve_artifacts_config,
    snapshot_bytes, SandboxCleanupGuard,
};
use serde::Deserialize;
use serde_json::Value;
use std::io::{self, BufRead, Write};
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
///
/// Validates the full policy, spawns the child process under sandbox,
/// then enters the request-response loop. The loop terminates when:
/// - The client sends a `terminate` action
/// - A protocol error occurs (invalid JSON, version mismatch)
/// - A budget is exceeded (runtime, steps, output, snapshot)
/// - The child process exits unexpectedly
///
/// # Errors
///
/// Returns [`RunnerError`] with codes including:
/// - `E_POLICY_DENIED` — Policy validation failed before spawning
/// - `E_PROTOCOL` — Invalid request JSON or payload
/// - `E_PROTOCOL_VERSION_MISMATCH` — Unsupported protocol version
/// - `E_TIMEOUT` — Budget exceeded (runtime, steps, output, snapshot, wait)
/// - `E_PROCESS_EXIT` — Child process exited during a wait condition
/// - `E_IO` — I/O failure on stdin/stdout or artifact writes
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
    const MAX_CONSECUTIVE_PARSE_ERRORS: u32 = 5;

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

    // Emit handshake so agents know protocol capabilities upfront
    let handshake = serde_json::json!({
        "type": "handshake",
        "protocol_version": PROTOCOL_VERSION,
        "run_id": run_id.to_string(),
        "terminal_size": {
            "rows": TerminalSize::default().rows,
            "cols": TerminalSize::default().cols,
        },
        "budgets": {
            "max_steps": policy.budgets.max_steps,
            "max_runtime_ms": policy.budgets.max_runtime_ms,
            "max_output_bytes": policy.budgets.max_output_bytes,
            "max_snapshot_bytes": policy.budgets.max_snapshot_bytes,
            "max_wait_ms": policy.budgets.max_wait_ms,
        },
        "supported_actions": ["key", "text", "resize", "wait", "observe", "terminate"],
        "supported_conditions": ["screen_contains", "screen_matches", "cursor_at", "process_exited"],
    });
    let handshake_str = serde_json::to_string(&handshake)
        .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to serialize handshake", err))?;
    writeln!(output, "{handshake_str}")
        .map_err(|err| RunnerError::io("E_IO", "failed to write handshake", err))?;
    output
        .flush()
        .map_err(|err| RunnerError::io("E_IO", "failed to flush handshake", err))?;

    let mut output_bytes: u64 = 0;
    let mut sequence: u64 = 0;
    let mut scenario_steps: Vec<Step> = Vec::new();
    let mut step_results: Vec<StepResult> = Vec::new();
    let mut final_observation = None;
    let mut final_error: Option<RunnerError> = None;
    let mut consecutive_parse_errors: u32 = 0;

    for line in input.lines() {
        let line =
            line.map_err(|err| RunnerError::io("E_IO", "failed to read driver input", err))?;
        if line.trim().is_empty() {
            continue;
        }

        let request: DriverRequestV2 = match serde_json::from_str(&line) {
            Ok(req) => {
                consecutive_parse_errors = 0;
                req
            }
            Err(err) => {
                consecutive_parse_errors += 1;
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
                            "hint": "request must be DriverRequestV2: protocol_version, request_id, action, timeout_ms?",
                            "consecutive_errors": consecutive_parse_errors,
                            "max_consecutive_errors": MAX_CONSECUTIVE_PARSE_ERRORS
                        })),
                    }),
                    action_metrics: None,
                    budget_status: None,
                };
                emit_driver_response(&mut output, &response)?;
                if consecutive_parse_errors >= MAX_CONSECUTIVE_PARSE_ERRORS {
                    final_error = Some(RunnerError::protocol(
                        "E_PROTOCOL",
                        format!(
                            "too many consecutive parse errors ({consecutive_parse_errors}), terminating driver"
                        ),
                        None,
                    ));
                    break;
                }
                continue;
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
                budget_status: None,
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
                budget_status: Some(make_budget_status(
                    sequence,
                    &policy,
                    &run_started,
                    output_bytes,
                )),
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
                budget_status: Some(make_budget_status(
                    sequence,
                    &policy,
                    &run_started,
                    output_bytes,
                )),
            };
            emit_driver_response(&mut output, &response)?;
            final_error = Some(RunnerError::timeout(
                "E_TIMEOUT",
                "run exceeded max runtime budget",
                Some(serde_json::json!({ "max_runtime_ms": policy.budgets.max_runtime_ms })),
            ));
            break;
        }

        let action = request.action.clone();
        let default_timeout_ms = if matches!(action.action_type, ActionType::Wait) {
            5000
        } else {
            200
        };
        let timeout_ms = request.timeout_ms.unwrap_or(default_timeout_ms);
        let started_at_ms = elapsed_ms(&run_started);
        let action_started = Instant::now();
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
                        duration_ms: elapsed_ms(&action_started),
                    }),
                    budget_status: Some(make_budget_status(
                        sequence,
                        &policy,
                        &run_started,
                        output_bytes,
                    )),
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
                    duration_ms: elapsed_ms(&action_started),
                }),
                budget_status: Some(make_budget_status(
                    sequence,
                    &policy,
                    &run_started,
                    output_bytes,
                )),
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
                    duration_ms: elapsed_ms(&action_started),
                }),
                budget_status: Some(make_budget_status(
                    sequence,
                    &policy,
                    &run_started,
                    output_bytes,
                )),
            };
            emit_driver_response(&mut output, &response)?;
            final_error = Some(err);
            break;
        }

        sequence += 1;
        let ended_at_ms = elapsed_ms(&run_started);
        let duration_ms = elapsed_ms(&action_started);

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
            budget_status: Some(make_budget_status(
                sequence,
                &policy,
                &run_started,
                output_bytes,
            )),
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
        ActionType::Observe => session.observe(timeout),
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

    // Validate required fields before entering the polling loop
    if wait_payload.condition.condition_type == "screen_contains"
        && wait_payload
            .condition
            .payload
            .get("text")
            .and_then(Value::as_str)
            .is_none()
    {
        return Err(RunnerError::protocol(
            "E_PROTOCOL",
            "screen_contains condition requires 'text' field",
            Some(serde_json::json!({ "received_payload": wait_payload.condition.payload })),
        ));
    }

    let compiled_regex = if wait_payload.condition.condition_type == "screen_matches" {
        let pattern = wait_payload
            .condition
            .payload
            .get("pattern")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RunnerError::protocol(
                    "E_PROTOCOL",
                    "screen_matches condition requires 'pattern' field",
                    Some(serde_json::json!({ "received_payload": wait_payload.condition.payload })),
                )
            })?;
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
            // text field is validated before the polling loop
            let text = condition
                .payload
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("");
            Ok(observation.screen.lines.join("\n").contains(text))
        }
        "screen_matches" => {
            // pattern field is validated and compiled before the polling loop
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
            let row_u64 = condition
                .payload
                .get("row")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    RunnerError::protocol(
                        "E_PROTOCOL",
                        "cursor_at condition requires 'row' field",
                        Some(serde_json::json!({ "received_payload": condition.payload })),
                    )
                })?;
            let col_u64 = condition
                .payload
                .get("col")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    RunnerError::protocol(
                        "E_PROTOCOL",
                        "cursor_at condition requires 'col' field",
                        Some(serde_json::json!({ "received_payload": condition.payload })),
                    )
                })?;
            let row = u16::try_from(row_u64).map_err(|_| {
                RunnerError::protocol(
                    "E_PROTOCOL",
                    format!("row value {row_u64} exceeds maximum u16 value {}", u16::MAX),
                    Some(serde_json::json!({ "received": row_u64, "max": u16::MAX })),
                )
            })?;
            let col = u16::try_from(col_u64).map_err(|_| {
                RunnerError::protocol(
                    "E_PROTOCOL",
                    format!("col value {col_u64} exceeds maximum u16 value {}", u16::MAX),
                    Some(serde_json::json!({ "received": col_u64, "max": u16::MAX })),
                )
            })?;
            Ok(observation.screen.cursor.row == row && observation.screen.cursor.col == col)
        }
        "process_exited" => Ok(false),
        other => Err(RunnerError::protocol(
            "E_PROTOCOL",
            format!("unsupported wait condition '{other}'"),
            Some(serde_json::json!({
                "received": other,
                "supported_conditions": ["screen_contains", "screen_matches", "cursor_at", "process_exited"]
            })),
        )),
    }
}

fn make_budget_status(
    sequence: u64,
    policy: &Policy,
    run_started: &Instant,
    output_bytes: u64,
) -> BudgetStatus {
    BudgetStatus {
        steps_used: sequence,
        steps_max: policy.budgets.max_steps,
        runtime_ms: elapsed_ms(run_started),
        runtime_max_ms: policy.budgets.max_runtime_ms,
        output_bytes_used: output_bytes,
        output_bytes_max: policy.budgets.max_output_bytes,
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
