//! Stateless session server backed by a Unix domain socket.
//!
//! The serve module implements a background daemon that holds a PTY session
//! and accepts one-shot commands over a UDS. Each client connection sends one
//! `ServeRequest`, receives one `ServeResponse`, then disconnects.
//!
//! The daemon exits when it receives a `Close` command, when the idle timeout
//! fires, or when the child process exits.

pub mod protocol;

use crate::actions::perform_action;
use crate::model::policy::Policy;
use crate::model::{Action, ActionType, RunConfig, RunId, TerminalSize};
use crate::policy::{
    validate_artifacts_dir, validate_artifacts_policy, validate_policy, validate_write_access,
    EffectivePolicy,
};
use crate::runner::{RunnerError, RunnerResult};
use crate::session::{Session, SessionConfig};
use crate::util::{build_spawn_command, resolve_artifacts_config, SandboxCleanupGuard};
use protocol::{ScreenOutput, ServeCommand, ServeRequest, ServeResponse};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::artifacts::{ArtifactsWriter, ArtifactsWriterConfig};

/// Configuration for the serve daemon.
pub struct ServeConfig {
    /// Unique session identifier (8-char hex).
    pub session_id: String,
    /// Path to the Unix domain socket.
    pub socket_path: PathBuf,
    /// Command to spawn.
    pub command: String,
    /// Command arguments.
    pub args: Vec<String>,
    /// Optional working directory.
    pub cwd: Option<String>,
    /// Security policy.
    pub policy: Policy,
    /// Optional artifacts configuration.
    pub artifacts: Option<ArtifactsWriterConfig>,
    /// Idle timeout before the daemon shuts down (default: 30 minutes).
    pub idle_timeout: Duration,
    /// Writer for the initial ready message (typically stdout piped to parent).
    pub initial_output: Box<dyn Write + Send>,
}

/// Initial message sent from daemon to the `open` command via piped stdout.
#[derive(serde::Serialize)]
struct ReadyMessage {
    ok: bool,
    session_id: String,
    screen: Option<ScreenOutput>,
    error: Option<String>,
}

/// Run the serve daemon loop.
///
/// 1. Validates policy and spawns the session.
/// 2. Observes the initial screen.
/// 3. Writes the initial screen as JSON to `initial_output`, then drops it.
/// 4. Binds a UDS and enters the accept loop.
/// 5. Exits on: close command, idle timeout, or child process exit.
///
/// # Errors
///
/// Returns [`RunnerError`] if policy validation fails, the session cannot be
/// spawned, or the UDS cannot be bound.
#[allow(clippy::too_many_lines)]
pub fn run_serve(mut config: ServeConfig) -> RunnerResult<()> {
    // --- Policy validation (reuses the driver/runner validation chain) ---
    validate_policy(&config.policy)?;
    validate_artifacts_policy(&config.policy)?;
    let effective_policy = EffectivePolicy::new(config.policy.clone());
    let run_config = RunConfig {
        command: config.command.clone(),
        args: config.args.clone(),
        cwd: config.cwd.clone(),
        initial_size: TerminalSize::default(),
        policy: crate::model::scenario::PolicyRef::Inline(Box::new(config.policy.clone())),
    };
    effective_policy.validate_run_config(&run_config)?;

    let artifacts_config = resolve_artifacts_config(&config.policy, config.artifacts.take());
    let artifacts_dir = artifacts_config.as_ref().map(|cfg| cfg.dir.clone());
    validate_write_access(&config.policy, artifacts_dir.as_deref())?;
    if let Some(cfg) = artifacts_config.as_ref() {
        validate_artifacts_dir(&cfg.dir, &config.policy.fs)?;
    }

    let run_id = RunId::new();
    let mut _writer = if let Some(cfg) = artifacts_config {
        Some(ArtifactsWriter::new(run_id, cfg)?)
    } else {
        None
    };

    // --- Spawn session ---
    let spawn = build_spawn_command(
        &config.policy,
        &config.command,
        &config.args,
        artifacts_dir.as_ref(),
        run_id,
    )?;
    let _cleanup_guard = SandboxCleanupGuard::new(spawn.cleanup_path.clone());

    let mut session = Session::spawn(SessionConfig {
        command: spawn.command,
        args: spawn.args,
        cwd: config.cwd.clone(),
        size: TerminalSize::default(),
        run_id,
        env: config.policy.env.clone(),
    })?;

    // --- Initial observation ---
    let initial_obs = session.observe(Duration::from_millis(500))?;
    let initial_screen = ScreenOutput::from_observation(&initial_obs);

    // Write ready message to parent then drop the pipe
    let ready = ReadyMessage {
        ok: true,
        session_id: config.session_id.clone(),
        screen: Some(initial_screen),
        error: None,
    };
    let ready_json = serde_json::to_string(&ready)
        .map_err(|e| RunnerError::io_err("failed to serialize ready message", e))?;
    writeln!(config.initial_output, "{ready_json}")
        .map_err(|e| RunnerError::io_err("failed to write ready message", e))?;
    config
        .initial_output
        .flush()
        .map_err(|e| RunnerError::io_err("failed to flush ready message", e))?;
    drop(config.initial_output);

    // --- Create socket directory ---
    let socket_dir = config
        .socket_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("/tmp/ptybox"));
    if !socket_dir.exists() {
        std::fs::create_dir_all(socket_dir)
            .map_err(|e| RunnerError::io_err("failed to create socket directory", e))?;
        // Best-effort chmod 0700 on the socket directory
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(socket_dir, std::fs::Permissions::from_mode(0o700));
        }
    }

    // Clean up stale socket file if it exists
    if config.socket_path.exists() {
        let _ = std::fs::remove_file(&config.socket_path);
    }

    // --- Bind UDS ---
    let listener = UnixListener::bind(&config.socket_path)
        .map_err(|e| RunnerError::io_err("failed to bind UDS", e))?;

    // Set socket file permissions to 0600
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ =
            std::fs::set_permissions(&config.socket_path, std::fs::Permissions::from_mode(0o600));
    }

    // Non-blocking so we can check idle timeout and child exit
    listener
        .set_nonblocking(true)
        .map_err(|e| RunnerError::io_err("failed to set UDS non-blocking", e))?;

    // --- Accept loop ---
    let mut last_activity = Instant::now();
    loop {
        // Check idle timeout
        if last_activity.elapsed() > config.idle_timeout {
            break;
        }

        // Check if child exited
        if let Ok(Some(_)) = session.wait_for_exit(Duration::from_millis(0)) {
            break;
        }

        // Try to accept a connection
        let stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
            Err(_) => {
                break;
            }
        };
        last_activity = Instant::now();

        // Set the connection to blocking for the request/response exchange
        if stream.set_nonblocking(false).is_err() {
            continue;
        }

        let response = handle_connection(&stream, &mut session, &config.policy);

        // Check if we should shut down
        let should_close = matches!(
            &response,
            Ok(true) // close command
        );

        // Write response (already written inside handle_connection on success path)
        if let Err(err_msg) = response {
            let resp = ServeResponse::err(err_msg);
            let _ = write_response(&stream, &resp);
        }

        if should_close {
            break;
        }
    }

    // --- Cleanup ---
    let _ = std::fs::remove_file(&config.socket_path);
    let _ = session.terminate();
    let _ = session.terminate_process_group(Duration::from_millis(500));

    Ok(())
}

/// Handle a single client connection. Returns Ok(true) if the daemon should shut down.
fn handle_connection(
    stream: &std::os::unix::net::UnixStream,
    session: &mut Session,
    policy: &Policy,
) -> Result<bool, String> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| format!("read error: {e}"))?;

    if line.trim().is_empty() {
        return Err("empty request".to_string());
    }

    let request: ServeRequest =
        serde_json::from_str(line.trim()).map_err(|e| format!("invalid request JSON: {e}"))?;

    match request.command {
        ServeCommand::Close => {
            let resp = ServeResponse::ok_empty();
            write_response(stream, &resp).map_err(|e| format!("write error: {e}"))?;
            Ok(true)
        }
        ServeCommand::Screen => {
            let obs = session
                .observe(Duration::from_millis(200))
                .map_err(|e| e.message)?;
            let resp = ServeResponse::ok(ScreenOutput::from_observation(&obs));
            write_response(stream, &resp).map_err(|e| format!("write error: {e}"))?;
            Ok(false)
        }
        ServeCommand::Keys { keys } => {
            let action = Action::key(&keys);
            let obs = perform_action(session, &action, Duration::from_millis(200), policy)
                .map_err(|e| e.message)?;
            let resp = ServeResponse::ok(ScreenOutput::from_observation(&obs));
            write_response(stream, &resp).map_err(|e| format!("write error: {e}"))?;
            Ok(false)
        }
        ServeCommand::Text { text } => {
            let action = Action::text(&text);
            let obs = perform_action(session, &action, Duration::from_millis(200), policy)
                .map_err(|e| e.message)?;
            let resp = ServeResponse::ok(ScreenOutput::from_observation(&obs));
            write_response(stream, &resp).map_err(|e| format!("write error: {e}"))?;
            Ok(false)
        }
        ServeCommand::Wait {
            contains,
            matches,
            timeout_ms,
        } => {
            let timeout = Duration::from_millis(timeout_ms.unwrap_or(5000));
            let action = if let Some(text) = contains {
                Action {
                    action_type: ActionType::Wait,
                    payload: serde_json::json!({
                        "condition": {
                            "type": "screen_contains",
                            "payload": { "text": text }
                        }
                    }),
                }
            } else if let Some(pattern) = matches {
                Action {
                    action_type: ActionType::Wait,
                    payload: serde_json::json!({
                        "condition": {
                            "type": "screen_matches",
                            "payload": { "pattern": pattern }
                        }
                    }),
                }
            } else {
                return Err("wait requires --contains or --matches".to_string());
            };
            let obs = perform_action(session, &action, timeout, policy).map_err(|e| e.message)?;
            let resp = ServeResponse::ok(ScreenOutput::from_observation(&obs));
            write_response(stream, &resp).map_err(|e| format!("write error: {e}"))?;
            Ok(false)
        }
        ServeCommand::Resize { rows, cols } => {
            let action = Action::resize(rows, cols);
            let obs = perform_action(session, &action, Duration::from_millis(200), policy)
                .map_err(|e| e.message)?;
            let resp = ServeResponse::ok(ScreenOutput::from_observation(&obs));
            write_response(stream, &resp).map_err(|e| format!("write error: {e}"))?;
            Ok(false)
        }
    }
}

/// Write a JSON response line to the stream.
fn write_response(
    mut stream: &std::os::unix::net::UnixStream,
    response: &ServeResponse,
) -> std::io::Result<()> {
    let json = serde_json::to_string(response).map_err(std::io::Error::other)?;
    writeln!(stream, "{json}")?;
    stream.flush()
}
