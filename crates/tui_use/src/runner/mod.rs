use crate::artifacts::{ArtifactsWriter, ArtifactsWriterConfig};
use crate::model::policy::Policy;
use crate::model::{
    Action, ActionType, AssertionResult, ExitStatus, PROTOCOL_VERSION, RunConfig, RunId, RunResult,
    RunStatus, Scenario, StepResult, StepStatus, TerminalSize,
};
use crate::policy::{
    EffectivePolicy, sandbox, validate_env_policy, validate_fs_policy, validate_sandbox_mode,
};
use crate::scenario::load_policy_ref;
use crate::session::{Session, SessionConfig};
use miette::Diagnostic;
use serde::Deserialize;
use serde_json::Value;
use std::fmt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub type RunnerResult<T> = Result<T, RunnerError>;

#[derive(Debug)]
pub struct RunnerError {
    pub code: String,
    pub message: String,
    pub context: Option<Value>,
}

impl RunnerError {
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

    pub fn protocol(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            context: None,
        }
    }

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

    pub fn internal(code: impl Into<String>, message: impl Into<String>) -> Self {
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

#[derive(Clone, Debug, Default)]
pub struct RunnerOptions {
    pub artifacts: Option<ArtifactsWriterConfig>,
}

pub fn run_scenario(scenario: Scenario, options: RunnerOptions) -> RunnerResult<RunResult> {
    let run_id = RunId::new();
    let run_started = Instant::now();
    let scenario_clone = scenario.clone();

    let artifacts_dir = options.artifacts.as_ref().map(|config| config.dir.clone());
    let mut artifacts = options
        .artifacts
        .map(|config| ArtifactsWriter::new(run_id, config))
        .transpose()?;

    if let Some(writer) = artifacts.as_mut() {
        writer.write_scenario(&scenario_clone)?;
    }

    let mut policy_for_error: Option<Policy> = None;
    let mut cleanup_path: Option<PathBuf> = None;

    let result: RunnerResult<RunResult> = (|| {
        let policy = load_policy_ref(&scenario.run.policy)?;
        policy_for_error = Some(policy.clone());
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

        for step in &scenario.steps {
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
                        last_error = Some(err);
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
                        serde_json::json!({"max_output_bytes": policy.budgets.max_output_bytes}),
                    ));
                    status = StepStatus::Errored;
                    break;
                }

                if snapshot_bytes(&observation.screen)? > policy.budgets.max_snapshot_bytes {
                    last_error = Some(RunnerError::timeout(
                        "E_TIMEOUT",
                        "snapshot budget exceeded",
                        serde_json::json!({"max_snapshot_bytes": policy.budgets.max_snapshot_bytes}),
                    ));
                    status = StepStatus::Errored;
                    break;
                }

                if let Some(writer) = artifacts.as_mut() {
                    writer.write_snapshot(&observation.screen)?;
                    if let Some(delta) = &observation.transcript_delta {
                        writer.write_transcript(delta)?;
                    }
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
                status,
                attempts,
                started_at_ms: step_started_ms,
                ended_at_ms: step_ended_ms,
                action: step.action.clone(),
                assertions: assertion_results.clone(),
                error: error_info,
            });
        }

        let final_observation = session.observe(Duration::from_millis(10)).ok();
        let exit_status = session
            .wait_for_exit(Duration::from_millis(policy.budgets.max_runtime_ms))?
            .map(|status| ExitStatus {
                success: status.success(),
                exit_code: Some(status.exit_code() as i32),
                signal: None,
                terminated_by_harness: false,
            });

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

        Ok(run_result)
    })();

    if let Err(err) = &result {
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

pub fn run_exec(
    command: String,
    args: Vec<String>,
    cwd: Option<String>,
    policy: Policy,
) -> RunnerResult<RunResult> {
    run_exec_with_options(command, args, cwd, policy, RunnerOptions::default())
}

pub fn run_exec_with_options(
    command: String,
    args: Vec<String>,
    cwd: Option<String>,
    policy: Policy,
    options: RunnerOptions,
) -> RunnerResult<RunResult> {
    let run_id = RunId::new();
    let run_started = Instant::now();

    let artifacts_dir = options.artifacts.as_ref().map(|config| config.dir.clone());
    let mut artifacts = options
        .artifacts
        .map(|config| ArtifactsWriter::new(run_id, config))
        .transpose()?;

    let mut cleanup_path: Option<PathBuf> = None;

    let result: RunnerResult<RunResult> = (|| {
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

        let final_observation = session.observe(Duration::from_millis(50)).ok();
        let exit_status = session
            .wait_for_exit(Duration::from_millis(policy.budgets.max_runtime_ms))?
            .map(|status| ExitStatus {
                success: status.success(),
                exit_code: Some(status.exit_code() as i32),
                signal: None,
                terminated_by_harness: false,
            });

        let ended_at = elapsed_ms(&run_started);
        let status = if exit_status.as_ref().map(|s| s.success).unwrap_or(true) {
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
            final_observation,
            exit_status,
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
    validate_sandbox_mode(&policy.sandbox)?;
    validate_env_policy(&policy.env)?;
    validate_fs_policy(&policy.fs)?;
    Ok(())
}

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
        .map_err(|_| RunnerError::protocol("E_PROTOCOL", "invalid wait payload"))?;
    let max_wait = Duration::from_millis(policy.budgets.max_wait_ms);
    let wait_timeout = if timeout > max_wait {
        max_wait
    } else {
        timeout
    };
    let deadline = Instant::now() + wait_timeout;

    loop {
        if Instant::now() > deadline {
            return Err(RunnerError::timeout(
                "E_TIMEOUT",
                "wait condition timed out",
                serde_json::json!({"condition": wait_payload.condition.condition_type}),
            ));
        }

        let observation = session.observe(Duration::from_millis(50))?;
        if condition_satisfied(&observation, &wait_payload.condition)? {
            return Ok(observation);
        }

        if wait_payload.condition.condition_type == "process_exited"
            && session.wait_for_exit(Duration::from_millis(0))?.is_some()
        {
            return Ok(observation);
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

fn condition_satisfied(
    observation: &crate::model::Observation,
    condition: &Condition,
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
            let pattern = condition
                .payload
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let re = regex::Regex::new(pattern)
                .map_err(|_| RunnerError::protocol("E_PROTOCOL", "invalid regex"))?;
            Ok(re.is_match(&observation.screen.lines.join("\n")))
        }
        "cursor_at" => {
            let row = condition
                .payload
                .get("row")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u16;
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

fn elapsed_ms(started_at: &Instant) -> u64 {
    started_at.elapsed().as_millis() as u64
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
                std::env::temp_dir().join(format!("tui-use-{}.sb", run_id))
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
