pub mod progress;

use crate::artifacts::{ArtifactsWriter, ArtifactsWriterConfig};
use crate::model::policy::Policy;
use crate::model::{
    Action, ActionType, AssertionResult, ExitStatus, NormalizationRecord, RunConfig, RunId,
    RunResult, RunStatus, Scenario, StepResult, StepStatus, TerminalSize, MAX_REGEX_PATTERN_LEN,
    NORMALIZATION_VERSION, PROTOCOL_VERSION,
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

/// Compile a regex pattern with length validation to prevent `ReDoS` attacks.
///
/// # Errors
/// Returns `E_PROTOCOL` if pattern exceeds `MAX_REGEX_PATTERN_LEN` or is invalid.
pub fn compile_safe_regex(pattern: &str) -> Result<regex::Regex, RunnerError> {
    if pattern.len() > MAX_REGEX_PATTERN_LEN {
        return Err(RunnerError::protocol(
            "E_PROTOCOL",
            format!("regex pattern exceeds maximum length of {MAX_REGEX_PATTERN_LEN} characters"),
            Some(serde_json::json!({
                "pattern_length": pattern.len(),
                "max_length": MAX_REGEX_PATTERN_LEN
            })),
        ));
    }
    regex::Regex::new(pattern)
        .map_err(|err| RunnerError::protocol("E_PROTOCOL", format!("invalid regex: {err}"), None))
}

/// Result type alias for runner operations.
pub type RunnerResult<T> = Result<T, RunnerError>;

/// Stable error codes that map to specific exit codes for automation.
///
/// Each variant maps to a specific exit code, documented in `spec/data-model.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    /// Policy validation failed (exit 2).
    PolicyDenied,
    /// Sandbox not available on platform (exit 3).
    SandboxUnavailable,
    /// Budget or step timeout exceeded (exit 4).
    Timeout,
    /// Assertion did not pass (exit 5).
    AssertionFailed,
    /// Process exited with non-zero code (exit 6).
    ProcessExit,
    /// Terminal output parsing failed (exit 7).
    TerminalParse,
    /// Protocol version mismatch (exit 8).
    ProtocolVersionMismatch,
    /// Generic protocol error (exit 9).
    Protocol,
    /// I/O operation failed (exit 10).
    Io,
    /// Replay comparison failed (exit 11).
    ReplayMismatch,
    /// Invalid CLI argument (exit 12).
    CliInvalidArg,
    /// Internal error (exit 1).
    Internal,
}

impl ErrorCode {
    /// Get the string representation of the error code.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PolicyDenied => "E_POLICY_DENIED",
            Self::SandboxUnavailable => "E_SANDBOX_UNAVAILABLE",
            Self::Timeout => "E_TIMEOUT",
            Self::AssertionFailed => "E_ASSERTION_FAILED",
            Self::ProcessExit => "E_PROCESS_EXIT",
            Self::TerminalParse => "E_TERMINAL_PARSE",
            Self::ProtocolVersionMismatch => "E_PROTOCOL_VERSION_MISMATCH",
            Self::Protocol => "E_PROTOCOL",
            Self::Io => "E_IO",
            Self::ReplayMismatch => "E_REPLAY_MISMATCH",
            Self::CliInvalidArg => "E_CLI_INVALID_ARG",
            Self::Internal => "E_INTERNAL",
        }
    }

    /// Get the exit code for this error code.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::PolicyDenied => 2,
            Self::SandboxUnavailable => 3,
            Self::Timeout => 4,
            Self::AssertionFailed => 5,
            Self::ProcessExit => 6,
            Self::TerminalParse => 7,
            Self::ProtocolVersionMismatch => 8,
            Self::Protocol => 9,
            Self::Io => 10,
            Self::ReplayMismatch => 11,
            Self::CliInvalidArg => 12,
            Self::Internal => 1,
        }
    }

    /// Parse an error code from its string representation.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "E_POLICY_DENIED" => Some(Self::PolicyDenied),
            "E_SANDBOX_UNAVAILABLE" => Some(Self::SandboxUnavailable),
            "E_TIMEOUT" => Some(Self::Timeout),
            "E_ASSERTION_FAILED" => Some(Self::AssertionFailed),
            "E_PROCESS_EXIT" => Some(Self::ProcessExit),
            "E_TERMINAL_PARSE" => Some(Self::TerminalParse),
            "E_PROTOCOL_VERSION_MISMATCH" => Some(Self::ProtocolVersionMismatch),
            "E_PROTOCOL" => Some(Self::Protocol),
            "E_IO" => Some(Self::Io),
            "E_REPLAY_MISMATCH" => Some(Self::ReplayMismatch),
            "E_CLI_INVALID_ARG" => Some(Self::CliInvalidArg),
            "E_INTERNAL" => Some(Self::Internal),
            _ => None,
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

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
    /// Stable error code.
    pub code: ErrorCode,
    /// Human-readable error message.
    pub message: String,
    /// Structured context for debugging.
    pub context: Option<Value>,
    /// Source error for error chaining.
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl RunnerError {
    /// Create a new error with the given code and message.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            context: None,
            source: None,
        }
    }

    /// Create a new error with context.
    pub fn with_context(code: ErrorCode, message: impl Into<String>, context: Value) -> Self {
        Self {
            code,
            message: message.into(),
            context: Some(context),
            source: None,
        }
    }

    /// Create a new error with a source error.
    pub fn with_source(
        code: ErrorCode,
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            context: Some(serde_json::json!({ "source": source.to_string() })),
            source: Some(Box::new(source)),
        }
    }

    /// Create a policy denied error (legacy API, kept for compatibility).
    pub fn policy_denied(
        _code: impl Into<String>,
        message: impl Into<String>,
        context: impl Into<Option<Value>>,
    ) -> Self {
        Self {
            code: ErrorCode::PolicyDenied,
            message: message.into(),
            context: context.into(),
            source: None,
        }
    }

    /// Create a timeout error (budget exceeded, legacy API).
    pub fn timeout(
        _code: impl Into<String>,
        message: impl Into<String>,
        context: impl Into<Option<Value>>,
    ) -> Self {
        Self {
            code: ErrorCode::Timeout,
            message: message.into(),
            context: context.into(),
            source: None,
        }
    }

    /// Create a protocol error (invalid JSON or version mismatch, legacy API).
    pub fn protocol(
        _code: impl Into<String>,
        message: impl Into<String>,
        context: impl Into<Option<Value>>,
    ) -> Self {
        Self {
            code: ErrorCode::Protocol,
            message: message.into(),
            context: context.into(),
            source: None,
        }
    }

    /// Create an I/O error (legacy API).
    pub fn io(
        _code: impl Into<String>,
        message: impl Into<String>,
        err: impl std::fmt::Display,
    ) -> Self {
        Self {
            code: ErrorCode::Io,
            message: message.into(),
            context: Some(serde_json::json!({ "source": err.to_string() })),
            source: None,
        }
    }

    /// Create an I/O error with proper error chaining.
    ///
    /// This preserves the source error for use with `std::error::Error::source()`.
    pub fn io_err<E>(message: impl Into<String>, err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self {
            code: ErrorCode::Io,
            message: message.into(),
            context: Some(serde_json::json!({ "source": err.to_string() })),
            source: Some(Box::new(err)),
        }
    }

    /// Create a terminal parse error (legacy API).
    pub fn terminal_parse(
        _code: impl Into<String>,
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
            code: ErrorCode::TerminalParse,
            message: message.into(),
            context: Some(Value::Object(context)),
            source: None,
        }
    }

    /// Create an internal error (legacy API).
    pub fn internal(_code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Internal,
            message: message.into(),
            context: None,
            source: None,
        }
    }

    /// Create a process exit error (legacy API).
    pub fn process_exit(_code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::ProcessExit,
            message: message.into(),
            context: None,
            source: None,
        }
    }

    /// Create a sandbox unavailable error.
    pub fn sandbox_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::SandboxUnavailable,
            message: message.into(),
            context: None,
            source: None,
        }
    }

    /// Create an assertion failed error.
    pub fn assertion_failed(message: impl Into<String>, context: impl Into<Option<Value>>) -> Self {
        Self {
            code: ErrorCode::AssertionFailed,
            message: message.into(),
            context: context.into(),
            source: None,
        }
    }

    /// Create a replay mismatch error.
    pub fn replay_mismatch(message: impl Into<String>, context: impl Into<Option<Value>>) -> Self {
        Self {
            code: ErrorCode::ReplayMismatch,
            message: message.into(),
            context: context.into(),
            source: None,
        }
    }

    /// Create a CLI invalid argument error.
    pub fn cli_invalid_arg(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::CliInvalidArg,
            message: message.into(),
            context: None,
            source: None,
        }
    }

    /// Create a protocol version mismatch error.
    pub fn protocol_version_mismatch(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::ProtocolVersionMismatch,
            message: message.into(),
            context: None,
            source: None,
        }
    }

    /// Get the exit code for this error.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        self.code.exit_code()
    }

    /// Convert to the stable `ErrorInfo` type for serialization.
    pub fn to_error_info(&self) -> crate::model::ErrorInfo {
        crate::model::ErrorInfo {
            code: self.code.as_str().to_string(),
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
        self.source
            .as_ref()
            .map(|e| e.as_ref() as &(dyn std::error::Error + 'static))
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

/// Get the current working directory as a string, with fallback.
fn get_cwd_string(cwd: Option<String>, policy_cwd: Option<&String>) -> String {
    cwd.or_else(|| policy_cwd.cloned()).unwrap_or_else(|| {
        std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "<unknown>".to_string())
    })
}

/// Create a skipped step result.
fn create_skipped_step(
    step: &crate::model::Step,
    time_ms: u64,
    error: Option<&RunnerError>,
) -> StepResult {
    StepResult {
        step_id: step.id,
        name: step.name.clone(),
        status: StepStatus::Skipped,
        attempts: 0,
        started_at_ms: time_ms,
        ended_at_ms: time_ms,
        action: step.action.clone(),
        assertions: Vec::new(),
        error: error.map(|e| e.to_error_info()),
    }
}

/// Result of executing a single step.
struct StepExecutionResult {
    step_result: StepResult,
    run_error: Option<RunnerError>,
}

/// Execute a single step with retry logic.
#[allow(clippy::too_many_arguments)]
fn execute_step(
    session: &mut Session,
    step: &crate::model::Step,
    policy: &Policy,
    effective_policy: &EffectivePolicy,
    artifacts: &mut Option<ArtifactsWriter>,
    output_bytes: &mut u64,
    step_started_ms: u64,
    run_started: &Instant,
) -> RunnerResult<StepExecutionResult> {
    let mut attempts = 0;
    let mut last_error: Option<RunnerError> = None;
    let mut status = StepStatus::Failed;
    let mut assertion_results = Vec::new();

    for _ in 0..=step.retries {
        attempts += 1;
        effective_policy.validate_action(&step.action)?;

        let observation = match perform_action(
            session,
            &step.action,
            Duration::from_millis(step.timeout_ms),
            policy,
        ) {
            Ok(obs) => obs,
            Err(err) => {
                last_error = Some(if err.code == ErrorCode::Timeout {
                    with_step_timeout_context(err, step)
                } else {
                    err
                });
                status = StepStatus::Errored;
                continue;
            }
        };

        // Check budgets
        *output_bytes += observation
            .transcript_delta
            .as_ref()
            .map(|s| s.len() as u64)
            .unwrap_or(0);

        if let Some(budget_error) = check_step_budgets(&observation, *output_bytes, policy, step)? {
            last_error = Some(budget_error);
            status = StepStatus::Errored;
            break;
        }

        // Write artifacts
        if let Some(writer) = artifacts.as_mut() {
            writer.write_snapshot(&observation.screen)?;
            if let Some(delta) = &observation.transcript_delta {
                writer.write_transcript(delta)?;
            }
            writer.write_observation(&observation)?;
        }

        // Evaluate assertions
        assertion_results.clear();
        let mut assertions_passed = true;
        for assertion in &step.assert {
            let (passed, message, details) = crate::assertions::evaluate(&observation, assertion);
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
        }
        last_error = Some(RunnerError::assertion_failed(
            "one or more assertions failed",
            None,
        ));
    }

    let step_ended_ms = elapsed_ms(run_started);
    let error_info = last_error.as_ref().map(|e| e.to_error_info());
    let run_error = if status == StepStatus::Passed {
        None
    } else {
        last_error
    };

    Ok(StepExecutionResult {
        step_result: StepResult {
            step_id: step.id,
            name: step.name.clone(),
            status,
            attempts,
            started_at_ms: step_started_ms,
            ended_at_ms: step_ended_ms,
            action: step.action.clone(),
            assertions: assertion_results,
            error: error_info,
        },
        run_error,
    })
}

/// Check step budgets and return an error if exceeded.
fn check_step_budgets(
    observation: &crate::model::Observation,
    output_bytes: u64,
    policy: &Policy,
    step: &crate::model::Step,
) -> RunnerResult<Option<RunnerError>> {
    if output_bytes > policy.budgets.max_output_bytes {
        return Ok(Some(RunnerError::timeout(
            "E_TIMEOUT",
            "output budget exceeded",
            Some(step_context(
                step,
                Some(serde_json::json!({
                    "max_output_bytes": policy.budgets.max_output_bytes
                })),
            )),
        )));
    }

    if snapshot_bytes(&observation.screen)? > policy.budgets.max_snapshot_bytes {
        return Ok(Some(RunnerError::timeout(
            "E_TIMEOUT",
            "snapshot budget exceeded",
            Some(step_context(
                step,
                Some(serde_json::json!({
                    "max_snapshot_bytes": policy.budgets.max_snapshot_bytes
                })),
            )),
        )));
    }

    Ok(None)
}

/// Wait for process exit with budget enforcement, returning exit status.
fn await_scenario_exit(
    session: &mut Session,
    policy: &Policy,
    run_started: &Instant,
    has_error: bool,
) -> RunnerResult<Option<ExitStatus>> {
    if has_error {
        return Ok(session
            .terminate_process_group(Duration::from_millis(200))
            .ok()
            .flatten()
            .map(|status| convert_exit_status(status, true)));
    }

    let max_runtime = Duration::from_millis(policy.budgets.max_runtime_ms);
    let elapsed = run_started.elapsed();

    if elapsed >= max_runtime {
        return Err(create_timeout_error(session, policy));
    }

    let remaining = max_runtime - elapsed;
    match session.wait_for_exit(remaining)? {
        Some(status) => Ok(Some(convert_exit_status(status, false))),
        None => Err(create_timeout_error(session, policy)),
    }
}

/// Convert a `portable_pty` exit status to our `ExitStatus` type.
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

/// Create a timeout error, terminating the process if needed.
fn create_timeout_error(session: &mut Session, policy: &Policy) -> RunnerError {
    let termination = session.terminate_process_group(Duration::from_millis(200));
    let context = match termination {
        Ok(_) => serde_json::json!({"max_runtime_ms": policy.budgets.max_runtime_ms}),
        Err(err) => serde_json::json!({
            "max_runtime_ms": policy.budgets.max_runtime_ms,
            "termination_error": err.to_string()
        }),
    };
    RunnerError::timeout("E_TIMEOUT", "run exceeded max runtime budget", context)
}

/// Poll for process exit in exec mode, returning final observation and exit status.
fn poll_exec_until_exit(
    session: &mut Session,
    policy: &Policy,
    artifacts: &mut Option<ArtifactsWriter>,
    deadline: Instant,
) -> RunnerResult<(crate::model::Observation, ExitStatus)> {
    let mut output_bytes: u64 = 0;
    let mut final_observation = session.observe(Duration::from_millis(50))?;
    enforce_exec_budgets(session, &final_observation, &mut output_bytes, policy)?;

    if let Some(writer) = artifacts.as_mut() {
        writer.write_observation(&final_observation)?;
    }

    loop {
        if let Some(status) = session.wait_for_exit(Duration::from_millis(0))? {
            // Capture final observation after exit
            let observation = session.observe(Duration::from_millis(10))?;
            if let Some(writer) = artifacts.as_mut() {
                writer.write_observation(&observation)?;
            }
            return Ok((observation, convert_exit_status(status, false)));
        }

        if Instant::now() > deadline {
            return Err(create_timeout_error(session, policy));
        }

        let observation = session.observe(Duration::from_millis(50))?;
        enforce_exec_budgets(session, &observation, &mut output_bytes, policy)?;
        if let Some(writer) = artifacts.as_mut() {
            writer.write_observation(&observation)?;
        }
        #[allow(unused_assignments)]
        {
            final_observation = observation;
        }
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

    emit_progress(
        progress.as_ref(),
        ProgressEvent::RunStarted {
            run_id,
            total_steps: scenario.steps.len(),
        },
    );

    let mut artifacts: Option<ArtifactsWriter> = None;
    let mut policy_for_error: Option<Policy> = None;
    let mut cleanup_guard = SandboxCleanupGuard::new(None);

    let result = run_scenario_inner(
        &scenario,
        &options,
        run_id,
        &run_started,
        &progress,
        &mut artifacts,
        &mut policy_for_error,
        &mut cleanup_guard,
    );

    handle_scenario_result(
        &result,
        &scenario_clone,
        run_id,
        &run_started,
        &progress,
        &mut artifacts,
        &policy_for_error,
    );

    drop(cleanup_guard);
    result
}

/// Inner implementation of `run_scenario` to reduce main function complexity.
#[allow(clippy::too_many_arguments, clippy::ref_option)]
fn run_scenario_inner(
    scenario: &Scenario,
    options: &RunnerOptions,
    run_id: RunId,
    run_started: &Instant,
    progress: &Option<Arc<dyn ProgressCallback>>,
    artifacts: &mut Option<ArtifactsWriter>,
    policy_for_error: &mut Option<Policy>,
    cleanup_guard: &mut SandboxCleanupGuard,
) -> RunnerResult<RunResult> {
    let policy = load_policy_ref(&scenario.run.policy)?;
    *policy_for_error = Some(policy.clone());

    let artifacts_dir = setup_scenario_artifacts(scenario, &policy, options, run_id, artifacts)?;
    validate_policy(&policy)?;
    validate_scenario_steps(scenario, &policy)?;

    let effective_policy = EffectivePolicy::new(policy.clone());
    effective_policy.validate_run_config(&scenario.run)?;

    if let Some(writer) = artifacts.as_mut() {
        writer.write_policy(&policy)?;
    }

    let mut session =
        spawn_scenario_session(scenario, &policy, &artifacts_dir, run_id, cleanup_guard)?;
    let (step_results, run_error) = execute_scenario_steps(
        &mut session,
        scenario,
        &policy,
        &effective_policy,
        artifacts,
        run_started,
        progress,
    )?;

    let final_observation = session.observe(Duration::from_millis(10)).ok();
    if let (Some(writer), Some(obs)) = (artifacts.as_mut(), final_observation.as_ref()) {
        writer.write_observation(obs)?;
    }

    let exit_status = await_scenario_exit(&mut session, &policy, run_started, run_error.is_some())?;
    let run_result = build_scenario_result(
        scenario,
        &policy,
        run_id,
        run_started,
        step_results,
        final_observation,
        exit_status,
        run_error,
    );

    if let Some(writer) = artifacts.as_mut() {
        writer.write_run_result(&run_result)?;
        writer.flush_checksums()?;
    }

    emit_progress(
        progress.as_ref(),
        ProgressEvent::RunCompleted {
            run_id,
            success: run_result.status == RunStatus::Passed,
            duration_ms: run_result.ended_at_ms,
        },
    );

    Ok(run_result)
}

/// Setup artifacts for scenario execution.
fn setup_scenario_artifacts(
    scenario: &Scenario,
    policy: &Policy,
    options: &RunnerOptions,
    run_id: RunId,
    artifacts: &mut Option<ArtifactsWriter>,
) -> RunnerResult<Option<PathBuf>> {
    validate_fs_policy(&policy.fs)?;
    validate_artifacts_policy(policy)?;

    let artifacts_config = resolve_artifacts_config(policy, options.artifacts.clone());
    let artifacts_dir = artifacts_config.as_ref().map(|config| config.dir.clone());
    validate_write_access(policy, artifacts_dir.as_deref())?;

    if let Some(config) = artifacts_config {
        validate_artifacts_dir(&config.dir, &policy.fs)?;
        let mut writer = ArtifactsWriter::new(run_id, config)?;
        writer.write_normalization(&NormalizationRecord {
            normalization_version: NORMALIZATION_VERSION,
            filters: Vec::new(),
            strict: false,
            source: crate::model::NormalizationSource::None,
            rules: Vec::new(),
        })?;
        writer.write_scenario(scenario)?;
        *artifacts = Some(writer);
    }

    Ok(artifacts_dir)
}

/// Validate scenario steps against policy budgets.
fn validate_scenario_steps(scenario: &Scenario, policy: &Policy) -> RunnerResult<()> {
    if scenario.steps.len() as u64 > policy.budgets.max_steps {
        return Err(RunnerError::timeout(
            "E_TIMEOUT",
            "scenario exceeds max_steps budget",
            serde_json::json!({"max_steps": policy.budgets.max_steps}),
        ));
    }
    Ok(())
}

/// Spawn a session for scenario execution.
#[allow(clippy::ref_option)]
fn spawn_scenario_session(
    scenario: &Scenario,
    policy: &Policy,
    artifacts_dir: &Option<PathBuf>,
    run_id: RunId,
    cleanup_guard: &mut SandboxCleanupGuard,
) -> RunnerResult<Session> {
    let cwd = scenario
        .run
        .cwd
        .clone()
        .or_else(|| policy.fs.working_dir.clone());
    let spawn = build_spawn_command(
        policy,
        &scenario.run.command,
        &scenario.run.args,
        artifacts_dir.as_ref(),
        run_id,
    )?;
    cleanup_guard.path = spawn.cleanup_path.clone();

    Session::spawn(SessionConfig {
        command: spawn.command,
        args: spawn.args,
        cwd,
        size: scenario.run.initial_size.clone(),
        run_id,
        env: policy.env.clone(),
    })
}

/// Execute all steps in a scenario.
#[allow(clippy::ref_option)]
fn execute_scenario_steps(
    session: &mut Session,
    scenario: &Scenario,
    policy: &Policy,
    effective_policy: &EffectivePolicy,
    artifacts: &mut Option<ArtifactsWriter>,
    run_started: &Instant,
    progress: &Option<Arc<dyn ProgressCallback>>,
) -> RunnerResult<(Vec<StepResult>, Option<RunnerError>)> {
    let mut step_results = Vec::new();
    let mut run_error: Option<RunnerError> = None;
    let mut output_bytes: u64 = 0;

    for (step_index, step) in scenario.steps.iter().enumerate() {
        if run_error.is_some() {
            step_results.push(create_skipped_step(step, elapsed_ms(run_started), None));
            continue;
        }

        if elapsed_ms(run_started) > policy.budgets.max_runtime_ms {
            run_error = Some(RunnerError::timeout(
                "E_TIMEOUT",
                "run exceeded max runtime budget",
                serde_json::json!({"max_runtime_ms": policy.budgets.max_runtime_ms}),
            ));
            step_results.push(create_skipped_step(
                step,
                elapsed_ms(run_started),
                run_error.as_ref(),
            ));
            continue;
        }

        emit_progress(
            progress.as_ref(),
            ProgressEvent::StepStarted {
                step_id: step.id,
                step_index: step_index + 1,
                name: step.name.clone(),
            },
        );

        let step_started_ms = elapsed_ms(run_started);
        let exec_result = execute_step(
            session,
            step,
            policy,
            effective_policy,
            artifacts,
            &mut output_bytes,
            step_started_ms,
            run_started,
        )?;

        let step_ended_ms = elapsed_ms(run_started);
        emit_progress(
            progress.as_ref(),
            ProgressEvent::StepCompleted {
                step_id: step.id,
                name: step.name.clone(),
                status: exec_result.step_result.status.clone(),
                duration_ms: step_ended_ms - step_started_ms,
                assertions: exec_result.step_result.assertions.clone(),
            },
        );

        if exec_result.run_error.is_some() {
            run_error = exec_result.run_error;
        }
        step_results.push(exec_result.step_result);
    }

    Ok((step_results, run_error))
}

/// Build the final run result for a scenario.
#[allow(clippy::too_many_arguments)]
fn build_scenario_result(
    scenario: &Scenario,
    policy: &Policy,
    run_id: RunId,
    run_started: &Instant,
    step_results: Vec<StepResult>,
    final_observation: Option<crate::model::Observation>,
    exit_status: Option<ExitStatus>,
    run_error: Option<RunnerError>,
) -> RunResult {
    let status = if step_results
        .iter()
        .all(|s| matches!(s.status, StepStatus::Passed))
    {
        RunStatus::Passed
    } else {
        RunStatus::Failed
    };

    RunResult {
        run_result_version: 1,
        protocol_version: PROTOCOL_VERSION,
        run_id,
        status,
        started_at_ms: 0,
        ended_at_ms: elapsed_ms(run_started),
        command: scenario.run.command.clone(),
        args: scenario.run.args.clone(),
        cwd: get_cwd_string(scenario.run.cwd.clone(), policy.fs.working_dir.as_ref()),
        policy: policy.clone(),
        scenario: Some(scenario.clone()),
        steps: Some(step_results),
        final_observation,
        exit_status,
        error: run_error.map(|err| err.to_error_info()),
    }
}

/// Handle scenario result (emit events and write error artifacts if needed).
#[allow(clippy::ref_option)]
fn handle_scenario_result(
    result: &RunnerResult<RunResult>,
    scenario: &Scenario,
    run_id: RunId,
    run_started: &Instant,
    progress: &Option<Arc<dyn ProgressCallback>>,
    artifacts: &mut Option<ArtifactsWriter>,
    policy_for_error: &Option<Policy>,
) {
    if let Err(err) = result {
        emit_progress(
            progress.as_ref(),
            ProgressEvent::RunCompleted {
                run_id,
                success: false,
                duration_ms: elapsed_ms(run_started),
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
                ended_at_ms: elapsed_ms(run_started),
                command: scenario.run.command.clone(),
                args: scenario.run.args.clone(),
                cwd: get_cwd_string(scenario.run.cwd.clone(), policy.fs.working_dir.as_ref()),
                policy,
                scenario: Some(scenario.clone()),
                steps: None,
                final_observation: None,
                exit_status: None,
                error: Some(err.to_error_info()),
            };
            let _ = writer.write_run_result(&run_result);
        }
    }
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
    let mut cleanup_guard = SandboxCleanupGuard::new(None);

    let result = run_exec_inner(
        &command,
        &args,
        &cwd,
        &policy,
        &options,
        run_id,
        &run_started,
        &mut artifacts,
        &mut cleanup_guard,
    );

    handle_exec_error(
        &result,
        &command,
        &args,
        &cwd,
        &policy,
        run_id,
        &run_started,
        &mut artifacts,
    );
    drop(cleanup_guard);
    result
}

/// Inner implementation of `run_exec_with_options`.
#[allow(clippy::too_many_arguments, clippy::ref_option)]
fn run_exec_inner(
    command: &str,
    args: &[String],
    cwd: &Option<String>,
    policy: &Policy,
    options: &RunnerOptions,
    run_id: RunId,
    run_started: &Instant,
    artifacts: &mut Option<ArtifactsWriter>,
    cleanup_guard: &mut SandboxCleanupGuard,
) -> RunnerResult<RunResult> {
    let artifacts_dir = setup_exec_artifacts(policy, options, run_id, artifacts)?;
    validate_policy(policy)?;

    let effective_cwd = cwd.clone().or_else(|| policy.fs.working_dir.clone());
    validate_exec_config(command, args, &effective_cwd, policy)?;

    if let Some(writer) = artifacts.as_mut() {
        writer.write_policy(policy)?;
    }

    let mut session = spawn_exec_session(
        command,
        args,
        &effective_cwd,
        policy,
        &artifacts_dir,
        run_id,
        cleanup_guard,
    )?;
    let deadline = Instant::now() + Duration::from_millis(policy.budgets.max_runtime_ms);
    let (final_observation, exit_status) =
        poll_exec_until_exit(&mut session, policy, artifacts, deadline)?;

    let run_result = build_exec_result(
        command,
        args,
        &effective_cwd,
        policy,
        run_id,
        run_started,
        final_observation,
        exit_status,
    );

    if let Some(writer) = artifacts.as_mut() {
        if let Some(obs) = &run_result.final_observation {
            writer.write_snapshot(&obs.screen)?;
            if let Some(delta) = &obs.transcript_delta {
                writer.write_transcript(delta)?;
            }
        }
        writer.write_run_result(&run_result)?;
        writer.flush_checksums()?;
    }

    Ok(run_result)
}

/// Setup artifacts for exec command.
fn setup_exec_artifacts(
    policy: &Policy,
    options: &RunnerOptions,
    run_id: RunId,
    artifacts: &mut Option<ArtifactsWriter>,
) -> RunnerResult<Option<PathBuf>> {
    validate_fs_policy(&policy.fs)?;
    validate_artifacts_policy(policy)?;

    let artifacts_config = resolve_artifacts_config(policy, options.artifacts.clone());
    let artifacts_dir = artifacts_config.as_ref().map(|config| config.dir.clone());
    validate_write_access(policy, artifacts_dir.as_deref())?;

    if let Some(config) = artifacts_config {
        validate_artifacts_dir(&config.dir, &policy.fs)?;
        let mut writer = ArtifactsWriter::new(run_id, config)?;
        writer.write_normalization(&NormalizationRecord {
            normalization_version: NORMALIZATION_VERSION,
            filters: Vec::new(),
            strict: false,
            source: crate::model::NormalizationSource::None,
            rules: Vec::new(),
        })?;
        *artifacts = Some(writer);
    }

    Ok(artifacts_dir)
}

/// Validate exec run configuration.
#[allow(clippy::ref_option)]
fn validate_exec_config(
    command: &str,
    args: &[String],
    cwd: &Option<String>,
    policy: &Policy,
) -> RunnerResult<()> {
    let effective_policy = EffectivePolicy::new(policy.clone());
    let run_config = RunConfig {
        command: command.to_string(),
        args: args.to_vec(),
        cwd: cwd.clone(),
        initial_size: TerminalSize::default(),
        policy: crate::model::scenario::PolicyRef::Inline(policy.clone()),
    };
    effective_policy.validate_run_config(&run_config)
}

/// Spawn a session for exec command.
#[allow(clippy::ref_option)]
fn spawn_exec_session(
    command: &str,
    args: &[String],
    cwd: &Option<String>,
    policy: &Policy,
    artifacts_dir: &Option<PathBuf>,
    run_id: RunId,
    cleanup_guard: &mut SandboxCleanupGuard,
) -> RunnerResult<Session> {
    let spawn = build_spawn_command(policy, command, args, artifacts_dir.as_ref(), run_id)?;
    cleanup_guard.path = spawn.cleanup_path.clone();

    Session::spawn(SessionConfig {
        command: spawn.command,
        args: spawn.args,
        cwd: cwd.clone(),
        size: TerminalSize::default(),
        run_id,
        env: policy.env.clone(),
    })
}

/// Build the run result for exec command.
#[allow(clippy::too_many_arguments, clippy::ref_option)]
fn build_exec_result(
    command: &str,
    args: &[String],
    cwd: &Option<String>,
    policy: &Policy,
    run_id: RunId,
    run_started: &Instant,
    final_observation: crate::model::Observation,
    exit_status: ExitStatus,
) -> RunResult {
    let status = if exit_status.success {
        RunStatus::Passed
    } else {
        RunStatus::Failed
    };
    let error = if status == RunStatus::Failed {
        Some(crate::model::ErrorInfo {
            code: "E_PROCESS_EXIT".to_string(),
            message: "process exited unsuccessfully".to_string(),
            context: None,
        })
    } else {
        None
    };

    RunResult {
        run_result_version: 1,
        protocol_version: PROTOCOL_VERSION,
        run_id,
        status,
        started_at_ms: 0,
        ended_at_ms: elapsed_ms(run_started),
        command: command.to_string(),
        args: args.to_vec(),
        cwd: get_cwd_string(cwd.clone(), policy.fs.working_dir.as_ref()),
        policy: policy.clone(),
        scenario: None,
        steps: None,
        final_observation: Some(final_observation),
        exit_status: Some(exit_status),
        error,
    }
}

/// Handle exec error by writing error artifacts.
#[allow(clippy::too_many_arguments, clippy::ref_option)]
fn handle_exec_error(
    result: &RunnerResult<RunResult>,
    command: &str,
    args: &[String],
    cwd: &Option<String>,
    policy: &Policy,
    run_id: RunId,
    run_started: &Instant,
    artifacts: &mut Option<ArtifactsWriter>,
) {
    if let Err(err) = result {
        if let Some(writer) = artifacts.as_mut() {
            let _ = writer.write_policy(policy);
            let run_result = RunResult {
                run_result_version: 1,
                protocol_version: PROTOCOL_VERSION,
                run_id,
                status: RunStatus::Errored,
                started_at_ms: 0,
                ended_at_ms: elapsed_ms(run_started),
                command: command.to_string(),
                args: args.to_vec(),
                cwd: get_cwd_string(cwd.clone(), policy.fs.working_dir.as_ref()),
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
}

fn validate_policy(policy: &Policy) -> RunnerResult<()> {
    validate_policy_version(policy)?;
    validate_sandbox_mode(&policy.sandbox)?;
    validate_network_policy(policy)?;
    validate_env_policy(&policy.env)?;
    validate_fs_policy(&policy.fs)?;
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
            RunnerError::protocol(
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
        Some(compile_safe_regex(pattern)?)
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
    RunnerError::with_context(err.code, err.message, step_context(step, details))
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
                let re = compile_safe_regex(pattern)?;
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
            None,
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
        crate::model::policy::SandboxMode::Disabled { .. } => Ok(SpawnCommand {
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
struct SandboxCleanupGuard {
    path: Option<PathBuf>,
}

impl SandboxCleanupGuard {
    /// Create a new cleanup guard for the given path.
    fn new(path: Option<PathBuf>) -> Self {
        Self { path }
    }
}

impl Drop for SandboxCleanupGuard {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(&path);
        }
    }
}
