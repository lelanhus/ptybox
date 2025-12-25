pub mod progress;

use crate::artifacts::{ArtifactsWriter, ArtifactsWriterConfig};
use crate::model::policy::Policy;
use crate::model::{
    Action, ActionType, AssertionResult, ExitStatus, NormalizationRecord, RunConfig, RunId,
    RunResult, RunStatus, Scenario, StepResult, StepStatus, TerminalSize, NORMALIZATION_VERSION,
    PROTOCOL_VERSION,
};
use crate::policy::{
    sandbox, validate_artifacts_dir, validate_artifacts_policy, validate_env_policy,
    validate_fs_policy, validate_network_policy, validate_policy_version, validate_sandbox_mode,
    validate_write_access, EffectivePolicy,
};
use crate::scenario::load_policy_ref;
use crate::session::{Session, SessionConfig};
use miette::Diagnostic;
pub use progress::{NoopProgress, ProgressCallback, ProgressEvent};
use serde::Deserialize;
use serde_json::Value;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Result type alias for runner operations.
pub type RunnerResult<T> = Result<T, RunnerError>;

/// Error type for runner operations with stable error codes.
///
/// Error codes are stable and map to specific exit codes for automation.
/// See `spec/data-model.md` for the complete list.
///
/// # Common Error Codes
/// - `E_POLICY_DENIED` (exit 2): Policy validation failed
/// - `E_TIMEOUT` (exit 4): Budget exceeded
/// - `E_ASSERTION_FAILED` (exit 5): Assertion did not pass
/// - `E_PROCESS_EXIT` (exit 6): Process exited unexpectedly
/// - `E_IO` (exit 10): I/O failure
#[derive(Debug)]
pub struct RunnerError {
    /// Stable error code (e.g., `E_POLICY_DENIED`).
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Structured context for debugging.
    pub context: Option<Value>,
}

impl RunnerError {
    /// Create a policy denied error.
    pub fn policy_denied(
        code: impl Into<String>,
        message: impl Into<String>,
        context: impl Into<Option<Value>>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            context: context.into(),
        }
    }

    /// Create a timeout error (budget exceeded).
    pub fn timeout(
        code: impl Into<String>,
        message: impl Into<String>,
        context: impl Into<Option<Value>>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            context: context.into(),
        }
    }

    /// Create a protocol error (invalid JSON or version mismatch).
    pub fn protocol(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            context: None,
        }
    }

    pub fn protocol_with_context(
        code: impl Into<String>,
        message: impl Into<String>,
        context: impl Into<Option<Value>>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            context: context.into(),
        }
    }

    /// Create an I/O error.
    pub fn io(
        code: impl Into<String>,
        message: impl Into<String>,
        err: impl std::fmt::Display,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            context: Some(serde_json::json!({ "source": err.to_string() })),
        }
    }

    pub fn terminal_parse(
        code: impl Into<String>,
        message: impl Into<String>,
        err: impl std::fmt::Display,
        valid_up_to: Option<usize>,
    ) -> Self {
        let mut context = serde_json::Map::new();
        context.insert("source".to_string(), Value::String(err.to_string()));
        if let Some(valid_up_to) = valid_up_to {
            context.insert("valid_up_to".to_string(), Value::Number(valid_up_to.into()));
        }
        Self {
            code: code.into(),
            message: message.into(),
            context: Some(Value::Object(context)),
        }
    }

    pub fn internal(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            context: None,
        }
    }

    pub fn process_exit(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            context: None,
        }
    }

    pub fn to_error_info(&self) -> crate::model::ErrorInfo {
        crate::model::ErrorInfo {
            code: self.code.clone(),
            message: self.message.clone(),
            context: self.context.clone(),
        }
    }
}

impl fmt::Display for RunnerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for RunnerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl Diagnostic for RunnerError {}

/// Options for configuring scenario execution.
#[derive(Clone, Default)]
pub struct RunnerOptions {
    /// Artifacts writer configuration.
    pub artifacts: Option<ArtifactsWriterConfig>,
    /// Progress callback for receiving execution events.
    pub progress: Option<Arc<dyn ProgressCallback>>,
}

impl std::fmt::Debug for RunnerOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunnerOptions")
            .field("artifacts", &self.artifacts)
            .field("progress", &self.progress.as_ref().map(|_| "..."))
            .finish()
    }
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

/// Emit a progress event if a callback is configured.
fn emit_progress(progress: Option<&Arc<dyn ProgressCallback>>, event: ProgressEvent) {
    if let Some(callback) = progress {
        callback.on_progress(&event);
    }
}

/// Run a scenario with the given options.
///
/// This is the primary entry point for scenario-based execution.
/// For simple command execution without scenarios, use [`run_exec`].
///
/// # Arguments
/// * `scenario` - Scenario to execute (from file or constructed)
/// * `options` - Runner configuration (artifacts, progress callback)
///
/// # Errors
/// Returns `RunnerError` for policy violations, timeouts, I/O failures, etc.
/// The error code indicates the specific failure type.
pub fn run_scenario(scenario: Scenario, options: RunnerOptions) -> RunnerResult<RunResult> {
    let run_id = RunId::new();
    let run_started = Instant::now();
    let scenario_clone = scenario.clone();
    let progress = options.progress.clone();

    // Emit run started event
    emit_progress(
        progress.as_ref(),
        ProgressEvent::RunStarted {
            run_id,
            total_steps: scenario.steps.len(),
        },
    );

    let mut artifacts: Option<ArtifactsWriter> = None;

    let mut policy_for_error: Option<Policy> = None;
    let mut cleanup_path: Option<PathBuf> = None;

    let result: RunnerResult<RunResult> = (|| {
        let policy = load_policy_ref(&scenario.run.policy)?;
        policy_for_error = Some(policy.clone());

        validate_fs_policy(&policy.fs, policy.fs_write_unsafe_ack)?;
        validate_artifacts_policy(&policy)?;

        let artifacts_config = resolve_artifacts_config(&policy, options.artifacts);
        let artifacts_dir = artifacts_config.as_ref().map(|config| config.dir.clone());

        validate_write_access(&policy, artifacts_dir.as_deref())?;

        if let Some(config) = artifacts_config.clone() {
            validate_artifacts_dir(&config.dir, &policy.fs)?;
            let mut writer = ArtifactsWriter::new(run_id, config)?;
            writer.write_normalization(&NormalizationRecord {
                normalization_version: NORMALIZATION_VERSION,
                filters: Vec::new(),
                strict: false,
                source: crate::model::NormalizationSource::None,
                rules: Vec::new(),
            })?;
            artifacts = Some(writer);
        }

        if let Some(writer) = artifacts.as_mut() {
            writer.write_scenario(&scenario_clone)?;
        }

        validate_policy(&policy)?;

        if scenario.steps.len() as u64 > policy.budgets.max_steps {
            return Err(RunnerError::timeout(
                "E_TIMEOUT",
                "scenario exceeds max_steps budget",
                serde_json::json!({"max_steps": policy.budgets.max_steps}),
            ));
        }

        let effective_policy = EffectivePolicy::new(policy.clone());
        effective_policy.validate_run_config(&scenario.run)?;

        if let Some(writer) = artifacts.as_mut() {
            writer.write_policy(&policy)?;
        }

        let cwd = scenario
            .run
            .cwd
            .clone()
            .or_else(|| policy.fs.working_dir.clone());

        let spawn = build_spawn_command(
            &policy,
            &scenario.run.command,
            &scenario.run.args,
            artifacts_dir.as_ref(),
            run_id,
        )?;
        cleanup_path = spawn.cleanup_path.clone();

        let session_config = SessionConfig {
            command: spawn.command,
            args: spawn.args,
            cwd,
            size: scenario.run.initial_size.clone(),
            run_id,
            env: policy.env.clone(),
        };

        let mut session = Session::spawn(session_config)?;
        let mut step_results = Vec::new();
        let mut run_error: Option<RunnerError> = None;
        let mut output_bytes: u64 = 0;

        for (step_index, step) in scenario.steps.iter().enumerate() {
            if run_error.is_some() {
                step_results.push(StepResult {
                    step_id: step.id,
                    name: step.name.clone(),
                    status: StepStatus::Skipped,
                    attempts: 0,
                    started_at_ms: elapsed_ms(&run_started),
                    ended_at_ms: elapsed_ms(&run_started),
                    action: step.action.clone(),
                    assertions: Vec::new(),
                    error: None,
                });
                continue;
            }

            if elapsed_ms(&run_started) > policy.budgets.max_runtime_ms {
                run_error = Some(RunnerError::timeout(
                    "E_TIMEOUT",
                    "run exceeded max runtime budget",
                    serde_json::json!({"max_runtime_ms": policy.budgets.max_runtime_ms}),
                ));
                step_results.push(StepResult {
                    step_id: step.id,
                    name: step.name.clone(),
                    status: StepStatus::Skipped,
                    attempts: 0,
                    started_at_ms: elapsed_ms(&run_started),
                    ended_at_ms: elapsed_ms(&run_started),
                    action: step.action.clone(),
                    assertions: Vec::new(),
                    error: run_error.as_ref().map(|err| err.to_error_info()),
                });
                continue;
            }

            // Emit step started event (1-based index for display)
            emit_progress(
                progress.as_ref(),
                ProgressEvent::StepStarted {
                    step_id: step.id,
                    step_index: step_index + 1,
                    name: step.name.clone(),
                },
            );

            let step_started_ms = elapsed_ms(&run_started);
            let mut attempts = 0;
            let mut last_error: Option<RunnerError> = None;
            let mut status = StepStatus::Failed;
            let mut assertion_results = Vec::new();

            for _ in 0..=step.retries {
                attempts += 1;
                effective_policy.validate_action(&step.action)?;

                let observation = perform_action(
                    &mut session,
                    &step.action,
                    Duration::from_millis(step.timeout_ms),
                    &policy,
                );

                let observation = match observation {
                    Ok(obs) => obs,
                    Err(err) => {
                        if err.code == "E_TIMEOUT" {
                            last_error = Some(with_step_timeout_context(err, step));
                        } else {
                            last_error = Some(err);
                        }
                        status = StepStatus::Errored;
                        continue;
                    }
                };

                output_bytes += observation
                    .transcript_delta
                    .as_ref()
                    .map(|s| s.len() as u64)
                    .unwrap_or(0);
                if output_bytes > policy.budgets.max_output_bytes {
                    last_error = Some(RunnerError::timeout(
                        "E_TIMEOUT",
                        "output budget exceeded",
                        Some(step_context(
                            step,
                            Some(serde_json::json!({
                                "max_output_bytes": policy.budgets.max_output_bytes
                            })),
                        )),
                    ));
                    status = StepStatus::Errored;
                    break;
                }

                if snapshot_bytes(&observation.screen)? > policy.budgets.max_snapshot_bytes {
                    last_error = Some(RunnerError::timeout(
                        "E_TIMEOUT",
                        "snapshot budget exceeded",
                        Some(step_context(
                            step,
                            Some(serde_json::json!({
                                "max_snapshot_bytes": policy.budgets.max_snapshot_bytes
                            })),
                        )),
                    ));
                    status = StepStatus::Errored;
                    break;
                }

                if let Some(writer) = artifacts.as_mut() {
                    writer.write_snapshot(&observation.screen)?;
                    if let Some(delta) = &observation.transcript_delta {
                        writer.write_transcript(delta)?;
                    }
                    writer.write_observation(&observation)?;
                }

                let mut assertions_passed = true;
                assertion_results.clear();
                for assertion in &step.assert {
                    let (passed, message, details) =
                        crate::assertions::evaluate(&observation, assertion);
                    if !passed {
                        assertions_passed = false;
                    }
                    assertion_results.push(AssertionResult {
                        assertion_type: assertion.assertion_type.clone(),
                        passed,
                        message,
                        details,
                    });
                }

                if assertions_passed {
                    status = StepStatus::Passed;
                    last_error = None;
                    break;
                } else {
                    last_error = Some(RunnerError::policy_denied(
                        "E_ASSERTION_FAILED",
                        "one or more assertions failed",
                        None,
                    ));
                }
            }

            let step_ended_ms = elapsed_ms(&run_started);
            let error_info = last_error.as_ref().map(|err| err.to_error_info());
            if status != StepStatus::Passed {
                run_error = last_error.take();
            }
            step_results.push(StepResult {
                step_id: step.id,
                name: step.name.clone(),
                status: status.clone(),
                attempts,
                started_at_ms: step_started_ms,
                ended_at_ms: step_ended_ms,
                action: step.action.clone(),
                assertions: assertion_results.clone(),
                error: error_info,
            });

            // Emit step completed event
            emit_progress(
                progress.as_ref(),
                ProgressEvent::StepCompleted {
                    step_id: step.id,
                    name: step.name.clone(),
                    status,
                    duration_ms: step_ended_ms - step_started_ms,
                    assertions: assertion_results,
                },
            );
        }

        let final_observation = session.observe(Duration::from_millis(10)).ok();
        if let (Some(writer), Some(observation)) = (artifacts.as_mut(), final_observation.as_ref())
        {
            writer.write_observation(observation)?;
        }
        let exit_status = match run_error.as_ref() {
            Some(_) => session
                .terminate_process_group(Duration::from_millis(200))
                .ok()
                .flatten()
                .map(|status| {
                    // Exit codes are typically 0-255, safe to cast
                    #[allow(clippy::cast_possible_wrap)]
                    let code = status.exit_code() as i32;
                    ExitStatus {
                        success: status.success(),
                        exit_code: Some(code),
                        signal: None,
                        terminated_by_harness: true,
                    }
                }),
            None => {
                let max_runtime = Duration::from_millis(policy.budgets.max_runtime_ms);
                let elapsed = run_started.elapsed();
                if elapsed >= max_runtime {
                    let termination = session.terminate_process_group(Duration::from_millis(200));
                    let context = match termination {
                        Ok(_) => {
                            serde_json::json!({"max_runtime_ms": policy.budgets.max_runtime_ms})
                        }
                        Err(err) => serde_json::json!({
                            "max_runtime_ms": policy.budgets.max_runtime_ms,
                            "termination_error": err.to_string()
                        }),
                    };
                    return Err(RunnerError::timeout(
                        "E_TIMEOUT",
                        "run exceeded max runtime budget",
                        context,
                    ));
                }
                let remaining = max_runtime - elapsed;
                match session.wait_for_exit(remaining)? {
                    Some(status) => {
                        // Exit codes are typically 0-255, safe to cast
                        #[allow(clippy::cast_possible_wrap)]
                        let code = status.exit_code() as i32;
                        Some(ExitStatus {
                            success: status.success(),
                            exit_code: Some(code),
                            signal: None,
                            terminated_by_harness: false,
                        })
                    }
                    None => {
                        let termination =
                            session.terminate_process_group(Duration::from_millis(200));
                        let context = match termination {
                            Ok(_) => {
                                serde_json::json!({"max_runtime_ms": policy.budgets.max_runtime_ms})
                            }
                            Err(err) => serde_json::json!({
                                "max_runtime_ms": policy.budgets.max_runtime_ms,
                                "termination_error": err.to_string()
                            }),
                        };
                        return Err(RunnerError::timeout(
                            "E_TIMEOUT",
                            "run exceeded max runtime budget",
                            context,
                        ));
                    }
                }
            }
        };

        let ended_at = elapsed_ms(&run_started);

        let status = if step_results
            .iter()
            .all(|s| matches!(s.status, StepStatus::Passed))
        {
            RunStatus::Passed
        } else {
            RunStatus::Failed
        };

        let run_result = RunResult {
            run_result_version: 1,
            protocol_version: PROTOCOL_VERSION,
            run_id,
            status,
            started_at_ms: 0,
            ended_at_ms: ended_at,
            command: scenario.run.command.clone(),
            args: scenario.run.args.clone(),
            cwd: scenario
                .run
                .cwd
                .clone()
                .or_else(|| policy.fs.working_dir.clone())
                .unwrap_or_else(|| {
                    std::env::current_dir()
                        .unwrap_or_default()
                        .display()
                        .to_string()
                }),
            policy,
            scenario: Some(scenario),
            steps: Some(step_results),
            final_observation,
            exit_status,
            error: run_error.map(|err| err.to_error_info()),
        };

        if let Some(writer) = artifacts.as_mut() {
            writer.write_run_result(&run_result)?;
        }

        // Emit run completed event
        emit_progress(
            progress.as_ref(),
            ProgressEvent::RunCompleted {
                run_id,
                success: run_result.status == RunStatus::Passed,
                duration_ms: run_result.ended_at_ms,
            },
        );

        Ok(run_result)
    })();

    if let Err(err) = &result {
        // Emit run completed event for error case
        emit_progress(
            progress.as_ref(),
            ProgressEvent::RunCompleted {
                run_id,
                success: false,
                duration_ms: elapsed_ms(&run_started),
            },
        );

        if let Some(writer) = artifacts.as_mut() {
            let policy = policy_for_error.clone().unwrap_or_default();
            let _ = writer.write_policy(&policy);
            let run_result = RunResult {
                run_result_version: 1,
                protocol_version: PROTOCOL_VERSION,
                run_id,
                status: RunStatus::Errored,
                started_at_ms: 0,
                ended_at_ms: elapsed_ms(&run_started),
                command: scenario_clone.run.command.clone(),
                args: scenario_clone.run.args.clone(),
                cwd: scenario_clone
                    .run
                    .cwd
                    .clone()
                    .or_else(|| policy.fs.working_dir.clone())
                    .unwrap_or_else(|| {
                        std::env::current_dir()
                            .unwrap_or_default()
                            .display()
                            .to_string()
                    }),
                policy,
                scenario: Some(scenario_clone),
                steps: None,
                final_observation: None,
                exit_status: None,
                error: Some(err.to_error_info()),
            };
            let _ = writer.write_run_result(&run_result);
        }
    }

    cleanup_sandbox(cleanup_path);

    result
}

/// Run a single command under policy control.
///
/// Simpler alternative to [`run_scenario`] when you just need to execute
/// a command without step sequences or assertions.
///
/// # Arguments
/// * `command` - Command to execute (absolute path recommended)
/// * `args` - Command arguments
/// * `cwd` - Working directory (optional)
/// * `policy` - Security policy to apply
///
/// # Errors
/// Returns `RunnerError` for policy violations, timeouts, I/O failures, etc.
pub fn run_exec(
    command: String,
    args: Vec<String>,
    cwd: Option<String>,
    policy: Policy,
) -> RunnerResult<RunResult> {
    run_exec_with_options(command, args, cwd, policy, RunnerOptions::default())
}

/// Run a single command with custom options.
///
/// Like [`run_exec`] but with additional configuration options for
/// artifact collection and progress callbacks.
pub fn run_exec_with_options(
    command: String,
    args: Vec<String>,
    cwd: Option<String>,
    policy: Policy,
    options: RunnerOptions,
) -> RunnerResult<RunResult> {
    let run_id = RunId::new();
    let run_started = Instant::now();
    let mut artifacts: Option<ArtifactsWriter> = None;

    let mut cleanup_path: Option<PathBuf> = None;

    let result: RunnerResult<RunResult> = (|| {
        validate_fs_policy(&policy.fs, policy.fs_write_unsafe_ack)?;
        validate_artifacts_policy(&policy)?;

        let artifacts_config = resolve_artifacts_config(&policy, options.artifacts);
        let artifacts_dir = artifacts_config.as_ref().map(|config| config.dir.clone());

        validate_write_access(&policy, artifacts_dir.as_deref())?;

        if let Some(config) = artifacts_config.clone() {
            validate_artifacts_dir(&config.dir, &policy.fs)?;
            let mut writer = ArtifactsWriter::new(run_id, config)?;
            writer.write_normalization(&NormalizationRecord {
                normalization_version: NORMALIZATION_VERSION,
                filters: Vec::new(),
                strict: false,
                source: crate::model::NormalizationSource::None,
                rules: Vec::new(),
            })?;
            artifacts = Some(writer);
        }

        validate_policy(&policy)?;
        let effective_policy = EffectivePolicy::new(policy.clone());
        let cwd = cwd.clone().or_else(|| policy.fs.working_dir.clone());
        let run_config = RunConfig {
            command: command.clone(),
            args: args.clone(),
            cwd: cwd.clone(),
            initial_size: TerminalSize::default(),
            policy: crate::model::scenario::PolicyRef::Inline(policy.clone()),
        };
        effective_policy.validate_run_config(&run_config)?;

        if let Some(writer) = artifacts.as_mut() {
            writer.write_policy(&policy)?;
        }

        let spawn = build_spawn_command(&policy, &command, &args, artifacts_dir.as_ref(), run_id)?;
        cleanup_path = spawn.cleanup_path.clone();

        let mut session = Session::spawn(SessionConfig {
            command: spawn.command,
            args: spawn.args,
            cwd: cwd.clone(),
            size: TerminalSize::default(),
            run_id,
            env: policy.env.clone(),
        })?;

        let mut output_bytes: u64 = 0;
        let max_runtime = Duration::from_millis(policy.budgets.max_runtime_ms);
        let deadline = Instant::now() + max_runtime;

        let mut final_observation = session.observe(Duration::from_millis(50))?;
        enforce_exec_budgets(&mut session, &final_observation, &mut output_bytes, &policy)?;
        if let Some(writer) = artifacts.as_mut() {
            writer.write_observation(&final_observation)?;
        }

        let exit_status = loop {
            if let Some(status) = session.wait_for_exit(Duration::from_millis(0))? {
                // Exit codes are typically 0-255, safe to cast
                #[allow(clippy::cast_possible_wrap)]
                let code = status.exit_code() as i32;
                break ExitStatus {
                    success: status.success(),
                    exit_code: Some(code),
                    signal: None,
                    terminated_by_harness: false,
                };
            }

            if Instant::now() > deadline {
                let termination = session.terminate_process_group(Duration::from_millis(200));
                let context = match termination {
                    Ok(_) => serde_json::json!({"max_runtime_ms": policy.budgets.max_runtime_ms}),
                    Err(err) => serde_json::json!({
                        "max_runtime_ms": policy.budgets.max_runtime_ms,
                        "termination_error": err.to_string()
                    }),
                };
                return Err(RunnerError::timeout(
                    "E_TIMEOUT",
                    "run exceeded max runtime budget",
                    context,
                ));
            }

            let observation = session.observe(Duration::from_millis(50))?;
            enforce_exec_budgets(&mut session, &observation, &mut output_bytes, &policy)?;
            if let Some(writer) = artifacts.as_mut() {
                writer.write_observation(&observation)?;
            }
            final_observation = observation;
        };

        let ended_at = elapsed_ms(&run_started);
        let status = if exit_status.success {
            RunStatus::Passed
        } else {
            RunStatus::Failed
        };

        let error = if status == RunStatus::Failed {
            Some(crate::model::ErrorInfo {
                code: "E_PROCESS_EXITED".to_string(),
                message: "process exited unsuccessfully".to_string(),
                context: None,
            })
        } else {
            None
        };

        let run_result = RunResult {
            run_result_version: 1,
            protocol_version: PROTOCOL_VERSION,
            run_id,
            status,
            started_at_ms: 0,
            ended_at_ms: ended_at,
            command: command.clone(),
            args: args.clone(),
            cwd: cwd.unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_default()
                    .display()
                    .to_string()
            }),
            policy: policy.clone(),
            scenario: None,
            steps: None,
            final_observation: Some(final_observation),
            exit_status: Some(exit_status),
            error,
        };

        if let Some(writer) = artifacts.as_mut() {
            writer.write_run_result(&run_result)?;
            if let Some(observation) = &run_result.final_observation {
                writer.write_snapshot(&observation.screen)?;
                if let Some(delta) = &observation.transcript_delta {
                    writer.write_transcript(delta)?;
                }
            }
        }

        Ok(run_result)
    })();

    if let Err(err) = &result {
        if let Some(writer) = artifacts.as_mut() {
            let _ = writer.write_policy(&policy);
            let run_result = RunResult {
                run_result_version: 1,
                protocol_version: PROTOCOL_VERSION,
                run_id,
                status: RunStatus::Errored,
                started_at_ms: 0,
                ended_at_ms: elapsed_ms(&run_started),
                command: command.clone(),
                args: args.clone(),
                cwd: cwd.clone().unwrap_or_else(|| {
                    std::env::current_dir()
                        .unwrap_or_default()
                        .display()
                        .to_string()
                }),
                policy: policy.clone(),
                scenario: None,
                steps: None,
                final_observation: None,
                exit_status: None,
                error: Some(err.to_error_info()),
            };
            let _ = writer.write_run_result(&run_result);
        }
    }

    cleanup_sandbox(cleanup_path);

    result
}

fn validate_policy(policy: &Policy) -> RunnerResult<()> {
    validate_policy_version(policy)?;
    validate_sandbox_mode(&policy.sandbox, policy.sandbox_unsafe_ack)?;
    validate_network_policy(policy)?;
    validate_env_policy(&policy.env)?;
    validate_fs_policy(&policy.fs, policy.fs_write_unsafe_ack)?;
    validate_artifacts_policy(policy)?;
    validate_write_access(policy, None)?;
    Ok(())
}

/// Load a scenario from a file.
///
/// Supports both JSON and YAML formats (detected by extension).
///
/// # Arguments
/// * `path` - Path to scenario file
///
/// # Errors
/// Returns `RunnerError` if the file cannot be read or parsed.
pub fn load_scenario(path: &str) -> RunnerResult<Scenario> {
    crate::scenario::load_scenario_file(path)
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

fn wait_for_condition(
    session: &mut Session,
    action: &Action,
    timeout: Duration,
    policy: &Policy,
) -> RunnerResult<crate::model::Observation> {
    let wait_payload: WaitPayload = serde_json::from_value(action.payload.clone())
        .map_err(|e| {
            RunnerError::protocol_with_context(
                "E_PROTOCOL",
                "invalid wait action payload",
                serde_json::json!({
                    "parse_error": e.to_string(),
                    "received_payload": action.payload,
                    "expected": {
                        "condition": {
                            "type": "screen_contains | screen_matches | cursor_at | process_exited",
                            "payload": "object (varies by condition type)"
                        }
                    },
                    "examples": {
                        "screen_contains": {"condition": {"type": "screen_contains", "payload": {"text": "Ready"}}},
                        "screen_matches": {"condition": {"type": "screen_matches", "payload": {"pattern": "\\$\\s*$"}}},
                        "cursor_at": {"condition": {"type": "cursor_at", "payload": {"row": 0, "col": 0}}},
                        "process_exited": {"condition": {"type": "process_exited", "payload": {}}}
                    }
                }),
            )
        })?;
    let max_wait = Duration::from_millis(policy.budgets.max_wait_ms);
    let wait_timeout = if timeout > max_wait {
        max_wait
    } else {
        timeout
    };
    let deadline = Instant::now() + wait_timeout;

    // Pre-compile regex before the loop to avoid recompilation on each iteration
    let compiled_regex = if wait_payload.condition.condition_type == "screen_matches" {
        let pattern = wait_payload
            .condition
            .payload
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        Some(
            regex::Regex::new(pattern)
                .map_err(|_| RunnerError::protocol("E_PROTOCOL", "invalid regex"))?,
        )
    } else {
        None
    };

    loop {
        if Instant::now() > deadline {
            return Err(RunnerError::timeout(
                "E_TIMEOUT",
                "wait condition timed out",
                serde_json::json!({"condition": wait_payload.condition.condition_type}),
            ));
        }

        let observation = session.observe(Duration::from_millis(50))?;

        // Check for unexpected process exit for all condition types
        if session.wait_for_exit(Duration::from_millis(0))?.is_some() {
            if wait_payload.condition.condition_type == "process_exited" {
                return Ok(observation);
            }
            // Process exited unexpectedly while waiting for another condition
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

        std::thread::sleep(Duration::from_millis(10));
    }
}

fn action_type_label(action_type: &ActionType) -> &'static str {
    match action_type {
        ActionType::Key => "key",
        ActionType::Text => "text",
        ActionType::Resize => "resize",
        ActionType::Wait => "wait",
        ActionType::Terminate => "terminate",
    }
}

fn step_context(step: &crate::model::Step, details: Option<Value>) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("step_id".to_string(), Value::String(step.id.to_string()));
    map.insert("step_name".to_string(), Value::String(step.name.clone()));
    map.insert(
        "action_type".to_string(),
        Value::String(action_type_label(&step.action.action_type).to_string()),
    );
    map.insert(
        "timeout_ms".to_string(),
        Value::Number(step.timeout_ms.into()),
    );
    if let Some(details) = details {
        map.insert("details".to_string(), details);
    }
    Value::Object(map)
}

fn with_step_timeout_context(err: RunnerError, step: &crate::model::Step) -> RunnerError {
    let details = err.context.clone();
    RunnerError::timeout(err.code, err.message, Some(step_context(step, details)))
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
                .and_then(|v| v.as_str())
                .unwrap_or("");
            Ok(observation.screen.lines.join("\n").contains(text))
        }
        "screen_matches" => {
            // Use pre-compiled regex if available, otherwise compile (fallback)
            let screen_text = observation.screen.lines.join("\n");
            if let Some(re) = compiled_regex {
                Ok(re.is_match(&screen_text))
            } else {
                let pattern = condition
                    .payload
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let re = regex::Regex::new(pattern)
                    .map_err(|_| RunnerError::protocol("E_PROTOCOL", "invalid regex"))?;
                Ok(re.is_match(&screen_text))
            }
        }
        "cursor_at" => {
            // Terminal coordinates are always small, safe to truncate
            #[allow(clippy::cast_possible_truncation)]
            let row = condition
                .payload
                .get("row")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u16;
            #[allow(clippy::cast_possible_truncation)]
            let col = condition
                .payload
                .get("col")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u16;
            Ok(observation.screen.cursor.row == row && observation.screen.cursor.col == col)
        }
        "process_exited" => Ok(false),
        other => Err(RunnerError::protocol(
            "E_PROTOCOL",
            format!("unsupported wait condition '{other}'"),
        )),
    }
}

fn snapshot_bytes(snapshot: &crate::model::ScreenSnapshot) -> RunnerResult<u64> {
    let data = serde_json::to_vec(snapshot)
        .map_err(|err| RunnerError::io("E_PROTOCOL", "failed to encode snapshot", err))?;
    Ok(data.len() as u64)
}

fn enforce_exec_budgets(
    session: &mut Session,
    observation: &crate::model::Observation,
    output_bytes: &mut u64,
    policy: &Policy,
) -> RunnerResult<()> {
    *output_bytes += observation
        .transcript_delta
        .as_ref()
        .map(|s| s.len() as u64)
        .unwrap_or(0);
    if *output_bytes > policy.budgets.max_output_bytes {
        let termination = session.terminate_process_group(Duration::from_millis(200));
        let context = match termination {
            Ok(_) => serde_json::json!({"max_output_bytes": policy.budgets.max_output_bytes}),
            Err(err) => serde_json::json!({
                "max_output_bytes": policy.budgets.max_output_bytes,
                "termination_error": err.to_string()
            }),
        };
        return Err(RunnerError::timeout(
            "E_TIMEOUT",
            "output budget exceeded",
            context,
        ));
    }

    if snapshot_bytes(&observation.screen)? > policy.budgets.max_snapshot_bytes {
        let termination = session.terminate_process_group(Duration::from_millis(200));
        let context = match termination {
            Ok(_) => serde_json::json!({"max_snapshot_bytes": policy.budgets.max_snapshot_bytes}),
            Err(err) => serde_json::json!({
                "max_snapshot_bytes": policy.budgets.max_snapshot_bytes,
                "termination_error": err.to_string()
            }),
        };
        return Err(RunnerError::timeout(
            "E_TIMEOUT",
            "snapshot budget exceeded",
            context,
        ));
    }

    Ok(())
}

fn elapsed_ms(started_at: &Instant) -> u64 {
    // Elapsed time in practice is always well under u64::MAX milliseconds
    #[allow(clippy::cast_possible_truncation)]
    let ms = started_at.elapsed().as_millis() as u64;
    ms
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
        crate::model::policy::SandboxMode::Seatbelt => {
            let profile_path = if let Some(dir) = artifacts_dir {
                dir.join("sandbox.sb")
            } else {
                std::env::temp_dir().join(format!("tui-use-{run_id}.sb"))
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
        crate::model::policy::SandboxMode::None => Ok(SpawnCommand {
            command: command.to_string(),
            args: args.to_vec(),
            cleanup_path: None,
        }),
    }
}

fn cleanup_sandbox(path: Option<PathBuf>) {
    if let Some(path) = path {
        let _ = std::fs::remove_file(path);
    }
}
