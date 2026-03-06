//! TUI-Use CLI: Safe terminal automation harness.
//!
//! Command-line interface for running scenarios and commands under policy control.

// CLI-specific lint allowances (CLI binary, not library)
#![allow(clippy::print_stdout, clippy::print_stderr)] // CLI output
#![allow(clippy::exit, clippy::unreachable)] // CLI control flow
#![allow(clippy::fn_params_excessive_bools)] // Clap flags
#![allow(missing_docs, deprecated)]

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use miette::{IntoDiagnostic, Result};
use serde::Serialize;

use ptybox::artifacts::ArtifactsWriterConfig;
use ptybox::model::policy::Policy;
use ptybox::policy::explain_policy_for_run_config;
use ptybox::runner::{
    load_scenario, run_exec_with_options, run_scenario, RunnerError, RunnerOptions,
};
use ptybox::scenario::load_policy_file;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
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
        #[arg(long)]
        policy: Option<PathBuf>,
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

    // =========================================================================
    // Stateless session commands (agent-friendly)
    // =========================================================================
    /// Open a new session: spawn command, print session ID + screen
    Open {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        policy: Option<PathBuf>,
        #[arg(long, help = "Override the policy working directory (absolute path)")]
        cwd: Option<String>,
        #[arg(long, help = "Idle timeout in seconds (default: 1800)")]
        idle_timeout: Option<u64>,
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
    /// Send keys to a session and print the screen
    Keys {
        /// Session ID
        session_id: String,
        /// Keys to send (e.g. "dd", "Enter", "iHello")
        keys: String,
        #[arg(long)]
        json: bool,
    },
    /// Type text into a session and print the screen
    Type {
        /// Session ID
        session_id: String,
        /// Text to type
        text: String,
        #[arg(long)]
        json: bool,
    },
    /// Wait for a condition and print the screen
    Wait {
        /// Session ID
        session_id: String,
        #[arg(long, help = "Wait until screen contains this text")]
        contains: Option<String>,
        #[arg(long, help = "Wait until screen matches this regex")]
        matches: Option<String>,
        #[arg(long, help = "Wait timeout in milliseconds (default: 5000)")]
        timeout: Option<u64>,
        #[arg(long)]
        json: bool,
    },
    /// Print the current screen of a session
    Screen {
        /// Session ID
        session_id: String,
        #[arg(long)]
        json: bool,
    },
    /// Close a session and terminate its process
    Close {
        /// Session ID
        session_id: String,
        #[arg(long)]
        json: bool,
    },
    /// List active sessions
    Sessions {
        #[arg(long)]
        json: bool,
    },
    /// Internal: run the serve daemon (not user-facing)
    #[command(hide = true)]
    Serve {
        #[arg(long)]
        session_id: String,
        #[arg(long)]
        policy: Option<PathBuf>,
        #[arg(long)]
        cwd: Option<String>,
        #[arg(long)]
        idle_timeout: Option<u64>,
        #[arg(long)]
        no_sandbox: bool,
        #[arg(long)]
        ack_unsafe_sandbox: bool,
        #[arg(long)]
        enable_network: bool,
        #[arg(long)]
        ack_unsafe_network: bool,
        #[arg(long)]
        ack_unsafe_write: bool,
        #[arg(long)]
        strict_write: bool,
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },
}

mod progress;
mod protocol_help;
mod session_client;
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

/// Global flag set by the signal handler.
static INTERRUPTED: AtomicBool = AtomicBool::new(false);

/// Install a signal handler for SIGINT/SIGTERM that sets a flag
/// instead of immediately terminating the process.
fn install_signal_handler() {
    ctrlc::set_handler(move || {
        if INTERRUPTED.swap(true, Ordering::SeqCst) {
            // Second signal: force exit
            std::process::exit(130);
        }
        // First signal: set flag for graceful shutdown
        // The driver/runner loops check INTERRUPTED and emit JSON error
    })
    .ok(); // Ignore error if handler already set (e.g., in tests)
}

#[allow(clippy::too_many_lines)]
fn main() -> Result<()> {
    install_signal_handler();
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
            policy,
            cwd,
            artifacts,
            overwrite,
            no_sandbox,
            ack_unsafe_sandbox,
            enable_network,
            ack_unsafe_network,
            strict_write,
            ack_unsafe_write,
            command,
        } => cmd_driver(
            stdio,
            json,
            policy,
            cwd,
            artifacts,
            overwrite,
            no_sandbox,
            ack_unsafe_sandbox,
            enable_network,
            ack_unsafe_network,
            strict_write,
            ack_unsafe_write,
            command,
        ),
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
        Commands::Open {
            json,
            policy,
            cwd,
            idle_timeout,
            no_sandbox,
            ack_unsafe_sandbox,
            enable_network,
            ack_unsafe_network,
            ack_unsafe_write,
            strict_write,
            command,
        } => cmd_open(
            json,
            policy,
            cwd,
            idle_timeout,
            no_sandbox,
            ack_unsafe_sandbox,
            enable_network,
            ack_unsafe_network,
            ack_unsafe_write,
            strict_write,
            command,
        ),
        Commands::Keys {
            session_id,
            keys,
            json,
        } => cmd_keys(session_id, keys, json),
        Commands::Type {
            session_id,
            text,
            json,
        } => cmd_type(session_id, text, json),
        Commands::Wait {
            session_id,
            contains,
            matches,
            timeout,
            json,
        } => cmd_wait(session_id, contains, matches, timeout, json),
        Commands::Screen { session_id, json } => cmd_screen(session_id, json),
        Commands::Close { session_id, json } => cmd_close(session_id, json),
        Commands::Sessions { json } => cmd_sessions(json),
        Commands::Serve {
            session_id,
            policy,
            cwd,
            idle_timeout,
            no_sandbox,
            ack_unsafe_sandbox,
            enable_network,
            ack_unsafe_network,
            ack_unsafe_write,
            strict_write,
            command,
        } => cmd_serve(
            session_id,
            policy,
            cwd,
            idle_timeout,
            no_sandbox,
            ack_unsafe_sandbox,
            enable_network,
            ack_unsafe_network,
            ack_unsafe_write,
            strict_write,
            command,
        ),
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
            policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy.clone())),
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
            policy: ptybox::model::scenario::PolicyRef::Inline(Box::new(policy.clone())),
        };
        let explanation = explain_policy_for_run_config(&policy, &run_config);
        emit_explanation(json, &explanation)?;
        return Ok(());
    }
    scenario.run.policy = ptybox::model::scenario::PolicyRef::Inline(Box::new(policy));

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
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn cmd_driver(
    stdio: bool,
    json: bool,
    policy_path: Option<PathBuf>,
    cwd: Option<String>,
    artifacts: Option<PathBuf>,
    overwrite: bool,
    no_sandbox: bool,
    ack_unsafe_sandbox: bool,
    enable_network: bool,
    ack_unsafe_network: bool,
    strict_write: bool,
    ack_unsafe_write: bool,
    command: Vec<String>,
) -> Result<()> {
    if !stdio || !json {
        return emit_cli_error(json, "driver requires --stdio --json");
    }
    let (cmd, args) = split_command(command)?;
    let mut policy = match policy_path {
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

    let config = ptybox::driver::DriverConfig {
        command: cmd,
        args,
        cwd,
        policy,
        artifacts: artifacts.map(|dir| ArtifactsWriterConfig { dir, overwrite }),
    };

    match ptybox::driver::run_driver(config) {
        Ok(()) => Ok(()),
        Err(err) => std::process::exit(exit_code_for_error(&err)),
    }
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
            emit_json(&explanation)?;
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
        emit_json(&report)?;
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
                emit_json(&run_result)?;
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
                emit_json(&err.to_error_info())?;
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
    Events,
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
            NormalizeFilterArg::Events => Self::Events,
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
        emit_json(explanation)?;
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

fn exit_code_for_error_code(code: &str) -> i32 {
    ptybox::runner::ErrorCode::parse(code).map_or(1, |c| c.exit_code())
}

fn emit_json<T: Serialize>(value: &T) -> Result<()> {
    let payload = serde_json::to_string(value).into_diagnostic()?;
    println!("{payload}");
    Ok(())
}

fn emit_cli_error(json: bool, message: &str) -> Result<()> {
    emit_result(json, Err(RunnerError::cli_invalid_arg(message)))
}

fn exit_code_for_error(err: &RunnerError) -> i32 {
    err.exit_code()
}

// =============================================================================
// Session Command Handlers
// =============================================================================

/// Generate an 8-char hex session ID from /dev/urandom.
fn generate_session_id() -> String {
    use std::io::Read;
    let mut buf = [0u8; 4];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut buf);
    } else {
        // Fallback: use process ID + time as entropy
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        buf = [
            (t & 0xFF) as u8,
            ((t >> 8) & 0xFF) as u8,
            ((t >> 16) & 0xFF) as u8,
            ((t >> 24) & 0xFF) as u8,
        ];
    }
    format!("{:02x}{:02x}{:02x}{:02x}", buf[0], buf[1], buf[2], buf[3])
}

/// Build CLI arguments for the _serve subprocess.
struct ServeArgsBuilder {
    args: Vec<String>,
}

impl ServeArgsBuilder {
    fn new(session_id: &str) -> Self {
        Self {
            args: vec![
                "serve".to_string(),
                "--session-id".to_string(),
                session_id.to_string(),
            ],
        }
    }

    fn flag_if(&mut self, flag: &str, enabled: bool) {
        if enabled {
            self.args.push(flag.to_string());
        }
    }

    fn opt(&mut self, name: &str, value: &str) {
        self.args.push(name.to_string());
        self.args.push(value.to_string());
    }

    fn finish(mut self, command: Vec<String>) -> Vec<String> {
        self.args.push("--".to_string());
        self.args.extend(command);
        self.args
    }
}

/// Handle the open command: spawn a _serve daemon and print the initial screen.
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn cmd_open(
    json: bool,
    policy: Option<PathBuf>,
    cwd: Option<String>,
    idle_timeout: Option<u64>,
    no_sandbox: bool,
    ack_unsafe_sandbox: bool,
    enable_network: bool,
    ack_unsafe_network: bool,
    ack_unsafe_write: bool,
    strict_write: bool,
    command: Vec<String>,
) -> Result<()> {
    use std::io::BufRead;

    let session_id = generate_session_id();

    let mut builder = ServeArgsBuilder::new(&session_id);
    if let Some(ref p) = policy {
        builder.opt("--policy", &p.display().to_string());
    }
    if let Some(ref c) = cwd {
        builder.opt("--cwd", c);
    }
    if let Some(t) = idle_timeout {
        builder.opt("--idle-timeout", &t.to_string());
    }
    builder.flag_if("--no-sandbox", no_sandbox);
    builder.flag_if("--ack-unsafe-sandbox", ack_unsafe_sandbox);
    builder.flag_if("--enable-network", enable_network);
    builder.flag_if("--ack-unsafe-network", ack_unsafe_network);
    builder.flag_if("--ack-unsafe-write", ack_unsafe_write);
    builder.flag_if("--strict-write", strict_write);
    let serve_args = builder.finish(command);

    // Spawn the daemon with stdout piped so we can read the ready message
    let current_exe = std::env::current_exe().into_diagnostic()?;
    let mut child = std::process::Command::new(current_exe)
        .args(&serve_args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .into_diagnostic()?;

    // Read the ready message from the child's stdout
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| miette::miette!("failed to capture daemon stdout"))?;
    let mut reader = std::io::BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).into_diagnostic()?;

    if line.trim().is_empty() {
        let status = child.wait().into_diagnostic()?;
        eprintln!("daemon exited immediately with status: {status}");
        std::process::exit(1);
    }

    // Parse and validate the ready message
    let ready: serde_json::Value = serde_json::from_str(line.trim()).into_diagnostic()?;
    if !ready.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        let err_msg = ready
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        eprintln!("error: {err_msg}");
        std::process::exit(1);
    }

    print_open_output(json, &session_id, &ready, line.trim());
    Ok(())
}

/// Print the output of the `open` command.
fn print_open_output(json: bool, session_id: &str, ready: &serde_json::Value, raw_line: &str) {
    if json {
        println!("{raw_line}");
        return;
    }
    println!("{session_id}");
    if let Some(screen) = ready.get("screen") {
        if let Some(lines) = screen.get("lines").and_then(|v| v.as_array()) {
            let text_lines: Vec<&str> = lines.iter().filter_map(|l| l.as_str()).collect();
            let mut trimmed = text_lines;
            while trimmed.last().is_some_and(|l| l.trim().is_empty()) {
                trimmed.pop();
            }
            for l in trimmed {
                println!("{l}");
            }
        }
    }
}

/// Handle the keys command.
fn cmd_keys(session_id: String, keys: String, json: bool) -> Result<()> {
    use ptybox::serve::protocol::{ServeCommand, ServeRequest};
    let request = ServeRequest {
        command: ServeCommand::Keys { keys },
    };
    handle_session_response(&session_id, &request, json)
}

/// Handle the type command.
fn cmd_type(session_id: String, text: String, json: bool) -> Result<()> {
    use ptybox::serve::protocol::{ServeCommand, ServeRequest};
    let request = ServeRequest {
        command: ServeCommand::Text { text },
    };
    handle_session_response(&session_id, &request, json)
}

/// Handle the wait command.
fn cmd_wait(
    session_id: String,
    contains: Option<String>,
    matches: Option<String>,
    timeout: Option<u64>,
    json: bool,
) -> Result<()> {
    use ptybox::serve::protocol::{ServeCommand, ServeRequest};
    if contains.is_none() && matches.is_none() {
        return emit_cli_error(json, "wait requires --contains or --matches");
    }
    let request = ServeRequest {
        command: ServeCommand::Wait {
            contains,
            matches,
            timeout_ms: timeout,
        },
    };
    handle_session_response(&session_id, &request, json)
}

/// Handle the screen command.
fn cmd_screen(session_id: String, json: bool) -> Result<()> {
    use ptybox::serve::protocol::{ServeCommand, ServeRequest};
    let request = ServeRequest {
        command: ServeCommand::Screen,
    };
    handle_session_response(&session_id, &request, json)
}

/// Handle the close command.
fn cmd_close(session_id: String, json: bool) -> Result<()> {
    use ptybox::serve::protocol::{ServeCommand, ServeRequest};
    let request = ServeRequest {
        command: ServeCommand::Close,
    };
    let resp = session_client::send_request(&session_id, &request);
    match resp {
        Ok(r) if r.ok => {
            if json {
                emit_json(&r)?;
            } else {
                eprintln!("session {session_id} closed");
            }
            Ok(())
        }
        Ok(r) => {
            let msg = r.error.as_deref().unwrap_or("unknown error");
            if json {
                emit_json(&r)?;
            } else {
                eprintln!("error: {msg}");
            }
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

/// Handle the sessions command.
fn cmd_sessions(json: bool) -> Result<()> {
    let dir = session_client::socket_dir();
    if !dir.exists() {
        if json {
            println!("[]");
        }
        return Ok(());
    }

    let mut sessions = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(id) = session_client::session_id_from_path(&path) {
                let alive = session_client::is_session_alive(&id);
                sessions.push((id, alive));
            }
        }
    }

    if json {
        let items: Vec<serde_json::Value> = sessions
            .iter()
            .map(|(id, alive)| {
                serde_json::json!({
                    "session_id": id,
                    "alive": alive,
                })
            })
            .collect();
        let payload = serde_json::to_string(&items).into_diagnostic()?;
        println!("{payload}");
    } else {
        if sessions.is_empty() {
            eprintln!("no active sessions");
            return Ok(());
        }
        for (id, alive) in &sessions {
            let status = if *alive { "alive" } else { "stale" };
            println!("{id}  {status}");
        }
    }
    Ok(())
}

/// Handle the _serve command (internal daemon).
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn cmd_serve(
    session_id: String,
    policy_path: Option<PathBuf>,
    cwd: Option<String>,
    idle_timeout: Option<u64>,
    no_sandbox: bool,
    ack_unsafe_sandbox: bool,
    enable_network: bool,
    ack_unsafe_network: bool,
    ack_unsafe_write: bool,
    strict_write: bool,
    command: Vec<String>,
) -> Result<()> {
    // Detach from parent process group so we survive the parent exiting
    #[cfg(unix)]
    {
        let _ = nix::unistd::setsid();
    }

    let (cmd, args) = split_command(command)?;
    let mut policy = match policy_path {
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

    let socket_path = session_client::socket_path(&session_id);
    let timeout_secs = idle_timeout.unwrap_or(1800);

    let config = ptybox::serve::ServeConfig {
        session_id,
        socket_path,
        command: cmd,
        args,
        cwd,
        policy,
        artifacts: None,
        idle_timeout: std::time::Duration::from_secs(timeout_secs),
        initial_output: Box::new(std::io::stdout()),
    };

    match ptybox::serve::run_serve(config) {
        Ok(()) => Ok(()),
        Err(err) => {
            // Write error as ready message for the parent to read
            let error_json = serde_json::json!({
                "ok": false,
                "session_id": null,
                "screen": null,
                "error": err.message,
            });
            let _ = writeln!(std::io::stdout(), "{error_json}");
            std::process::exit(exit_code_for_error(&err));
        }
    }
}

/// Common handler for session commands that return a screen.
fn handle_session_response(
    session_id: &str,
    request: &ptybox::serve::protocol::ServeRequest,
    json: bool,
) -> Result<()> {
    let resp = session_client::send_request(session_id, request);
    match resp {
        Ok(r) if r.ok => {
            if json {
                emit_json(&r)?;
            } else if let Some(ref screen) = r.screen {
                let text = session_client::format_screen_text(screen);
                println!("{text}");
            }
            Ok(())
        }
        Ok(r) => {
            let msg = r.error.as_deref().unwrap_or("unknown error");
            if json {
                emit_json(&r)?;
            } else {
                eprintln!("error: {msg}");
            }
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
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
