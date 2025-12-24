//! TUI-Use CLI: Safe terminal automation harness.
//!
//! Command-line interface for running scenarios and commands under policy control.

// CLI-specific lint allowances
#![allow(missing_docs)] // TODO: Add docs
#![allow(clippy::print_stdout)] // CLI must print to stdout
#![allow(clippy::print_stderr)] // CLI must print to stderr
#![allow(clippy::exit)] // CLI uses exit codes
#![allow(clippy::unreachable)] // Used for exhaustive enum matching
#![allow(clippy::fn_params_excessive_bools)] // CLI flags are naturally bools

use clap::{Parser, Subcommand, ValueEnum};
use miette::{IntoDiagnostic, Result};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use tui_use::artifacts::ArtifactsWriterConfig;
use tui_use::model::policy::Policy;
use tui_use::policy::explain_policy_for_run_config;
use tui_use::runner::{
    load_scenario, run_exec_with_options, run_scenario, RunnerError, RunnerOptions,
};
use tui_use::scenario::load_policy_file;

#[derive(Debug, Parser)]
#[command(name = "tui-use", version, about = "Safe TUI automation harness")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Exec {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        policy: Option<PathBuf>,
        #[arg(long)]
        explain_policy: bool,
        #[arg(long, help = "Override the policy working directory (absolute path)")]
        cwd: Option<String>,
        #[arg(
            long,
            help = "Write artifacts to this directory (requires allowlisted write access)"
        )]
        artifacts: Option<PathBuf>,
        #[arg(long, help = "Overwrite existing artifacts directory")]
        overwrite: bool,
        #[arg(
            long,
            help = "Disable sandboxing (unsafe without --ack-unsafe-sandbox)"
        )]
        no_sandbox: bool,
        #[arg(long, help = "Acknowledge unsafe sandbox disablement")]
        ack_unsafe_sandbox: bool,
        #[arg(
            long,
            help = "Enable network access (unsafe without --ack-unsafe-network)"
        )]
        enable_network: bool,
        #[arg(long, help = "Acknowledge unsafe network access")]
        ack_unsafe_network: bool,
        #[arg(long, help = "Acknowledge unsafe write access")]
        ack_unsafe_write: bool,
        #[arg(
            long,
            help = "Require explicit write acknowledgement for any write access"
        )]
        strict_write: bool,
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },
    Run {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        scenario: PathBuf,
        #[arg(long)]
        explain_policy: bool,
        #[arg(
            long,
            help = "Write artifacts to this directory (requires allowlisted write access)"
        )]
        artifacts: Option<PathBuf>,
        #[arg(long, help = "Overwrite existing artifacts directory")]
        overwrite: bool,
        #[arg(
            long,
            help = "Disable sandboxing (unsafe without --ack-unsafe-sandbox)"
        )]
        no_sandbox: bool,
        #[arg(long, help = "Acknowledge unsafe sandbox disablement")]
        ack_unsafe_sandbox: bool,
        #[arg(
            long,
            help = "Enable network access (unsafe without --ack-unsafe-network)"
        )]
        enable_network: bool,
        #[arg(long, help = "Acknowledge unsafe network access")]
        ack_unsafe_network: bool,
        #[arg(long, help = "Acknowledge unsafe write access")]
        ack_unsafe_write: bool,
        #[arg(
            long,
            help = "Require explicit write acknowledgement for any write access"
        )]
        strict_write: bool,
    },
    Replay {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        artifacts: PathBuf,
        #[arg(long)]
        strict: bool,
        #[arg(long, value_enum)]
        normalize: Vec<NormalizeFilterArg>,
        #[arg(long)]
        explain: bool,
        #[arg(long)]
        require_events: bool,
        #[arg(long)]
        require_checksums: bool,
    },
    ReplayReport {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        artifacts: PathBuf,
    },
    Driver {
        #[arg(long)]
        stdio: bool,
        #[arg(long)]
        json: bool,
        #[arg(
            long,
            help = "Require explicit write acknowledgement for any write access"
        )]
        strict_write: bool,
        #[arg(long, help = "Acknowledge unsafe write access")]
        ack_unsafe_write: bool,
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Exec {
            json,
            policy,
            explain_policy,
            cwd,
            artifacts,
            overwrite,
            no_sandbox,
            ack_unsafe_sandbox,
            enable_network,
            ack_unsafe_network,
            ack_unsafe_write,
            strict_write,
            command,
        } => {
            let (cmd, args) = split_command(command)?;
            let mut policy = match policy {
                Some(path) => load_policy_file(&path)?,
                None => Policy::default(),
            };
            apply_cli_policy_overrides(
                &mut policy,
                no_sandbox,
                ack_unsafe_sandbox,
                enable_network,
                ack_unsafe_network,
                ack_unsafe_write,
                strict_write,
            );
            if let Some(dir) = cwd.as_ref() {
                if !std::path::Path::new(dir).is_absolute() {
                    return emit_cli_error(json, "--cwd must be an absolute path");
                }
            }
            if explain_policy {
                let cwd = cwd.clone().or_else(|| policy.fs.working_dir.clone());
                let run_config = tui_use::model::RunConfig {
                    command: cmd.clone(),
                    args: args.clone(),
                    cwd,
                    initial_size: tui_use::model::TerminalSize::default(),
                    policy: tui_use::model::scenario::PolicyRef::Inline(policy.clone()),
                };
                let explanation = explain_policy_for_run_config(&policy, &run_config);
                emit_explanation(json, &explanation)?;
                return Ok(());
            }
            let options = RunnerOptions {
                artifacts: artifacts.map(|dir| ArtifactsWriterConfig { dir, overwrite }),
            };
            let result = run_exec_with_options(cmd, args, cwd, policy, options);
            emit_result(json, result)
        }
        Commands::Run {
            json,
            scenario,
            explain_policy,
            artifacts,
            overwrite,
            no_sandbox,
            ack_unsafe_sandbox,
            enable_network,
            ack_unsafe_network,
            ack_unsafe_write,
            strict_write,
        } => {
            let mut scenario = load_scenario(scenario.to_str().unwrap_or(""))?;
            let mut policy = tui_use::scenario::load_policy_ref(&scenario.run.policy)?;
            apply_cli_policy_overrides(
                &mut policy,
                no_sandbox,
                ack_unsafe_sandbox,
                enable_network,
                ack_unsafe_network,
                ack_unsafe_write,
                strict_write,
            );
            if let Some(dir) = scenario.run.cwd.as_ref() {
                if !std::path::Path::new(dir).is_absolute() {
                    return emit_cli_error(json, "scenario cwd must be an absolute path");
                }
            }
            if explain_policy {
                let run_config = tui_use::model::RunConfig {
                    command: scenario.run.command.clone(),
                    args: scenario.run.args.clone(),
                    cwd: scenario.run.cwd.clone(),
                    initial_size: scenario.run.initial_size.clone(),
                    policy: tui_use::model::scenario::PolicyRef::Inline(policy.clone()),
                };
                let explanation = explain_policy_for_run_config(&policy, &run_config);
                emit_explanation(json, &explanation)?;
                return Ok(());
            }
            scenario.run.policy = tui_use::model::scenario::PolicyRef::Inline(policy);
            let options = RunnerOptions {
                artifacts: artifacts.map(|dir| ArtifactsWriterConfig { dir, overwrite }),
            };
            let result = run_scenario(scenario, options);
            emit_result(json, result)
        }
        Commands::Driver {
            stdio,
            json,
            strict_write,
            ack_unsafe_write,
            command,
        } => {
            if !stdio || !json {
                return emit_cli_error(json, "driver requires --stdio --json");
            }
            let (cmd, args) = split_command(command)?;
            let mut policy = Policy::default();
            apply_cli_policy_overrides(
                &mut policy,
                true,
                true,
                false,
                true,
                ack_unsafe_write,
                strict_write,
            );
            run_driver(cmd, args, policy)
        }
        Commands::Replay {
            json,
            artifacts,
            strict,
            normalize,
            explain,
            require_events,
            require_checksums,
        } => {
            let has_none = normalize
                .iter()
                .any(|filter| matches!(filter, NormalizeFilterArg::None));
            if has_none && normalize.len() > 1 {
                return emit_cli_error(
                    json,
                    "--normalize none cannot be combined with other filters",
                );
            }
            if strict && !normalize.is_empty() && !has_none {
                return emit_cli_error(json, "--strict cannot be combined with --normalize");
            }
            let has_all = normalize
                .iter()
                .any(|filter| matches!(filter, NormalizeFilterArg::All));
            if has_all && normalize.len() > 1 {
                return emit_cli_error(
                    json,
                    "--normalize all cannot be combined with other filters",
                );
            }
            let filters = if normalize.is_empty() || has_all {
                None
            } else if has_none {
                Some(Vec::new())
            } else {
                Some(normalize.into_iter().map(|f| f.into()).collect())
            };
            let options = tui_use::replay::ReplayOptions {
                strict,
                filters,
                require_events,
                require_checksums,
            };
            if explain {
                let explanation = tui_use::replay::explain_replay(&artifacts, options)?;
                if json {
                    let payload = serde_json::to_string(&explanation).into_diagnostic()?;
                    println!("{payload}");
                } else {
                    eprintln!("replay normalization: {explanation:?}");
                }
                return Ok(());
            }
            let result = tui_use::replay::replay_artifacts(&artifacts, options);
            emit_result(json, result)
        }
        Commands::ReplayReport { json, artifacts } => {
            let report = tui_use::replay::read_replay_report(&artifacts)?;
            if json {
                let payload = serde_json::to_string(&report).into_diagnostic()?;
                println!("{payload}");
            } else {
                eprintln!("replay report: {}", report.dir);
            }
            Ok(())
        }
    }
}

fn emit_result(json: bool, result: Result<tui_use::model::RunResult, RunnerError>) -> Result<()> {
    match result {
        Ok(run_result) => {
            if json {
                let payload = serde_json::to_string(&run_result).into_diagnostic()?;
                println!("{payload}");
            } else {
                eprintln!("run completed: {:?}", run_result.status);
            }
            if matches!(run_result.status, tui_use::model::RunStatus::Failed) {
                if let Some(err) = run_result.error.as_ref() {
                    std::process::exit(exit_code_for_error_code(&err.code));
                }
                std::process::exit(1);
            }
            Ok(())
        }
        Err(err) => {
            if json {
                let payload = serde_json::to_string(&err.to_error_info()).into_diagnostic()?;
                println!("{payload}");
            } else {
                eprintln!("error: {err}");
            }
            if err.message.contains("open pty") {
                eprintln!(
                    "warning: PTY support appears unavailable; this is common in minimal containers"
                );
            }
            std::process::exit(exit_code_for_error(&err));
        }
    }
}

fn split_command(mut command: Vec<String>) -> Result<(String, Vec<String>), RunnerError> {
    if command.is_empty() {
        return Err(RunnerError::protocol("E_PROTOCOL", "missing command"));
    }
    let cmd = command.remove(0);
    Ok((cmd, command))
}

fn run_driver(command: String, args: Vec<String>, policy: Policy) -> Result<()> {
    use tui_use::model::{Action, ActionType, RunId, PROTOCOL_VERSION};
    use tui_use::policy::validate_policy;
    use tui_use::session::{Session, SessionConfig};
    #[derive(serde::Deserialize)]
    struct DriverInput {
        protocol_version: u32,
        action: Action,
    }
    let run_id = RunId::new();
    validate_policy(&policy)?;
    let mut session = Session::spawn(SessionConfig {
        command,
        args,
        cwd: None,
        size: tui_use::model::TerminalSize::default(),
        run_id,
        env: policy.env,
    })?;

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line.into_diagnostic()?;
        if line.trim().is_empty() {
            continue;
        }
        let input: DriverInput = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => {
                emit_driver_error("E_PROTOCOL", "invalid json action", None)?;
                return Err(miette::miette!("invalid json action"));
            }
        };
        if input.protocol_version != PROTOCOL_VERSION {
            emit_driver_error(
                "E_PROTOCOL_VERSION_MISMATCH",
                "unsupported protocol version; update the client to the supported version",
                Some(serde_json::json!({
                    "provided_version": input.protocol_version,
                    "supported_version": PROTOCOL_VERSION
                })),
            )?;
            return Err(miette::miette!("protocol version mismatch"));
        }
        let action = input.action;
        session.send(&action)?;
        let observation = session.observe(std::time::Duration::from_millis(50))?;
        let payload = serde_json::to_string(&observation).into_diagnostic()?;
        writeln!(stdout, "{payload}").into_diagnostic()?;
        stdout.flush().into_diagnostic()?;

        if matches!(action.action_type, ActionType::Terminate) {
            break;
        }
    }

    Ok(())
}

#[derive(Copy, Clone, Debug, ValueEnum)]
#[value(rename_all = "snake_case")]
enum NormalizeFilterArg {
    All,
    None,
    SnapshotId,
    RunId,
    RunTimestamps,
    StepTimestamps,
    ObservationTimestamp,
    SessionId,
}

impl From<NormalizeFilterArg> for tui_use::model::NormalizationFilter {
    fn from(value: NormalizeFilterArg) -> Self {
        match value {
            NormalizeFilterArg::All => unreachable!("all handled before normalization mapping"),
            NormalizeFilterArg::None => {
                unreachable!("none handled before normalization mapping")
            }
            NormalizeFilterArg::SnapshotId => Self::SnapshotId,
            NormalizeFilterArg::RunId => Self::RunId,
            NormalizeFilterArg::RunTimestamps => Self::RunTimestamps,
            NormalizeFilterArg::StepTimestamps => Self::StepTimestamps,
            NormalizeFilterArg::ObservationTimestamp => Self::ObservationTimestamp,
            NormalizeFilterArg::SessionId => Self::SessionId,
        }
    }
}

fn apply_cli_policy_overrides(
    policy: &mut Policy,
    no_sandbox: bool,
    ack_unsafe_sandbox: bool,
    enable_network: bool,
    ack_unsafe_network: bool,
    ack_unsafe_write: bool,
    strict_write: bool,
) {
    if no_sandbox {
        policy.sandbox = tui_use::model::policy::SandboxMode::None;
    }
    if ack_unsafe_sandbox {
        policy.sandbox_unsafe_ack = true;
    }
    if enable_network {
        policy.network = tui_use::model::policy::NetworkPolicy::Enabled;
    }
    if ack_unsafe_network {
        policy.network_unsafe_ack = true;
    }
    if ack_unsafe_write {
        policy.fs_write_unsafe_ack = true;
    }
    if strict_write {
        policy.fs_strict_write = true;
    }
}

fn emit_explanation(json: bool, explanation: &tui_use::policy::PolicyExplanation) -> Result<()> {
    if json {
        let payload = serde_json::to_string(explanation).into_diagnostic()?;
        println!("{payload}");
    } else if explanation.allowed {
        println!("policy: allowed");
    } else {
        println!("policy: denied");
        for err in &explanation.errors {
            println!(" - {}: {}", err.code, err.message);
        }
    }
    Ok(())
}

fn emit_driver_error(code: &str, message: &str, context: Option<serde_json::Value>) -> Result<()> {
    let error = tui_use::model::ErrorInfo {
        code: code.to_string(),
        message: message.to_string(),
        context,
    };
    let payload = serde_json::to_string(&error).into_diagnostic()?;
    println!("{payload}");
    Ok(())
}

fn exit_code_for_error_code(code: &str) -> i32 {
    match code {
        "E_POLICY_DENIED" => 2,
        "E_SANDBOX_UNAVAILABLE" => 3,
        "E_TIMEOUT" => 4,
        "E_ASSERTION_FAILED" => 5,
        "E_PROCESS_EXITED" => 6,
        "E_TERMINAL_PARSE" => 7,
        "E_PROTOCOL_VERSION_MISMATCH" => 8,
        "E_PROTOCOL" => 9,
        "E_IO" => 10,
        "E_REPLAY_MISMATCH" => 11,
        "E_CLI_INVALID_ARG" => 12,
        _ => 1,
    }
}

fn emit_cli_error(json: bool, message: &str) -> Result<()> {
    emit_result(
        json,
        Err(RunnerError::protocol("E_CLI_INVALID_ARG", message)),
    )
}

fn exit_code_for_error(err: &RunnerError) -> i32 {
    exit_code_for_error_code(&err.code)
}

#[cfg(test)]
mod tests {
    use super::exit_code_for_error;
    use tui_use::runner::RunnerError;

    #[test]
    fn exit_code_maps_policy_denied() {
        let err = RunnerError::policy_denied("E_POLICY_DENIED", "denied", None);
        assert_eq!(exit_code_for_error(&err), 2);
    }

    #[test]
    fn exit_code_maps_timeout() {
        let err = RunnerError::timeout("E_TIMEOUT", "timeout", None);
        assert_eq!(exit_code_for_error(&err), 4);
    }
}
