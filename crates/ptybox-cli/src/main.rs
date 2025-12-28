//! TUI-Use CLI: Safe terminal automation harness.
//!
//! Command-line interface for running scenarios and commands under policy control.

// CLI-specific lint allowances (CLI binary, not library)
#![allow(missing_docs)]
#![allow(clippy::print_stdout)] // CLI must print to stdout
#![allow(clippy::print_stderr)] // CLI must print to stderr
#![allow(clippy::exit)] // CLI uses exit codes
#![allow(clippy::unreachable)] // Used for exhaustive enum matching
#![allow(clippy::fn_params_excessive_bools)] // CLI flags are naturally bools

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use miette::{IntoDiagnostic, Result};
use ptybox::artifacts::ArtifactsWriterConfig;
use ptybox::model::policy::Policy;
use ptybox::policy::explain_policy_for_run_config;
use ptybox::runner::{
    load_scenario, run_exec_with_options, run_scenario, RunnerError, RunnerOptions,
};
use ptybox::scenario::load_policy_file;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;

/// Color output mode
#[derive(Copy, Clone, Debug, Default, ValueEnum)]
enum ColorMode {
    /// Auto-detect based on terminal and `NO_COLOR` env
    #[default]
    Auto,
    /// Always use colors
    Always,
    /// Never use colors
    Never,
}

#[derive(Debug, Parser)]
#[command(name = "ptybox", version, about = "Safe TUI automation harness")]
struct Cli {
    /// Control color output
    #[arg(long, value_enum, default_value = "auto", global = true)]
    color: ColorMode,

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
        #[arg(long, short = 'v', help = "Show step-by-step progress to stderr")]
        verbose: bool,
        #[arg(long, help = "Run with interactive TUI showing live terminal output")]
        tui: bool,
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
    /// Output protocol documentation for LLM consumption
    ProtocolHelp {
        #[arg(long, help = "Output as JSON (default: human-readable)")]
        json: bool,
    },
    /// Generate shell completions for bash, zsh, or fish
    Completions {
        #[arg(value_enum, help = "Shell to generate completions for")]
        shell: Shell,
    },
    /// Generate an interactive HTML trace viewer from run artifacts
    Trace {
        #[arg(long, help = "Path to artifacts directory")]
        artifacts: PathBuf,
        #[arg(
            long,
            short = 'o',
            help = "Output HTML file path (default: trace.html)"
        )]
        output: Option<PathBuf>,
    },
}

mod progress;
mod protocol_help;
mod trace;
mod tui_mode;

/// Configure color output based on CLI flag and environment
fn configure_colors(mode: ColorMode) {
    let use_color = match mode {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => {
            // Respect NO_COLOR environment variable
            if std::env::var("NO_COLOR").is_ok() {
                false
            } else {
                // Check if stderr supports color (where we output diagnostics)
                supports_color::on(supports_color::Stream::Stderr).is_some()
            }
        }
    };

    // Configure miette's graphical reporting based on color mode
    if use_color {
        miette::set_hook(Box::new(|_| {
            Box::new(
                miette::MietteHandlerOpts::new()
                    .color(true)
                    .unicode(true)
                    .build(),
            )
        }))
        .ok(); // Ignore error if hook already set
    } else {
        miette::set_hook(Box::new(|_| {
            Box::new(
                miette::MietteHandlerOpts::new()
                    .color(false)
                    .unicode(false)
                    .build(),
            )
        }))
        .ok(); // Ignore error if hook already set
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    configure_colors(cli.color);
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
        } => cmd_exec(
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
        ),
        Commands::Run {
            json,
            scenario,
            explain_policy,
            verbose,
            tui,
            artifacts,
            overwrite,
            no_sandbox,
            ack_unsafe_sandbox,
            enable_network,
            ack_unsafe_network,
            ack_unsafe_write,
            strict_write,
        } => cmd_run(
            json,
            scenario,
            explain_policy,
            verbose,
            tui,
            artifacts,
            overwrite,
            no_sandbox,
            ack_unsafe_sandbox,
            enable_network,
            ack_unsafe_network,
            ack_unsafe_write,
            strict_write,
        ),
        Commands::Driver {
            stdio,
            json,
            strict_write,
            ack_unsafe_write,
            command,
        } => cmd_driver(stdio, json, strict_write, ack_unsafe_write, command),
        Commands::ProtocolHelp { json } => cmd_protocol_help(json),
        Commands::Replay {
            json,
            artifacts,
            strict,
            normalize,
            explain,
            require_events,
            require_checksums,
        } => cmd_replay(
            json,
            artifacts,
            strict,
            normalize,
            explain,
            require_events,
            require_checksums,
        ),
        Commands::ReplayReport { json, artifacts } => cmd_replay_report(json, artifacts),
        Commands::Completions { shell } => cmd_completions(shell),
        Commands::Trace { artifacts, output } => cmd_trace(artifacts, output),
    }
}

// =============================================================================
// Command Handlers
// =============================================================================

/// Handle the exec command.
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn cmd_exec(
    json: bool,
    policy: Option<PathBuf>,
    explain_policy: bool,
    cwd: Option<String>,
    artifacts: Option<PathBuf>,
    overwrite: bool,
    no_sandbox: bool,
    ack_unsafe_sandbox: bool,
    enable_network: bool,
    ack_unsafe_network: bool,
    ack_unsafe_write: bool,
    strict_write: bool,
    command: Vec<String>,
) -> Result<()> {
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
        let run_config = ptybox::model::RunConfig {
            command: cmd.clone(),
            args: args.clone(),
            cwd,
            initial_size: ptybox::model::TerminalSize::default(),
            policy: ptybox::model::scenario::PolicyRef::Inline(policy.clone()),
        };
        let explanation = explain_policy_for_run_config(&policy, &run_config);
        emit_explanation(json, &explanation)?;
        return Ok(());
    }
    let options = RunnerOptions {
        artifacts: artifacts.map(|dir| ArtifactsWriterConfig { dir, overwrite }),
        progress: None,
    };
    let result = run_exec_with_options(cmd, args, cwd, policy, options);
    emit_result(json, result)
}

/// Handle the run command.
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn cmd_run(
    json: bool,
    scenario_path: PathBuf,
    explain_policy: bool,
    verbose: bool,
    tui: bool,
    artifacts: Option<PathBuf>,
    overwrite: bool,
    no_sandbox: bool,
    ack_unsafe_sandbox: bool,
    enable_network: bool,
    ack_unsafe_network: bool,
    ack_unsafe_write: bool,
    strict_write: bool,
) -> Result<()> {
    let mut scenario = load_scenario(scenario_path.to_str().unwrap_or(""))?;
    let mut policy = ptybox::scenario::load_policy_ref(&scenario.run.policy)?;
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
        let run_config = ptybox::model::RunConfig {
            command: scenario.run.command.clone(),
            args: scenario.run.args.clone(),
            cwd: scenario.run.cwd.clone(),
            initial_size: scenario.run.initial_size.clone(),
            policy: ptybox::model::scenario::PolicyRef::Inline(policy.clone()),
        };
        let explanation = explain_policy_for_run_config(&policy, &run_config);
        emit_explanation(json, &explanation)?;
        return Ok(());
    }
    scenario.run.policy = ptybox::model::scenario::PolicyRef::Inline(policy);

    // TUI mode runs the scenario in an interactive terminal UI
    if tui {
        if verbose || json {
            return emit_cli_error(json, "--tui cannot be combined with --verbose or --json");
        }
        let artifacts_config = artifacts.map(|dir| ArtifactsWriterConfig { dir, overwrite });
        return tui_mode::run_tui(scenario, artifacts_config);
    }

    let progress_callback = if verbose {
        Some(Arc::new(progress::VerboseProgress::new()) as Arc<dyn ptybox::runner::ProgressCallback>)
    } else {
        None
    };
    let options = RunnerOptions {
        artifacts: artifacts.map(|dir| ArtifactsWriterConfig { dir, overwrite }),
        progress: progress_callback,
    };
    let result = run_scenario(scenario, options);
    emit_result(json, result)
}

/// Handle the driver command.
fn cmd_driver(
    stdio: bool,
    json: bool,
    strict_write: bool,
    ack_unsafe_write: bool,
    command: Vec<String>,
) -> Result<()> {
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

/// Handle the protocol-help command.
fn cmd_protocol_help(json: bool) -> Result<()> {
    let help = protocol_help::generate_protocol_help();
    if json {
        let output = serde_json::to_string_pretty(&help).into_diagnostic()?;
        println!("{output}");
    } else {
        print_protocol_help_text(&help);
    }
    Ok(())
}

/// Handle the replay command.
#[allow(clippy::fn_params_excessive_bools)]
fn cmd_replay(
    json: bool,
    artifacts: PathBuf,
    strict: bool,
    normalize: Vec<NormalizeFilterArg>,
    explain: bool,
    require_events: bool,
    require_checksums: bool,
) -> Result<()> {
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
    let options = ptybox::replay::ReplayOptions {
        strict,
        filters,
        require_events,
        require_checksums,
    };
    if explain {
        let explanation = ptybox::replay::explain_replay(&artifacts, options)?;
        if json {
            let payload = serde_json::to_string(&explanation).into_diagnostic()?;
            println!("{payload}");
        } else {
            eprintln!("replay normalization: {explanation:?}");
        }
        return Ok(());
    }
    let result = ptybox::replay::replay_artifacts(&artifacts, options);
    emit_result(json, result)
}

/// Handle the replay-report command.
fn cmd_replay_report(json: bool, artifacts: PathBuf) -> Result<()> {
    let report = ptybox::replay::read_replay_report(&artifacts)?;
    if json {
        let payload = serde_json::to_string(&report).into_diagnostic()?;
        println!("{payload}");
    } else {
        eprintln!("replay report: {}", report.dir);
    }
    Ok(())
}

/// Handle the completions command.
#[allow(clippy::unnecessary_wraps)] // Consistent with other command handlers
fn cmd_completions(shell: Shell) -> Result<()> {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    generate(shell, &mut cmd, name, &mut io::stdout());
    Ok(())
}

/// Handle the trace command.
fn cmd_trace(artifacts: PathBuf, output: Option<PathBuf>) -> Result<()> {
    let output_path = output.unwrap_or_else(|| PathBuf::from("trace.html"));
    trace::generate_trace(&artifacts, &output_path)?;
    eprintln!("trace written to: {}", output_path.display());
    Ok(())
}

fn emit_result(json: bool, result: Result<ptybox::model::RunResult, RunnerError>) -> Result<()> {
    match result {
        Ok(run_result) => {
            if json {
                let payload = serde_json::to_string(&run_result).into_diagnostic()?;
                println!("{payload}");
            } else {
                eprintln!("run completed: {:?}", run_result.status);
            }
            match run_result.status {
                ptybox::model::RunStatus::Passed => Ok(()),
                ptybox::model::RunStatus::Failed
                | ptybox::model::RunStatus::Errored
                | ptybox::model::RunStatus::Canceled => {
                    if let Some(err) = run_result.error.as_ref() {
                        std::process::exit(exit_code_for_error_code(&err.code));
                    }
                    std::process::exit(1);
                }
            }
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
        return Err(RunnerError::protocol("E_PROTOCOL", "missing command", None));
    }
    let cmd = command.remove(0);
    Ok((cmd, command))
}

fn run_driver(command: String, args: Vec<String>, policy: Policy) -> Result<()> {
    use ptybox::model::{Action, ActionType, RunId, PROTOCOL_VERSION};
    use ptybox::policy::validate_policy;
    use ptybox::session::{Session, SessionConfig};
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
        size: ptybox::model::TerminalSize::default(),
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
            Err(e) => {
                let context = serde_json::json!({
                    "parse_error": e.to_string(),
                    "received": line.chars().take(200).collect::<String>(),
                    "expected_schema": {
                        "protocol_version": "number (must be 1)",
                        "action": {
                            "type": "key | text | resize | wait | terminate",
                            "payload": "object (varies by action type)"
                        }
                    },
                    "example": {
                        "protocol_version": 1,
                        "action": {
                            "type": "text",
                            "payload": {"text": "hello"}
                        }
                    },
                    "hint": "Run 'ptybox protocol-help --json' for full schema documentation"
                });
                emit_driver_error("E_PROTOCOL", "invalid json action", Some(context))?;
                std::process::exit(exit_code_for_error_code("E_PROTOCOL"));
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
            std::process::exit(exit_code_for_error_code("E_PROTOCOL_VERSION_MISMATCH"));
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

impl From<NormalizeFilterArg> for ptybox::model::NormalizationFilter {
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
    use ptybox::model::policy::{NetworkPolicy, SandboxMode};

    // Handle sandbox mode
    if no_sandbox {
        // Set disabled with ack if ack_unsafe_sandbox is also set
        policy.sandbox = SandboxMode::Disabled {
            ack: ack_unsafe_sandbox,
        };
    } else if ack_unsafe_sandbox {
        // If already disabled, update the ack
        if let SandboxMode::Disabled { ref mut ack } = policy.sandbox {
            *ack = true;
        }
    }

    // Handle network policy
    if enable_network {
        // Set enabled with ack if ack_unsafe_network is also set
        policy.network = NetworkPolicy::Enabled {
            ack: ack_unsafe_network,
        };
    } else if ack_unsafe_network {
        // If already enabled, update the ack
        if let NetworkPolicy::Enabled { ref mut ack } = policy.network {
            *ack = true;
        }
    }

    // Handle network enforcement ack (for unenforced network when sandbox disabled)
    if ack_unsafe_network {
        policy.network_enforcement.unenforced_ack = true;
    }

    // Handle filesystem write acknowledgement
    if ack_unsafe_write {
        policy.fs.write_ack = true;
    }
    if strict_write {
        policy.fs.strict_write = true;
    }
}

fn emit_explanation(json: bool, explanation: &ptybox::policy::PolicyExplanation) -> Result<()> {
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
    use ptybox::model::PROTOCOL_VERSION;
    let error_response = serde_json::json!({
        "protocol_version": PROTOCOL_VERSION,
        "code": code,
        "message": message,
        "context": context,
    });
    let payload = serde_json::to_string(&error_response).into_diagnostic()?;
    println!("{payload}");
    Ok(())
}

fn exit_code_for_error_code(code: &str) -> i32 {
    ptybox::runner::ErrorCode::parse(code).map_or(1, |c| c.exit_code())
}

fn emit_cli_error(json: bool, message: &str) -> Result<()> {
    emit_result(json, Err(RunnerError::cli_invalid_arg(message)))
}

fn exit_code_for_error(err: &RunnerError) -> i32 {
    err.exit_code()
}

fn print_protocol_help_text(help: &protocol_help::ProtocolHelp) {
    println!("ptybox Protocol Help");
    println!("=====================");
    println!();
    println!("Protocol version: {}", help.protocol_version);
    println!();
    println!("COMMANDS");
    println!("--------");
    for (name, cmd) in &help.commands {
        println!("  {name}");
        println!("    {}", cmd.description);
        println!("    Usage: {}", cmd.usage);
        if let Some(flags) = &cmd.required_flags {
            println!("    Required: {}", flags.join(", "));
        }
        println!();
    }
    println!("ACTION TYPES");
    println!("------------");
    if let Some(action_schema) = help.schemas.get("Action") {
        if let Some(types) = &action_schema.types {
            for (name, variant) in types {
                let payload_desc: Vec<String> = variant
                    .payload
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}"))
                    .collect();
                if payload_desc.is_empty() {
                    println!("  {name}: {{}}");
                } else {
                    println!("  {name}: {{{}}}", payload_desc.join(", "));
                }
            }
        }
    }
    println!();
    println!("WAIT CONDITIONS");
    println!("---------------");
    if let Some(cond_schema) = help.schemas.get("Condition") {
        if let Some(types) = &cond_schema.types {
            for (name, variant) in types {
                let payload_desc: Vec<String> = variant
                    .payload
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}"))
                    .collect();
                if payload_desc.is_empty() {
                    println!("  {name}: {{}}");
                } else {
                    println!("  {name}: {{{}}}", payload_desc.join(", "));
                }
            }
        }
    }
    println!();
    println!("ERROR CODES");
    println!("-----------");
    for (code, info) in &help.error_codes {
        println!("  {code} (exit {}): {}", info.exit_code, info.description);
    }
    println!();
    println!("QUICKSTART");
    println!("----------");
    for step in &help.quickstart.steps {
        println!("  {step}");
    }
    println!();
    println!("For full JSON documentation: ptybox protocol-help --json");
}

#[cfg(test)]
mod tests {
    use super::exit_code_for_error;
    use ptybox::runner::RunnerError;

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
