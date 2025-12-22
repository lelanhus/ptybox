use clap::{Parser, Subcommand};
use miette::{IntoDiagnostic, Result};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use tui_use::artifacts::ArtifactsWriterConfig;
use tui_use::model::policy::Policy;
use tui_use::runner::{
    RunnerError, RunnerOptions, load_scenario, run_exec_with_options, run_scenario,
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
        cwd: Option<String>,
        #[arg(long)]
        artifacts: Option<PathBuf>,
        #[arg(long)]
        overwrite: bool,
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },
    Run {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        scenario: PathBuf,
        #[arg(long)]
        artifacts: Option<PathBuf>,
        #[arg(long)]
        overwrite: bool,
    },
    Driver {
        #[arg(long)]
        stdio: bool,
        #[arg(long)]
        json: bool,
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
            cwd,
            artifacts,
            overwrite,
            command,
        } => {
            let (cmd, args) = split_command(command)?;
            let policy = match policy {
                Some(path) => load_policy_file(&path)?,
                None => Policy::default(),
            };
            let options = RunnerOptions {
                artifacts: artifacts.map(|dir| ArtifactsWriterConfig { dir, overwrite }),
            };
            let result = run_exec_with_options(cmd, args, cwd, policy, options);
            emit_result(json, result)
        }
        Commands::Run {
            json,
            scenario,
            artifacts,
            overwrite,
        } => {
            let scenario = load_scenario(scenario.to_str().unwrap_or(""))?;
            let options = RunnerOptions {
                artifacts: artifacts.map(|dir| ArtifactsWriterConfig { dir, overwrite }),
            };
            let result = run_scenario(scenario, options);
            emit_result(json, result)
        }
        Commands::Driver {
            stdio,
            json,
            command,
        } => {
            if !stdio || !json {
                return Err(miette::miette!("driver requires --stdio --json"));
            }
            let (cmd, args) = split_command(command)?;
            run_driver(cmd, args)
        }
    }
}

fn emit_result(json: bool, result: Result<tui_use::model::RunResult, RunnerError>) -> Result<()> {
    match result {
        Ok(run_result) => {
            if json {
                let payload = serde_json::to_string(&run_result).into_diagnostic()?;
                println!("{}", payload);
            } else {
                eprintln!("run completed: {:?}", run_result.status);
            }
            Ok(())
        }
        Err(err) => {
            if json {
                let payload = serde_json::to_string(&err.to_error_info()).into_diagnostic()?;
                println!("{}", payload);
            } else {
                eprintln!("error: {}", err);
            }
            Err(miette::miette!(err.to_string()))
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

fn run_driver(command: String, args: Vec<String>) -> Result<()> {
    use tui_use::model::{Action, ActionType, RunId};
    use tui_use::session::{Session, SessionConfig};
    let run_id = RunId::new();
    let mut session = Session::spawn(SessionConfig {
        command,
        args,
        cwd: None,
        size: tui_use::model::TerminalSize::default(),
        run_id,
        env: Policy::default().env,
    })?;

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line.into_diagnostic()?;
        if line.trim().is_empty() {
            continue;
        }
        let action: Action =
            serde_json::from_str(&line).map_err(|_| miette::miette!("invalid json action"))?;
        session.send(&action)?;
        let observation = session.observe(std::time::Duration::from_millis(50))?;
        let payload = serde_json::to_string(&observation).into_diagnostic()?;
        writeln!(stdout, "{}", payload).into_diagnostic()?;
        stdout.flush().into_diagnostic()?;

        if matches!(action.action_type, ActionType::Terminate) {
            break;
        }
    }

    Ok(())
}
